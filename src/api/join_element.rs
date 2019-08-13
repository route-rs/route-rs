use crate::api::ElementStream;
use crossbeam::crossbeam_channel;
use crossbeam::crossbeam_channel::{Receiver, Sender, TryRecvError};
use futures::{task, Async, Future, Poll, Stream};

pub struct JoinElementLink<Packet: Sized> {
    pub consumers: Vec<JoinElementConsumer<Packet>>,
    pub provider: JoinElementProvider<Packet>,
    pub overseer: JoinElementOverseer<Packet>,
}

impl<Packet: Sized> JoinElementLink<Packet> {
    pub fn new(input_streams: Vec<ElementStream<Packet>>, queue_capacity: usize) -> Self {
        assert!(
            input_streams.len() <= 1000,
            format!("input_steams len {} > 1000", input_streams.len())
        ); //Let's be reasonable here
        assert!(
            queue_capacity <= 1000,
            format!("Split Element queue_capacity: {} > 1000", queue_capacity)
        );

        let number_consumers = input_streams.len();

        let (await_consumers, wake_provider) = crossbeam_channel::bounded::<task::Task>(1);

        let mut consumers: Vec<JoinElementConsumer<Packet>> = Vec::new();
        let mut from_consumers: Vec<Receiver<Option<Packet>>> = Vec::new();
        let mut wake_consumers: Vec<Receiver<task::Task>> = Vec::new();
        let mut overseer_wake_consumers: Vec<Option<Receiver<task::Task>>> = Vec::new();

        for input_stream in input_streams {
            let (to_provider, from_consumer) =
                crossbeam_channel::bounded::<Option<Packet>>(queue_capacity);
            let (await_provider, wake_consumer) =
                crossbeam_channel::bounded::<task::Task>(number_consumers);

            let consumer = JoinElementConsumer::new(
                input_stream,
                to_provider.clone(),
                await_provider,
                wake_provider.clone(),
            );
            consumers.push(consumer);
            from_consumers.push(from_consumer);
            wake_consumers.push(wake_consumer.clone());
            overseer_wake_consumers.push(Some(wake_consumer));
        }

        let provider = JoinElementProvider::new(
            from_consumers.clone(),
            await_consumers,
            wake_consumers,
            number_consumers,
        );
        let overseer =
            JoinElementOverseer::new(from_consumers, overseer_wake_consumers, wake_provider);

        JoinElementLink {
            consumers,
            provider,
            overseer,
        }
    }
}

pub struct JoinElementOverseer<Packet: Sized> {
    from_consumers: Vec<Receiver<Option<Packet>>>,
    wake_consumers: Vec<Option<Receiver<task::Task>>>,
    wake_provider: Receiver<task::Task>,
    consumers_alive: usize,
}

impl<Packet: Sized> JoinElementOverseer<Packet> {
    fn new(
        from_consumers: Vec<Receiver<Option<Packet>>>,
        wake_consumers: Vec<Option<Receiver<task::Task>>>,
        wake_provider: Receiver<task::Task>,
    ) -> Self {
        let consumers_alive = from_consumers.len();
        JoinElementOverseer {
            from_consumers,
            wake_consumers,
            wake_provider,
            consumers_alive,
        }
    }
}

impl<Packet: Sized> Future for JoinElementOverseer<Packet> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        for (port, from_consumer) in self.from_consumers.iter().enumerate() {
            if from_consumer.is_empty() {
                match &self.wake_consumers[port] {
                    Some(wake_consumer) => {
                        match wake_consumer.try_recv() {
                            Ok(task) => {
                                task.notify();
                            }
                            Err(TryRecvError::Empty) => {}
                            Err(TryRecvError::Disconnected) => {
                                // Only return once all the consumers are disconnected and the provider has emptied their channels.
                                self.consumers_alive -= 1;
                                // This ensures we don't continue to count a disconnected wake_consumer
                                self.wake_consumers[port] = None;
                                if self.consumers_alive == 0 {
                                    return Ok(Async::Ready(()));
                                }
                            }
                        }
                    }
                    None => {}
                }
            } else {
                //In this case we found a non-empty channel.
                if let Ok(task) = self.wake_provider.try_recv() {
                    task.notify();
                }
                break; //break out of for loop, we tried to awaken the provider.
            }
        }
        task::current().notify();
        Ok(Async::NotReady)
    }
}
pub struct JoinElementConsumer<Packet: Sized> {
    input_stream: ElementStream<Packet>,
    to_provider: Sender<Option<Packet>>,
    await_provider: Sender<task::Task>,
    wake_provider: Receiver<task::Task>,
}

