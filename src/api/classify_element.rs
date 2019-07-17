use futures::{Future, Stream, Async, Poll, task};
use crossbeam::crossbeam_channel::{Sender, Receiver, TryRecvError};
use crossbeam::crossbeam_channel;
use crate::api::ElementStream;

pub trait ClassifyElement {
    type Packet: Sized;

    fn classify(&mut self, packet: Self::Packet) -> (usize, Self::Packet);
}

pub struct ClassifyElementLink<E: ClassifyElement> {
    pub consumer: ClassifyElementConsumer<E>,
    pub providers: Vec<ClassifyElementProvider<E>>,
}

impl<E: ClassifyElement> ClassifyElementLink<E> {
    pub fn new(input_stream: ElementStream<E::Packet>, element: E, queue_capacity: usize, branches: usize) -> Self {
        assert!( branches <= 1000, format!("Classify Element branches: {} > 1000", branches));
        assert!( queue_capacity <= 1000, format!("Classify Element queue_capacity: {} > 1000", queue_capacity));

        let mut to_providers: Vec<Sender<Option<E::Packet>>> = Vec::new();
        let mut await_providers: Vec<Sender<task::Task>> = Vec::new();
        let mut wake_providers: Vec<Receiver<task::Task>> = Vec::new();
        let mut providers: Vec<ClassifyElementProvider<E>> = Vec::new();

        for _ in 0..branches {
            let (to_provider, from_consumer) = crossbeam_channel::bounded::<Option<E::Packet>>(queue_capacity);
            let (await_consumer, wake_provider) = crossbeam_channel::bounded::<task::Task>(1);
            let (await_provider, wake_consumer) = crossbeam_channel::bounded::<task::Task>(1);

            let provider = ClassifyElementProvider::new(from_consumer, await_consumer, wake_consumer);

            to_providers.push(to_provider);
            await_providers.push(await_provider);
            wake_providers.push(wake_provider);
            providers.push(provider);
        }

        ClassifyElementLink {
            consumer: ClassifyElementConsumer::new(input_stream, to_providers, element, await_providers, wake_providers),
            providers
        }
    }
}

pub struct ClassifyElementConsumer<E: ClassifyElement> {
    input_stream: ElementStream<E::Packet>,
    to_providers: Vec<Sender<Option<E::Packet>>>,
    element: E,
    await_providers: Vec<Sender<task::Task>>,
    wake_providers: Vec<Receiver<task::Task>>
}

impl<E: ClassifyElement> ClassifyElementConsumer<E> {
    fn new(
        input_stream: ElementStream<E::Packet>,
        to_providers: Vec<Sender<Option<E::Packet>>>,
        element: E,
        await_providers: Vec<Sender<task::Task>>,
        wake_providers: Vec<Receiver<task::Task>>
    ) -> Self {
        ClassifyElementConsumer {
            input_stream,
            to_providers,
            element,
            await_providers,
            wake_providers
        }
    }
}

impl<E: ClassifyElement> Drop for ClassifyElementConsumer<E> {
    fn drop(&mut self) {
        //TODO: do this with a closure or something, this could be a one-liner
        for to_provider in self.to_providers.iter() {
            if let Err(err) = to_provider.try_send(None) {
                panic!("Consumer: Drop: try_send to provider, fail?: {:?}", err);
            }
        }

        for wake_provider in self.wake_providers.iter() {
            if let Ok(task) = wake_provider.try_recv() {
                task.notify();
            }
        }
    }
}

impl<E: ClassifyElement> Future for ClassifyElementConsumer<E> {
    type Item = ();
    type Error = ();

    /// Same logic as AsyncElementConsumer, except if any of the channels are full we
    /// await that channel to clear before processing a new packet. This is somewhat
    /// inefficient, but seems acceptable for now since we want to yield compute to 
    /// that producer, as there is a backup.
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            for (port, to_provider) in self.to_providers.iter().enumerate() {
                if to_provider.is_full() {
                    let task = task::current();
                    if let Err(_) = self.await_providers[port].try_send(task) {
                        task::current().notify();
                    }
                    return Ok(Async::NotReady)
                }
            }
            let packet_option: Option<E::Packet> = try_ready!(self.input_stream.poll());

