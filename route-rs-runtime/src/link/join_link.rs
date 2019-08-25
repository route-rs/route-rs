use crate::link::task_park::*;
use crate::link::PacketStream;
use crossbeam::atomic::AtomicCell;
use crossbeam::crossbeam_channel;
use crossbeam::crossbeam_channel::{Receiver, Sender};
use futures::{task, Async, Future, Poll, Stream};
use std::sync::Arc;

pub struct JoinElementLink<Packet: Sized> {
    pub consumers: Vec<JoinElementConsumer<Packet>>,
    pub provider: JoinElementProvider<Packet>,
}

impl<Packet: Sized> JoinElementLink<Packet> {
    pub fn new(input_streams: Vec<PacketStream<Packet>>, queue_capacity: usize) -> Self {
        assert!(
            input_streams.len() <= 1000,
            format!("input_streams len {} > 1000", input_streams.len())
        ); //Let's be reasonable here
        assert!(
            queue_capacity <= 1000,
            format!("Split Element queue_capacity: {} > 1000", queue_capacity)
        );
        assert_ne!(queue_capacity, 0, "queue capacity must be non-zero");

        let number_consumers = input_streams.len();

        let mut consumers: Vec<JoinElementConsumer<Packet>> = Vec::new();
        let mut from_consumers: Vec<Receiver<Option<Packet>>> = Vec::new();
        let mut task_parks: Vec<Arc<AtomicCell<TaskParkState>>> = Vec::new();

        for input_stream in input_streams {
            let (to_provider, from_consumer) =
                crossbeam_channel::bounded::<Option<Packet>>(queue_capacity);
            let task_park = Arc::new(AtomicCell::new(TaskParkState::Empty));

            let consumer =
                JoinElementConsumer::new(input_stream, to_provider.clone(), Arc::clone(&task_park));
            consumers.push(consumer);
            from_consumers.push(from_consumer);
            task_parks.push(task_park);
        }

        let provider =
            JoinElementProvider::new(from_consumers.clone(), task_parks, number_consumers);

        JoinElementLink {
            consumers,
            provider,
        }
    }
}

pub struct JoinElementConsumer<Packet: Sized> {
    input_stream: PacketStream<Packet>,
    to_provider: Sender<Option<Packet>>,
    task_park: Arc<AtomicCell<TaskParkState>>,
}

impl<Packet: Sized> JoinElementConsumer<Packet> {
    fn new(
        input_stream: PacketStream<Packet>,
        to_provider: Sender<Option<Packet>>,
        task_park: Arc<AtomicCell<TaskParkState>>,
    ) -> Self {
        JoinElementConsumer {
            input_stream,
            to_provider,
            task_park,
        }
    }
}

impl<Packet: Sized> Drop for JoinElementConsumer<Packet> {
    fn drop(&mut self) {
        if let Err(err) = self.to_provider.try_send(None) {
            panic!("Consumer: Drop: try_send to_provider, fail?: {:?}", err);
        }
        die_and_notify(&self.task_park);
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
                park_and_notify(&self.task_park);
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
                    unpark_and_notify(&self.task_park);
                }
            }
        }
    }
}

#[allow(dead_code)]
pub struct JoinElementProvider<Packet: Sized> {
    from_consumers: Vec<Receiver<Option<Packet>>>,
    task_parks: Vec<Arc<AtomicCell<TaskParkState>>>,
    consumers_alive: usize,
    most_recent_consumer: usize,
}

impl<Packet: Sized> JoinElementProvider<Packet> {
    fn new(
        from_consumers: Vec<Receiver<Option<Packet>>>,
        task_parks: Vec<Arc<AtomicCell<TaskParkState>>>,
        consumers_alive: usize,
    ) -> Self {
        let most_recent_consumer = 0;
        JoinElementProvider {
            from_consumers,
            task_parks,
            consumers_alive,
            most_recent_consumer,
        }
    }
}