impl<Packet: Sized> JoinElementConsumer<Packet> {
    fn new(
        input_stream: ElementStream<Packet>,
        to_provider: Sender<Option<Packet>>,
        await_provider: Sender<task::Task>,
        wake_provider: Receiver<task::Task>,
    ) -> Self {
        JoinElementConsumer {
            input_stream,
            to_provider,
            await_provider,
            wake_provider,
        }
    }
}

impl<Packet: Sized> Drop for JoinElementConsumer<Packet> {
    fn drop(&mut self) {
        if let Err(err) = self.to_provider.try_send(None) {
            panic!("Consumer: Drop: try_send to_provider, fail?: {:?}", err);
        }
        if let Ok(task) = self.wake_provider.try_recv() {
            task.notify();
        }
    }
}

impl<Packet: Sized> Future for JoinElementConsumer<Packet> {
    type Item = ();
    type Error = ();

    /// Implement Poll for Future for JoinElementConsumer
    ///
    /// Note that this function works a bit different, it continues to process
    /// packets off it's input queue until it reaches a point where it can not
    /// make forward progress. There are three cases:
    /// ###
    /// #1 The to_provider queue is full, we notify the provider that we need
    /// awaking when there is work to do, and go to sleep.
    ///
    /// #2 The input_stream returns a NotReady, we sleep, with the assumption
    /// that whomever produced the NotReady will awaken the task in the Future.
    ///
    /// #3 We get a Ready(None), in which case we push a None onto the to_provider
    /// queue and then return Ready(()), which means we enter tear-down, since there
    /// is no futher work to complete.
    /// ###
    /// By Sleep, we mean we return a NotReady to the runtime which will sleep the task.
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            if self.to_provider.is_full() {
                let task = task::current();
                if self.await_provider.try_send(task).is_err() {
                    task::current().notify();
                }
                return Ok(Async::NotReady);
            }
            let input_packet_option: Option<Packet> = try_ready!(self.input_stream.poll());

            match input_packet_option {
                None => return Ok(Async::Ready(())),
                Some(packet) => {
                    if let Err(err) = self.to_provider.try_send(Some(packet)) {
                        panic!(
                            "Error in to_provider sender, have nowhere to put packet: {:?}",
                            err
                        );
                    }
                    if let Ok(task) = self.wake_provider.try_recv() {
                        task.notify();
                    }
                }
            }
        }
    }
}

#[allow(dead_code)]
pub struct JoinElementProvider<Packet: Sized> {
    from_consumers: Vec<Receiver<Option<Packet>>>,
    await_consumers: Sender<task::Task>,
    wake_consumers: Vec<Receiver<task::Task>>,
    consumers_alive: usize,
    most_recent_consumer: usize,
}

impl<Packet: Sized> JoinElementProvider<Packet> {
    fn new(
        from_consumers: Vec<Receiver<Option<Packet>>>,
        await_consumers: Sender<task::Task>,
        wake_consumers: Vec<Receiver<task::Task>>,
        consumers_alive: usize,
    ) -> Self {
        let most_recent_consumer = 0;
        JoinElementProvider {
            from_consumers,
            await_consumers,
            wake_consumers,
            consumers_alive,
            most_recent_consumer,
        }
    }
}

impl<Packet: Sized> Drop for JoinElementProvider<Packet> {
    fn drop(&mut self) {
        for wake_consumer in self.wake_consumers.iter() {
            if let Ok(task) = wake_consumer.try_recv() {
                task.notify();
            }
        }
    }
}