            match packet_option {
                None => {
                    return Ok(Async::Ready(()))
                },
                Some(packet) => {
                    let (port, packet) = self.element.classify(packet);
                    if port >= self.to_providers.len() {
                        panic!("Tried to access invalid port: {}", port);
                    }
                    if let Err(err) = self.to_providers[port].try_send(Some(packet)) {
                        panic!("Error in to_providers[{}] sender, have nowhere to put packet: {:?}", port, err);
                    }
                    if let Ok(task) = self.wake_providers[port].try_recv() {
                        task.notify();
                    }
                }
            }
        }
    }
}

/// Split Element Provider, exactly the same as AsyncElementProvider, but
/// they have different trait bounds. Hence the reimplementaton. Would love
/// a PR that solves this problem.
pub struct ClassifyElementProvider<E: ClassifyElement> {
    from_consumer: crossbeam_channel::Receiver<Option<E::Packet>>,
    await_consumer: crossbeam_channel::Sender<task::Task>,
    wake_consumer: crossbeam_channel::Receiver<task::Task>,
}

impl<E: ClassifyElement> ClassifyElementProvider<E> {
    fn new(
        from_consumer: crossbeam_channel::Receiver<Option<E::Packet>>,
        await_consumer: crossbeam_channel::Sender<task::Task>,
        wake_consumer: crossbeam_channel::Receiver<task::Task>
    ) -> Self {
        ClassifyElementProvider {
            from_consumer,
            await_consumer,
            wake_consumer
        }
    }
}

impl<E: ClassifyElement> Drop for ClassifyElementProvider<E> {
    fn drop(&mut self) {
        if let Ok(task) = self.wake_consumer.try_recv() {
            task.notify();
        }
    }
}

impl<E: ClassifyElement> Stream for ClassifyElementProvider<E> {
    type Item = E::Packet;
    type Error = ();

    ///Implement Poll for Stream for AsyncElementProvider
    /// 
    /// This function, tries to retrieve a packet off the `from_consumer`
    /// channel, there are four cases: 
    /// ###
    /// #1 Ok(Some(Packet)): Got a packet. If the consumer needs, (likely due to 
    /// an until-now full channel) to be awoken, wake them. Return the Async::Ready(Option(Packet))
    /// 
    /// #2 Ok(None): this means that the consumer is in tear-down, and we
    /// will no longer be receivig packets. Return Async::Ready(None) to forward propagate teardown
    /// 
    /// #3 Err(TryRecvError::Empty): Packet queue is empty, await the consumer to awaken us with more
    /// work, and return Async::NotReady to signal to runtime to sleep this task.
    /// 
    /// #4 Err(TryRecvError::Disconnected): Consumer is in teardown and has dropped its side of the
    /// from_consumer channel; we will no longer receive packets. Return Async::Ready(None) to forward
    /// propagate teardown.
    /// ###
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.from_consumer.try_recv() {
            Ok(Some(packet)) => {
                if let Ok(task) = self.wake_consumer.try_recv() {
                        task.notify();
                }
                Ok(Async::Ready(Some(packet)))
            },
            Ok(None) => {
                Ok(Async::Ready(None))
            },
            Err(TryRecvError::Empty) => {
                let task = task::current();
                if let Err(_) = self.await_consumer.try_send(task) {
                    task::current().notify();
                }
                Ok(Async::NotReady)
            },
            Err(TryRecvError::Disconnected) => {
                Ok(Async::Ready(None))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::element::{Element};
    use crate::utils::test::packet_generators::{immediate_stream, PacketIntervalGenerator};
    use crate::utils::test::packet_collectors::ExhaustiveCollector;
    use crossbeam::crossbeam_channel;
    use core::time;

    use futures::future::lazy;

    #[allow(dead_code)]
    struct IdentityElement {
        id: i32
    }

    impl Element for IdentityElement {
        type Input = i32;
        type Output = i32;

        fn process(&mut self, packet: Self::Input) -> Self::Output {
            packet
        }
    }

    #[allow(dead_code)]
    struct ClassifyEvenOddElement {
        id: i32
    }

    impl ClassifyElement for ClassifyEvenOddElement {
        type Packet = i32;

        fn classify(&mut self, packet: Self::Packet) -> (usize, Self::Packet) {
            if (packet % 2) == 0 {
                (0, packet)
            } else {
                (1, packet)
            }
        }
    }   

    #[test]
    fn one_classify_element() {
        let default_channel_size = 10;
        let number_branches = 2;
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9, 11];
        let packet_generator = PacketIntervalGenerator::new(time::Duration::from_millis(100), packets.clone().into_iter());

        let elem0 = ClassifyEvenOddElement { id: 0 };

        let mut elem0_link = ClassifyElementLink::new(Box::new(packet_generator), elem0, default_channel_size, number_branches);
        let elem0_drain = elem0_link.consumer;

        // Ordering is important since we are popping.   
        let (s1, elem0_port1_collector_output) = crossbeam_channel::unbounded();
        let elem0_port1_collector = ExhaustiveCollector::new(0, Box::new(elem0_link.providers.pop().unwrap()), s1);

        let (s0, elem0_port0_collector_output) = crossbeam_channel::unbounded();
        let elem0_port0_collector = ExhaustiveCollector::new(0, Box::new(elem0_link.providers.pop().unwrap()), s0);

        tokio::run(lazy (|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem0_port0_collector);
            tokio::spawn(elem0_port1_collector);
            Ok(())
        }));