impl<Packet: Sized> Drop for JoinElementProvider<Packet> {
    fn drop(&mut self) {
        for task_park in self.task_parks.iter() {
            die_and_notify(&task_park);
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
                    unpark_and_notify(&self.task_parks[port + self.most_recent_consumer]);
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
                    unpark_and_notify(&self.task_parks[port]);
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

        // We could not get a packet from any of our consumers, this means we will park our task in a
        // common location, and then hand out Arcs to all the consumers to the common location. The first
        // one to access the provider task will awaken us, so we can continue providing packets.
        let mut parked_consumer_task = false;
        let provider_task = Arc::new(AtomicCell::new(Some(task::current())));
        for task_park in self.task_parks.iter() {
            if indirect_park_and_notify(&task_park, Arc::clone(&provider_task)) {
                parked_consumer_task = true;
            }
        }
        //we were unable to park task, so we must self notify, presumably all the consumers are dead.
        if !parked_consumer_task {
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

        let mut input_streams: Vec<PacketStream<usize>> = Vec::new();
        input_streams.push(Box::new(packet_generator0));
        input_streams.push(Box::new(packet_generator1));

        let mut join0_link = JoinElementLink::new(input_streams, default_channel_size);
        let join0_input1_drain = join0_link.consumers.pop().unwrap();
        let join0_input0_drain = join0_link.consumers.pop().unwrap();

        let (s, join0_collector_output) = crossbeam_channel::unbounded();
        let join0_collector = ExhaustiveCollector::new(0, Box::new(join0_link.provider), s);

        tokio::run(lazy(|| {
            tokio::spawn(join0_input0_drain);
            tokio::spawn(join0_input1_drain);
            tokio::spawn(join0_collector);
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

        let mut input_streams: Vec<PacketStream<usize>> = Vec::new();
        input_streams.push(Box::new(packet_generator0));
        input_streams.push(Box::new(packet_generator1));

        let mut join0_link = JoinElementLink::new(input_streams, default_channel_size);
        let join0_input1_drain = join0_link.consumers.pop().unwrap();
        let join0_input0_drain = join0_link.consumers.pop().unwrap();

        let (s, join0_collector_output) = crossbeam_channel::unbounded();
        let join0_collector = ExhaustiveCollector::new(0, Box::new(join0_link.provider), s);

        tokio::run(lazy(|| {
            tokio::spawn(join0_input0_drain);
            tokio::spawn(join0_input1_drain);
            tokio::spawn(join0_collector);
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

        let mut input_streams: Vec<PacketStream<usize>> = Vec::new();
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

        tokio::run(lazy(|| {
            tokio::spawn(join0_input0_drain);
            tokio::spawn(join0_input1_drain);
            tokio::spawn(join0_input2_drain);
            tokio::spawn(join0_input3_drain);
            tokio::spawn(join0_input4_drain);
            tokio::spawn(join0_collector);
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

        let mut input_streams: Vec<PacketStream<usize>> = Vec::new();
        input_streams.push(Box::new(packet_generator0));
        input_streams.push(Box::new(packet_generator1));

        let mut join0_link = JoinElementLink::new(input_streams, default_channel_size);
        let join0_input1_drain = join0_link.consumers.pop().unwrap();
        let join0_input0_drain = join0_link.consumers.pop().unwrap();

        let (s, join0_collector_output) = crossbeam_channel::unbounded();
        let join0_collector = ExhaustiveCollector::new(0, Box::new(join0_link.provider), s);

        tokio::run(lazy(|| {
            tokio::spawn(join0_input0_drain);
            tokio::spawn(join0_input1_drain);
            tokio::spawn(join0_collector);
            Ok(())
        }));

        let join0_output: Vec<_> = join0_collector_output.iter().collect();
        assert_eq!(join0_output.len(), packets.len() * 2);
    }

    #[test]
    fn join_element_small_channel() {
        let default_channel_size = 1;
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9, 11];
        let packet_generator0 = immediate_stream(packets.clone());
        let packet_generator1 = immediate_stream(packets.clone());

        let mut input_streams: Vec<PacketStream<usize>> = Vec::new();
        input_streams.push(Box::new(packet_generator0));
        input_streams.push(Box::new(packet_generator1));

        let mut join0_link = JoinElementLink::new(input_streams, default_channel_size);
        let join0_input1_drain = join0_link.consumers.pop().unwrap();
        let join0_input0_drain = join0_link.consumers.pop().unwrap();

        let (s, join0_collector_output) = crossbeam_channel::unbounded();
        let join0_collector = ExhaustiveCollector::new(0, Box::new(join0_link.provider), s);

        tokio::run(lazy(|| {
            tokio::spawn(join0_input0_drain);
            tokio::spawn(join0_input1_drain);
            tokio::spawn(join0_collector);
            Ok(())
        }));

        let join0_output: Vec<_> = join0_collector_output.iter().collect();
        assert_eq!(join0_output.len(), packets.len() * 2);
    }

    #[test]
    fn join_element_empty_stream() {
        let default_channel_size = 10;
        let packets = vec![];
        let packet_generator0 = immediate_stream(packets.clone());
        let packet_generator1 = immediate_stream(packets.clone());

        let mut input_streams: Vec<PacketStream<usize>> = Vec::new();
        input_streams.push(Box::new(packet_generator0));
        input_streams.push(Box::new(packet_generator1));

        let mut join0_link = JoinElementLink::new(input_streams, default_channel_size);
        let join0_input1_drain = join0_link.consumers.pop().unwrap();
        let join0_input0_drain = join0_link.consumers.pop().unwrap();

        let (s, join0_collector_output) = crossbeam_channel::unbounded();
        let join0_collector = ExhaustiveCollector::new(0, Box::new(join0_link.provider), s);

        tokio::run(lazy(|| {
            tokio::spawn(join0_input0_drain);
            tokio::spawn(join0_input1_drain);
            tokio::spawn(join0_collector);
            Ok(())
        }));

        let join0_output: Vec<_> = join0_collector_output.iter().collect();
        assert_eq!(join0_output.len(), packets.len() * 2);
    }

    #[test]
    #[should_panic(expected = "queue capacity must be non-zero")]
    fn one_join_element_empty_channel() {
        let default_channel_size = 0;
        let packets = vec![];
        let packet_generator0 = immediate_stream(packets.clone());
        let packet_generator1 = immediate_stream(packets.clone());

        let mut input_streams: Vec<PacketStream<usize>> = Vec::new();
        input_streams.push(Box::new(packet_generator0));
        input_streams.push(Box::new(packet_generator1));

        let mut _join0_link = JoinElementLink::new(input_streams, default_channel_size);
    }
}
