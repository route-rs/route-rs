use crate::api::task_park::*;
use crate::api::ElementStream;
use crossbeam::atomic::AtomicCell;
use crossbeam::crossbeam_channel;
use crossbeam::crossbeam_channel::{Receiver, Sender, TryRecvError};
use futures::{Async, Future, Poll, Stream};
use std::sync::Arc;

pub trait ClassifyElement {
    type Packet: Sized;

    fn classify(&mut self, packet: &Self::Packet) -> usize;
}

pub struct ClassifyElementLink<E: ClassifyElement> {
    pub consumer: ClassifyElementConsumer<E>,
    pub providers: Vec<ClassifyElementProvider<E>>,
}

impl<E: ClassifyElement> ClassifyElementLink<E> {
    pub fn new(
        input_stream: ElementStream<E::Packet>,
        element: E,
        queue_capacity: usize,
        branches: usize,
    ) -> Self {
        assert!(
            branches <= 1000,
            format!("Classify Element branches: {} > 1000", branches)
        );
        assert!(
            queue_capacity <= 1000,
            format!("Classify Element queue_capacity: {} > 1000", queue_capacity)
        );

        let mut to_providers: Vec<Sender<Option<E::Packet>>> = Vec::new();
        let mut providers: Vec<ClassifyElementProvider<E>> = Vec::new();

        let mut from_consumers: Vec<Receiver<Option<E::Packet>>> = Vec::new();

        let mut task_parks: Vec<Arc<AtomicCell<TaskParkState>>> = Vec::new();

        for _ in 0..branches {
            let (to_provider, from_consumer) =
                crossbeam_channel::bounded::<Option<E::Packet>>(queue_capacity);
            let task_park = Arc::new(AtomicCell::new(TaskParkState::Empty));

            let provider =
                ClassifyElementProvider::new(from_consumer.clone(), Arc::clone(&task_park));

            to_providers.push(to_provider);
            providers.push(provider);
            from_consumers.push(from_consumer);
            task_parks.push(task_park);
        }

        ClassifyElementLink {
            consumer: ClassifyElementConsumer::new(input_stream, to_providers, element, task_parks),
            providers,
        }
    }
}

pub struct ClassifyElementConsumer<E: ClassifyElement> {
    input_stream: ElementStream<E::Packet>,
    to_providers: Vec<Sender<Option<E::Packet>>>,
    element: E,
    task_parks: Vec<Arc<AtomicCell<TaskParkState>>>,
}

impl<E: ClassifyElement> ClassifyElementConsumer<E> {
    fn new(
        input_stream: ElementStream<E::Packet>,
        to_providers: Vec<Sender<Option<E::Packet>>>,
        element: E,
        task_parks: Vec<Arc<AtomicCell<TaskParkState>>>,
    ) -> Self {
        ClassifyElementConsumer {
            input_stream,
            to_providers,
            element,
            task_parks,
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

        for task_park in self.task_parks.iter() {
            die_and_notify(&task_park);
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
                    park_and_notify(&self.task_parks[port]);
                    return Ok(Async::NotReady);
                }
            }
            let packet_option: Option<E::Packet> = try_ready!(self.input_stream.poll());

            match packet_option {
                None => return Ok(Async::Ready(())),
                Some(packet) => {
                    let port = self.element.classify(&packet);
                    if port >= self.to_providers.len() {
                        panic!("Tried to access invalid port: {}", port);
                    }
                    if let Err(err) = self.to_providers[port].try_send(Some(packet)) {
                        panic!(
                            "Error in to_providers[{}] sender, have nowhere to put packet: {:?}",
                            port, err
                        );
                    }
                    unpark_and_notify(&self.task_parks[port]);
                }
            }
        }
    }
}

/// Classify Element Provider, exactly the same as AsyncElementProvider, but
/// they have different trait bounds. Hence the reimplementaton. Would love
/// a PR that solves this problem.
pub struct ClassifyElementProvider<E: ClassifyElement> {
    from_consumer: crossbeam_channel::Receiver<Option<E::Packet>>,
    task_park: Arc<AtomicCell<TaskParkState>>,
}