        let elem0_port0_output: Vec<_> = elem0_port0_collector_output.iter().collect();
        assert_eq!(elem0_port0_output, vec![0, 2, 420, 4, 6, 8]);

        let elem0_port1_output: Vec<_> = elem0_port1_collector_output.iter().collect();
        assert_eq!(elem0_port1_output, vec![1, 1337, 3, 5, 7, 9, 11]);
    }

    #[test]
    fn one_classify_element_only_odd() {
        let default_channel_size = 5;
        let number_branches = 2;
        let packets = vec![1, 1337, 3, 5, 7, 9, 11];
        let packet_generator = PacketIntervalGenerator::new(time::Duration::from_millis(100), packets.clone().into_iter());

        let elem0 = ClassifyEvenOddElement { id: 0 };

        let mut elem0_link = ClassifyElementLink::new(Box::new(packet_generator), elem0, default_channel_size, number_branches);
        let elem0_drain = elem0_link.consumer;

        // Ordering is important since we are popping.   
        let (s1, elem0_port1_collector_output) = crossbeam_channel::unbounded();
        let elem0_port1_collector = ExhaustiveCollector::new(0, Box::new(elem0_link.providers.pop().unwrap()), s1);

        let (s0, elem0_port0_collector_output) = crossbeam_channel::unbounded();
        let elem0_port0_collector = ExhaustiveCollector::new(0, Box::new(elem0_link.providers.pop().unwrap()), s0);

        tokio::run(lazy (|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem0_port0_collector);
            tokio::spawn(elem0_port1_collector);
            Ok(())
        }));

        let elem0_port0_output: Vec<_> = elem0_port0_collector_output.iter().collect();
        assert!(elem0_port0_output.is_empty());

        let elem0_port1_output: Vec<_> = elem0_port1_collector_output.iter().collect();
        assert_eq!(elem0_port1_output, vec![1, 1337, 3, 5, 7, 9, 11]);
    }

    #[test]
    fn one_classify_element_immediate_yield() {
        let default_channel_size = 10;
        let number_branches = 2;
        let packet_generator = immediate_stream(0..=2000);

        let elem0 = ClassifyEvenOddElement { id: 0 };

        let mut elem0_link = ClassifyElementLink::new(Box::new(packet_generator), elem0, default_channel_size, number_branches);
        let elem0_drain = elem0_link.consumer;

        // Ordering is important since we are popping.   
        let (s1, elem0_port1_collector_output) = crossbeam_channel::unbounded();
        let elem0_port1_collector = ExhaustiveCollector::new(0, Box::new(elem0_link.providers.pop().unwrap()), s1);

        let (s0, elem0_port0_collector_output) = crossbeam_channel::unbounded();
        let elem0_port0_collector = ExhaustiveCollector::new(0, Box::new(elem0_link.providers.pop().unwrap()), s0);

        tokio::run(lazy (|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem0_port0_collector);
            tokio::spawn(elem0_port1_collector);
            Ok(())
        }));

        let elem0_port0_output: Vec<_> = elem0_port0_collector_output.iter().collect();
        assert_eq!(elem0_port0_output.len(), 1001);

        let elem0_port1_output: Vec<_> = elem0_port1_collector_output.iter().collect();
        assert_eq!(elem0_port1_output.len(), 1000);
    }
}