impl<Packet: Sized> Stream for JoinElementProvider<Packet> {
    type Item = Packet;
    type Error = ();

    /// Iterate over all the channels, pull the first packet that is available.
    /// this is unfair if we don't start from a new point each time. But it should
    /// work as a POC with the unfair version to start. The
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let (before_recent, after_recent) = self.from_consumers.split_at(self.most_recent_consumer);
        for (port, from_consumer) in after_recent.iter().enumerate() {
            match from_consumer.try_recv() {
                Ok(Some(packet)) => {
                    if let Ok(task) =
                        self.wake_consumers[port + self.most_recent_consumer].try_recv()
                    {
                        task.notify();
                    }
                    self.most_recent_consumer += port;
                    return Ok(Async::Ready(Some(packet)));
                }
                Ok(None) => {
                    //Got a none from a consumer that has shutdown
                    self.consumers_alive -= 1;
                    if self.consumers_alive == 0 {
                        return Ok(Async::Ready(None));
                    }
                }
                Err(_) => {
                    //On an error go to next channel.
                }
            }
        }

        for (port, from_consumer) in before_recent.iter().enumerate() {
            match from_consumer.try_recv() {
                Ok(Some(packet)) => {
                    if let Ok(task) = self.wake_consumers[port].try_recv() {
                        task.notify();
                    }
                    self.most_recent_consumer = port;
                    return Ok(Async::Ready(Some(packet)));
                }
                Ok(None) => {
                    //Got a none from a consumer that has shutdown
                    self.consumers_alive -= 1;
                    if self.consumers_alive == 0 {
                        return Ok(Async::Ready(None));
                    }
                }
                Err(_) => {
                    //On an error go to next channel.
                }
            }
        }

        let task = task::current();
        if self.await_consumers.try_send(task).is_err() {
            task::current().notify();
        }
        Ok(Async::NotReady)
    }
}