impl<E: ClassifyElement> ClassifyElementProvider<E> {
    fn new(
        from_consumer: crossbeam_channel::Receiver<Option<E::Packet>>,
        task_park: Arc<AtomicCell<TaskParkState>>,
    ) -> Self {
        ClassifyElementProvider {
            from_consumer,
            task_park,
        }
    }
}

impl<E: ClassifyElement> Drop for ClassifyElementProvider<E> {
    fn drop(&mut self) {
        die_and_notify(&self.task_park);
    }
}

impl<E: ClassifyElement> Stream for ClassifyElementProvider<E> {
    type Item = E::Packet;
    type Error = ();

    /// Implement Poll for Stream for ClassifyElementProvider
    ///
    /// This function, tries to retrieve a packet off the `from_consumer`
    /// channel, there are four cases:
    /// ###
    /// #1 Ok(Some(Packet)): Got a packet. If the consumer needs, (likely due to
    /// an until-now full channel) to be awoken, wake them. Return the Async::Ready(Option(Packet))
    ///
    /// #2 Ok(None): this means that the consumer is in tear-down, and we
    /// will no longer be receiving packets. Return Async::Ready(None) to forward propagate teardown
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
                unpark_and_notify(&self.task_park);
                Ok(Async::Ready(Some(packet)))
            }
            Ok(None) => Ok(Async::Ready(None)),
            Err(TryRecvError::Empty) => {
                park_and_notify(&self.task_park);
                Ok(Async::NotReady)
            }
            Err(TryRecvError::Disconnected) => Ok(Async::Ready(None)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::element::Element;
    use crate::utils::test::packet_collectors::ExhaustiveCollector;
    use crate::utils::test::packet_generators::{immediate_stream, PacketIntervalGenerator};
    use core::time;
    use crossbeam::crossbeam_channel;

    use futures::future::lazy;

    #[allow(dead_code)]
    struct IdentityElement {
        id: i32,
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
        id: i32,
    }

    impl ClassifyElement for ClassifyEvenOddElement {
        type Packet = i32;

        fn classify(&mut self, packet: &Self::Packet) -> usize {
            (packet % 2) as usize
        }
    }

    #[allow(dead_code)]
    struct ClassifyFizzBuzzElement {
        id: i32,
    }

    impl ClassifyElement for ClassifyFizzBuzzElement {
        type Packet = i32;

        fn classify(&mut self, packet: &Self::Packet) -> usize {
            if packet % 3 == 0 && packet % 5 == 0 {
                0 // FizzBuzz
            } else if packet % 3 == 0 {
                1 // Fizz
            } else if packet % 5 == 0 {
                2 // Buzz
            } else {
                3 // other
            }
        }
    }

    #[test]
    fn one_even_odd() {
        let default_channel_size = 10;
        let number_branches = 2;
        let packet_generator = immediate_stream(vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9]);

        let elem0 = ClassifyEvenOddElement { id: 0 };

        let mut elem0_link = ClassifyElementLink::new(
            Box::new(packet_generator),
            elem0,
            default_channel_size,
            number_branches,
        );
        let elem0_drain = elem0_link.consumer;

        let (s1, elem0_port1_collector_output) = crossbeam_channel::unbounded();
        let elem0_port1_collector =
            ExhaustiveCollector::new(0, Box::new(elem0_link.providers.pop().unwrap()), s1);

        let (s0, elem0_port0_collector_output) = crossbeam_channel::unbounded();
        let elem0_port0_collector =
            ExhaustiveCollector::new(0, Box::new(elem0_link.providers.pop().unwrap()), s0);

        tokio::run(lazy(|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem0_port0_collector);
            tokio::spawn(elem0_port1_collector);
            Ok(())
        }));

        let elem0_port0_output: Vec<_> = elem0_port0_collector_output.iter().collect();
        assert_eq!(elem0_port0_output, vec![0, 2, 420, 4, 6, 8]);

        let elem0_port1_output: Vec<_> = elem0_port1_collector_output.iter().collect();
        assert_eq!(elem0_port1_output, vec![1, 1337, 3, 5, 7, 9]);
    }

    #[test]
    fn one_only_odd() {
        let default_channel_size = 5;
        let number_branches = 2;
        let packet_generator = immediate_stream(vec![1, 1337, 3, 5, 7, 9]);

        let elem0 = ClassifyEvenOddElement { id: 0 };

        let mut elem0_link = ClassifyElementLink::new(
            Box::new(packet_generator),
            elem0,
            default_channel_size,
            number_branches,
        );
        let elem0_drain = elem0_link.consumer;

        let (s1, elem0_port1_collector_output) = crossbeam_channel::unbounded();
        let elem0_port1_collector =
            ExhaustiveCollector::new(0, Box::new(elem0_link.providers.pop().unwrap()), s1);

        let (s0, elem0_port0_collector_output) = crossbeam_channel::unbounded();
        let elem0_port0_collector =
            ExhaustiveCollector::new(0, Box::new(elem0_link.providers.pop().unwrap()), s0);

        tokio::run(lazy(|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem0_port0_collector);
            tokio::spawn(elem0_port1_collector);
            Ok(())
        }));

        let elem0_port0_output: Vec<_> = elem0_port0_collector_output.iter().collect();
        assert!(elem0_port0_output.is_empty());

        let elem0_port1_output: Vec<_> = elem0_port1_collector_output.iter().collect();
        assert_eq!(elem0_port1_output, vec![1, 1337, 3, 5, 7, 9]);
    }

    #[test]
    fn one_even_odd_long_stream() {
        let default_channel_size = 10;
        let number_branches = 2;
        let packet_generator = immediate_stream(0..2000);

        let even_odd_elem = ClassifyEvenOddElement { id: 0 };

        let mut even_odd_link = ClassifyElementLink::new(
            Box::new(packet_generator),
            even_odd_elem,
            default_channel_size,
            number_branches,
        );
        let even_odd_drain = even_odd_link.consumer;

        let (s1, odd_collector_output) = crossbeam_channel::unbounded();
        let odd_collector =
            ExhaustiveCollector::new(0, Box::new(even_odd_link.providers.pop().unwrap()), s1);

        let (s0, even_collector_output) = crossbeam_channel::unbounded();
        let even_collector =
            ExhaustiveCollector::new(0, Box::new(even_odd_link.providers.pop().unwrap()), s0);

        tokio::run(lazy(|| {
            tokio::spawn(even_odd_drain);
            tokio::spawn(even_collector);
            tokio::spawn(odd_collector);
            Ok(())
        }));

        let even_output: Vec<_> = even_collector_output.iter().collect();
        assert_eq!(even_output.len(), 1000);

        let odd_output: Vec<_> = odd_collector_output.iter().collect();
        assert_eq!(odd_output.len(), 1000);
    }

    #[test]
    fn one_fizz_buzz() {
        let default_channel_size = 10;
        let packet_generator = immediate_stream(0..=30);

        let elem = ClassifyFizzBuzzElement { id: 0 };

        let mut elem_link =
            ClassifyElementLink::new(Box::new(packet_generator), elem, default_channel_size, 4);
        let elem_drain = elem_link.consumer;

        let (s3, other_output) = crossbeam_channel::unbounded();
        let port3_collector =
            ExhaustiveCollector::new(0, Box::new(elem_link.providers.pop().unwrap()), s3);

        let (s2, buzz_output) = crossbeam_channel::unbounded();
        let port2_collector =
            ExhaustiveCollector::new(0, Box::new(elem_link.providers.pop().unwrap()), s2);

        let (s1, fizz_output) = crossbeam_channel::unbounded();
        let port1_collector =
            ExhaustiveCollector::new(0, Box::new(elem_link.providers.pop().unwrap()), s1);

        let (s0, fizz_buzz_output) = crossbeam_channel::unbounded();
        let port0_collector =
            ExhaustiveCollector::new(0, Box::new(elem_link.providers.pop().unwrap()), s0);

        tokio::run(lazy(|| {
            tokio::spawn(elem_drain);
            tokio::spawn(port0_collector);
            tokio::spawn(port1_collector);
            tokio::spawn(port2_collector);
            tokio::spawn(port3_collector);
            Ok(())
        }));

        let actual_fizz_buzz = fizz_buzz_output.iter().collect::<Vec<i32>>();
        let expected_fizz_buzz = vec![0, 15, 30];
        assert_eq!(expected_fizz_buzz, actual_fizz_buzz);

        let actual_fizz = fizz_output.iter().collect::<Vec<i32>>();
        let expected_fizz = vec![3, 6, 9, 12, 18, 21, 24, 27];
        assert_eq!(expected_fizz, actual_fizz);

        let actual_buzz = buzz_output.iter().collect::<Vec<i32>>();
        let expected_buzz = vec![5, 10, 20, 25];
        assert_eq!(expected_buzz, actual_buzz);

        let actual_other = other_output.iter().collect::<Vec<i32>>();
        let expected_other = vec![1, 2, 4, 7, 8, 11, 13, 14, 16, 17, 19, 22, 23, 26, 28, 29];
        assert_eq!(expected_other, actual_other);
    }

    #[test]
    fn fizz_buzz_to_even_odd() {
        let default_channel_size = 10;
        let packet_generator = immediate_stream(0..=30);

        let fizz_buzz_elem = ClassifyFizzBuzzElement { id: 0 };

        let mut fizz_buzz_elem_link: ClassifyElementLink<ClassifyFizzBuzzElement> =
            ClassifyElementLink::new(
                Box::new(packet_generator),
                fizz_buzz_elem,
                default_channel_size,
                4,
            );
        let fizz_buzz_drain = fizz_buzz_elem_link.consumer;

        let even_odd_elem = ClassifyEvenOddElement { id: 0 };

        let mut even_odd_elem_link: ClassifyElementLink<ClassifyEvenOddElement> =
            ClassifyElementLink::new(
                Box::new(fizz_buzz_elem_link.providers.pop().unwrap()),
                even_odd_elem,
                default_channel_size,
                2,
            );

        let even_odd_drain = even_odd_elem_link.consumer;

        let (s1, odd_collector_output) = crossbeam_channel::unbounded();
        let odd_collector =
            ExhaustiveCollector::new(0, Box::new(even_odd_elem_link.providers.pop().unwrap()), s1);

        let (s0, even_collector_output) = crossbeam_channel::unbounded();
        let even_collector =
            ExhaustiveCollector::new(0, Box::new(even_odd_elem_link.providers.pop().unwrap()), s0);

        tokio::run(lazy(|| {
            tokio::spawn(fizz_buzz_drain);
            tokio::spawn(even_odd_drain);
            tokio::spawn(even_collector);
            tokio::spawn(odd_collector);
            Ok(())
        }));

        let actual_evens = even_collector_output.iter().collect::<Vec<i32>>();
        assert_eq!(actual_evens, vec![2, 4, 8, 14, 16, 22, 26, 28]);

        let actual_odds = odd_collector_output.iter().collect::<Vec<i32>>();
        assert_eq!(actual_odds, vec![1, 7, 11, 13, 17, 19, 23, 29]);
    }

    #[test]
    fn one_even_odd_wait_between_packets() {
        let default_channel_size = 10;
        let number_branches = 2;
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = PacketIntervalGenerator::new(
            time::Duration::from_millis(10),
            packets.clone().into_iter(),
        );

        let elem0 = ClassifyEvenOddElement { id: 0 };

        let mut elem0_link = ClassifyElementLink::new(
            Box::new(packet_generator),
            elem0,
            default_channel_size,
            number_branches,
        );

        let elem0_drain = elem0_link.consumer;

        let (s1, elem0_port1_collector_output) = crossbeam_channel::unbounded();
        let elem0_port1_collector =
            ExhaustiveCollector::new(0, Box::new(elem0_link.providers.pop().unwrap()), s1);

        let (s0, elem0_port0_collector_output) = crossbeam_channel::unbounded();
        let elem0_port0_collector =
            ExhaustiveCollector::new(0, Box::new(elem0_link.providers.pop().unwrap()), s0);

        tokio::run(lazy(|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem0_port0_collector);
            tokio::spawn(elem0_port1_collector);
            Ok(())
        }));

        let elem0_port0_output: Vec<_> = elem0_port0_collector_output.iter().collect();
        assert_eq!(elem0_port0_output, vec![0, 2, 420, 4, 6, 8]);

        let elem0_port1_output: Vec<_> = elem0_port1_collector_output.iter().collect();
        assert_eq!(elem0_port1_output, vec![1, 1337, 3, 5, 7, 9]);
    }
}