#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use super::*;
    use crate::utils::test::packet_collectors::ExhaustiveCollector;
    use crate::utils::test::packet_generators::{immediate_stream, PacketIntervalGenerator};
    use core::time;
    use crossbeam::crossbeam_channel;

    use futures::future::lazy;

    #[test]
    fn join_element() {
        let default_channel_size = 10;
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9, 11];
        let packet_generator0 = immediate_stream(packets.clone());
        let packet_generator1 = immediate_stream(packets.clone());

        let mut input_streams: Vec<ElementStream<usize>> = Vec::new();
        input_streams.push(Box::new(packet_generator0));
        input_streams.push(Box::new(packet_generator1));

        let mut join0_link = JoinElementLink::new(input_streams, default_channel_size);
        let join0_input1_drain = join0_link.consumers.pop().unwrap();
        let join0_input0_drain = join0_link.consumers.pop().unwrap();

        let (s, join0_collector_output) = crossbeam_channel::unbounded();
        let join0_collector = ExhaustiveCollector::new(0, Box::new(join0_link.provider), s);

        let join0_overseer = join0_link.overseer;

        tokio::run(lazy(|| {
            tokio::spawn(join0_input0_drain);
            tokio::spawn(join0_input1_drain);
            tokio::spawn(join0_collector);
            tokio::spawn(join0_overseer);
            Ok(())
        }));

        let join0_output: Vec<_> = join0_collector_output.iter().collect();
        assert_eq!(join0_output.len(), packets.len() * 2);
    }

    #[test]
    fn one_join_element_long_stream() {
        let default_channel_size = 10;
        let packet_generator0 = immediate_stream(0..=2000);
        let packet_generator1 = immediate_stream(0..=2000);

        let mut input_streams: Vec<ElementStream<usize>> = Vec::new();
        input_streams.push(Box::new(packet_generator0));
        input_streams.push(Box::new(packet_generator1));

        let mut join0_link = JoinElementLink::new(input_streams, default_channel_size);
        let join0_input1_drain = join0_link.consumers.pop().unwrap();
        let join0_input0_drain = join0_link.consumers.pop().unwrap();

        let (s, join0_collector_output) = crossbeam_channel::unbounded();
        let join0_collector = ExhaustiveCollector::new(0, Box::new(join0_link.provider), s);

        let join0_overseer = join0_link.overseer;

        tokio::run(lazy(|| {
            tokio::spawn(join0_input0_drain);
            tokio::spawn(join0_input1_drain);
            tokio::spawn(join0_collector);
            tokio::spawn(join0_overseer);
            Ok(())
        }));

        let join0_output: Vec<_> = join0_collector_output.iter().collect();
        assert_eq!(join0_output.len(), 4002);
    }

    #[test]
    fn five_inputs_one_join_element() {
        let default_channel_size = 10;
        let packet_generator0 = immediate_stream(0..=2000);
        let packet_generator1 = immediate_stream(0..=2000);
        let packet_generator2 = immediate_stream(0..=2000);
        let packet_generator3 = immediate_stream(0..=2000);
        let packet_generator4 = immediate_stream(0..=2000);

        let mut input_streams: Vec<ElementStream<usize>> = Vec::new();
        input_streams.push(Box::new(packet_generator0));
        input_streams.push(Box::new(packet_generator1));
        input_streams.push(Box::new(packet_generator2));
        input_streams.push(Box::new(packet_generator3));
        input_streams.push(Box::new(packet_generator4));

        let mut join0_link = JoinElementLink::new(input_streams, default_channel_size);
        let join0_input4_drain = join0_link.consumers.pop().unwrap();
        let join0_input3_drain = join0_link.consumers.pop().unwrap();
        let join0_input2_drain = join0_link.consumers.pop().unwrap();
        let join0_input1_drain = join0_link.consumers.pop().unwrap();
        let join0_input0_drain = join0_link.consumers.pop().unwrap();

        let (s, join0_collector_output) = crossbeam_channel::unbounded();
        let join0_collector = ExhaustiveCollector::new(0, Box::new(join0_link.provider), s);

        let join0_overseer = join0_link.overseer;

        tokio::run(lazy(|| {
            tokio::spawn(join0_input0_drain);
            tokio::spawn(join0_input1_drain);
            tokio::spawn(join0_input2_drain);
            tokio::spawn(join0_input3_drain);
            tokio::spawn(join0_input4_drain);
            tokio::spawn(join0_collector);
            tokio::spawn(join0_overseer);
            Ok(())
        }));

        let join0_output: Vec<_> = join0_collector_output.iter().collect();
        assert_eq!(join0_output.len(), 10005);
    }

    #[test]
    fn join_element_wait_between_packets() {
        let default_channel_size = 10;
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9, 11];
        let packet_generator0 = PacketIntervalGenerator::new(
            time::Duration::from_millis(10),
            packets.clone().into_iter(),
        );
        let packet_generator1 = PacketIntervalGenerator::new(
            time::Duration::from_millis(10),
            packets.clone().into_iter(),
        );

        let mut input_streams: Vec<ElementStream<usize>> = Vec::new();
        input_streams.push(Box::new(packet_generator0));
        input_streams.push(Box::new(packet_generator1));

        let mut join0_link = JoinElementLink::new(input_streams, default_channel_size);
        let join0_input1_drain = join0_link.consumers.pop().unwrap();
        let join0_input0_drain = join0_link.consumers.pop().unwrap();

        let (s, join0_collector_output) = crossbeam_channel::unbounded();
        let join0_collector = ExhaustiveCollector::new(0, Box::new(join0_link.provider), s);

        let join0_overseer = join0_link.overseer;

        tokio::run(lazy(|| {
            tokio::spawn(join0_input0_drain);
            tokio::spawn(join0_input1_drain);
            tokio::spawn(join0_collector);
            tokio::spawn(join0_overseer);
            Ok(())
        }));

        let join0_output: Vec<_> = join0_collector_output.iter().collect();
        assert_eq!(join0_output.len(), packets.len() * 2);
    }
}
