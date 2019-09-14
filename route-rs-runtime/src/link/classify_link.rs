use crate::element::Classifier;
use crate::link::task_park::*;
use crate::link::{AsyncEgressor, PacketStream};
use crossbeam::atomic::AtomicCell;
use crossbeam::crossbeam_channel;
use crossbeam::crossbeam_channel::{Receiver, Sender};
use futures::{Async, Future, Poll, Stream};
use std::sync::Arc;

pub struct ClassifyLink<'a, C: Classifier> {
    pub ingressor: ClassifyIngressor<'a, C>,
    pub egressors: Vec<AsyncEgressor<C::Packet>>,
}

impl<'a, C: Classifier> ClassifyLink<'a, C> {
    pub fn new(
        input_stream: PacketStream<C::Packet>,
        classifier: C,
        dispatcher: Box<dyn Fn(C::Class) -> usize + Send + Sync + 'a>,
        queue_capacity: usize,
        branches: usize,
    ) -> Self {
        assert!(
            branches <= 1000,
            format!("ClassifyLink branches: {} > 1000", branches)
        );
        assert!(
            queue_capacity <= 1000,
            format!("ClassifyLink queue_capacity: {} > 1000", queue_capacity)
        );
        assert_ne!(queue_capacity, 0, "queue capacity must be non-zero");

        let mut to_egressors: Vec<Sender<Option<C::Packet>>> = Vec::new();
        let mut egressors: Vec<AsyncEgressor<C::Packet>> = Vec::new();

        let mut from_ingressors: Vec<Receiver<Option<C::Packet>>> = Vec::new();

        let mut task_parks: Vec<Arc<AtomicCell<TaskParkState>>> = Vec::new();

        for _ in 0..branches {
            let (to_egressor, from_ingressor) =
                crossbeam_channel::bounded::<Option<C::Packet>>(queue_capacity);
            let task_park = Arc::new(AtomicCell::new(TaskParkState::Empty));

            let provider = AsyncEgressor::new(from_ingressor.clone(), Arc::clone(&task_park));

            to_egressors.push(to_egressor);
            egressors.push(provider);
            from_ingressors.push(from_ingressor);
            task_parks.push(task_park);
        }

        ClassifyLink {
            ingressor: ClassifyIngressor::new(
                input_stream,
                dispatcher,
                to_egressors,
                classifier,
                task_parks,
            ),
            egressors,
        }
    }
}

pub struct ClassifyIngressor<'a, C: Classifier> {
    input_stream: PacketStream<C::Packet>,
    dispatcher: Box<dyn Fn(C::Class) -> usize + Send + Sync + 'a>,
    to_egressors: Vec<Sender<Option<C::Packet>>>,
    classifier: C,
    task_parks: Vec<Arc<AtomicCell<TaskParkState>>>,
}

impl<'a, C: Classifier> ClassifyIngressor<'a, C> {
    fn new(
        input_stream: PacketStream<C::Packet>,
        dispatcher: Box<dyn Fn(C::Class) -> usize + Send + Sync + 'a>,
        to_egressors: Vec<Sender<Option<C::Packet>>>,
        classifier: C,
        task_parks: Vec<Arc<AtomicCell<TaskParkState>>>,
    ) -> Self {
        ClassifyIngressor {
            input_stream,
            dispatcher,
            to_egressors,
            classifier,
            task_parks,
        }
    }
}

impl<'a, C: Classifier> Drop for ClassifyIngressor<'a, C> {
    fn drop(&mut self) {
        //TODO: do this with a closure or something, this could be a one-liner
        for to_egressor in self.to_egressors.iter() {
            to_egressor
                .try_send(None)
                .expect("ClassifyIngressor::Drop: try_send to_egressor shouldn't fail");
        }
        for task_park in self.task_parks.iter() {
            die_and_notify(&task_park);
        }
    }
}

impl<'a, C: Classifier> Future for ClassifyIngressor<'a, C> {
    type Item = ();
    type Error = ();

    /// Same logic as AsyncEgressor, except if any of the channels are full we
    /// await that channel to clear before processing a new packet. This is somewhat
    /// inefficient, but seems acceptable for now since we want to yield compute to
    /// that egressor, as there is a backup in its queue.
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            for (port, to_egressor) in self.to_egressors.iter().enumerate() {
                if to_egressor.is_full() {
                    park_and_notify(&self.task_parks[port]);
                    return Ok(Async::NotReady);
                }
            }
            let packet_option: Option<C::Packet> = try_ready!(self.input_stream.poll());

            match packet_option {
                None => return Ok(Async::Ready(())),
                Some(packet) => {
                    let class = self.classifier.classify(&packet);
                    let port = (self.dispatcher)(class);
                    if port >= self.to_egressors.len() {
                        panic!("Tried to access invalid port: {}", port);
                    }
                    if let Err(err) = self.to_egressors[port].try_send(Some(packet)) {
                        panic!(
                            "Error in to_egressors[{}] sender, have nowhere to put packet: {:?}",
                            port, err
                        );
                    }
                    unpark_and_notify(&self.task_parks[port]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test::packet_collectors::ExhaustiveCollector;
    use crate::utils::test::packet_generators::{immediate_stream, PacketIntervalGenerator};
    use core::time;
    use futures::future::lazy;

    struct ClassifyEvenness {}

    impl ClassifyEvenness {
        pub fn new() -> Self {
            ClassifyEvenness {}
        }
    }

    impl Classifier for ClassifyEvenness {
        type Packet = i32;
        type Class = bool;

        fn classify(&self, packet: &Self::Packet) -> Self::Class {
            packet % 2 == 0
        }
    }

    #[test]
    fn even_odd() {
        let default_channel_size = 10;
        let number_branches = 2;
        let packet_generator = immediate_stream(vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9]);

        let elem0 = ClassifyEvenness::new();

        let mut elem0_link = ClassifyLink::new(
            packet_generator,
            elem0,
            Box::new(|evenness| if evenness { 0 } else { 1 }),
            default_channel_size,
            number_branches,
        );
        let (s1, elem0_port1_collector_output) = crossbeam_channel::unbounded();

        let elem0_port1_collector =
            ExhaustiveCollector::new(0, Box::new(elem0_link.egressors.pop().unwrap()), s1);

        let (s0, elem0_port0_collector_output) = crossbeam_channel::unbounded();

        let elem0_port0_collector =
            ExhaustiveCollector::new(0, Box::new(elem0_link.egressors.pop().unwrap()), s0);

        tokio::run(lazy(|| {
            tokio::spawn(elem0_link.ingressor);
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
    fn only_odd() {
        let default_channel_size = 5;
        let number_branches = 2;
        let packet_generator = immediate_stream(vec![1, 1337, 3, 5, 7, 9]);

        let elem0 = ClassifyEvenness::new();

        let mut elem0_link = ClassifyLink::new(
            Box::new(packet_generator),
            elem0,
            Box::new(|evenness| if evenness { 0 } else { 1 }),
            default_channel_size,
            number_branches,
        );
        let elem0_drain = elem0_link.ingressor;

        let (s1, elem0_port1_collector_output) = crossbeam_channel::unbounded();
        let elem0_port1_collector =
            ExhaustiveCollector::new(0, Box::new(elem0_link.egressors.pop().unwrap()), s1);

        let (s0, elem0_port0_collector_output) = crossbeam_channel::unbounded();
        let elem0_port0_collector =
            ExhaustiveCollector::new(0, Box::new(elem0_link.egressors.pop().unwrap()), s0);

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
    fn even_odd_long_stream() {
        let default_channel_size = 10;
        let number_branches = 2;
        let packet_generator = immediate_stream(0..2000);

        let even_odd_elem = ClassifyEvenness::new();

        let mut even_odd_link = ClassifyLink::new(
            Box::new(packet_generator),
            even_odd_elem,
            Box::new(|evenness| if evenness { 0 } else { 1 }),
            default_channel_size,
            number_branches,
        );
        let even_odd_drain = even_odd_link.ingressor;

        let (s1, odd_collector_output) = crossbeam_channel::unbounded();
        let odd_collector =
            ExhaustiveCollector::new(0, Box::new(even_odd_link.egressors.pop().unwrap()), s1);

        let (s0, even_collector_output) = crossbeam_channel::unbounded();
        let even_collector =
            ExhaustiveCollector::new(0, Box::new(even_odd_link.egressors.pop().unwrap()), s0);

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

    enum FizzBuzz {
        FizzBuzz,
        Fizz,
        Buzz,
        None,
    }

    struct ClassifyFizzBuzz {}

    impl ClassifyFizzBuzz {
        pub fn new() -> Self {
            ClassifyFizzBuzz {}
        }
    }

    impl Classifier for ClassifyFizzBuzz {
        type Packet = i32;
        type Class = FizzBuzz;

        fn classify(&self, packet: &Self::Packet) -> Self::Class {
            if packet % 3 == 0 && packet % 5 == 0 {
                FizzBuzz::FizzBuzz
            } else if packet % 3 == 0 {
                FizzBuzz::Fizz
            } else if packet % 5 == 0 {
                FizzBuzz::Buzz
            } else {
                FizzBuzz::None
            }
        }
    }

    #[test]
    fn fizz_buzz() {
        let default_channel_size = 10;
        let packet_generator = immediate_stream(0..=30);

        let elem = ClassifyFizzBuzz::new();

        let mut elem_link = ClassifyLink::new(
            Box::new(packet_generator),
            elem,
            Box::new(|fb| match fb {
                FizzBuzz::FizzBuzz => 0,
                FizzBuzz::Fizz => 1,
                FizzBuzz::Buzz => 2,
                FizzBuzz::None => 3,
            }),
            default_channel_size,
            4,
        );
        let elem_drain = elem_link.ingressor;

        let (s3, other_output) = crossbeam_channel::unbounded();
        let port3_collector =
            ExhaustiveCollector::new(0, Box::new(elem_link.egressors.pop().unwrap()), s3);

        let (s2, buzz_output) = crossbeam_channel::unbounded();
        let port2_collector =
            ExhaustiveCollector::new(0, Box::new(elem_link.egressors.pop().unwrap()), s2);

        let (s1, fizz_output) = crossbeam_channel::unbounded();
        let port1_collector =
            ExhaustiveCollector::new(0, Box::new(elem_link.egressors.pop().unwrap()), s1);

        let (s0, fizz_buzz_output) = crossbeam_channel::unbounded();
        let port0_collector =
            ExhaustiveCollector::new(0, Box::new(elem_link.egressors.pop().unwrap()), s0);

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

        let fizz_buzz_elem = ClassifyFizzBuzz::new();

        let mut fizz_buzz_elem_link: ClassifyLink<ClassifyFizzBuzz> = ClassifyLink::new(
            Box::new(packet_generator),
            fizz_buzz_elem,
            Box::new(|fb| match fb {
                FizzBuzz::FizzBuzz => 0,
                FizzBuzz::Fizz => 1,
                FizzBuzz::Buzz => 2,
                FizzBuzz::None => 3,
            }),
            default_channel_size,
            4,
        );
        let fizz_buzz_drain = fizz_buzz_elem_link.ingressor;

        let even_odd_elem = ClassifyEvenness::new();

        let mut even_odd_elem_link: ClassifyLink<ClassifyEvenness> = ClassifyLink::new(
            Box::new(fizz_buzz_elem_link.egressors.pop().unwrap()),
            even_odd_elem,
            Box::new(|evenness| if evenness { 0 } else { 1 }),
            default_channel_size,
            2,
        );

        let even_odd_drain = even_odd_elem_link.ingressor;

        let (s1, odd_collector_output) = crossbeam_channel::unbounded();
        let odd_collector =
            ExhaustiveCollector::new(0, Box::new(even_odd_elem_link.egressors.pop().unwrap()), s1);

        let (s0, even_collector_output) = crossbeam_channel::unbounded();
        let even_collector =
            ExhaustiveCollector::new(0, Box::new(even_odd_elem_link.egressors.pop().unwrap()), s0);

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
    fn even_odd_wait_between_packets() {
        let default_channel_size = 10;
        let number_branches = 2;
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = PacketIntervalGenerator::new(
            time::Duration::from_millis(10),
            packets.clone().into_iter(),
        );

        let elem0 = ClassifyEvenness::new();

        let mut elem0_link = ClassifyLink::new(
            Box::new(packet_generator),
            elem0,
            Box::new(|evenness| if evenness { 0 } else { 1 }),
            default_channel_size,
            number_branches,
        );

        let elem0_drain = elem0_link.ingressor;

        let (s1, elem0_port1_collector_output) = crossbeam_channel::unbounded();
        let elem0_port1_collector =
            ExhaustiveCollector::new(0, Box::new(elem0_link.egressors.pop().unwrap()), s1);

        let (s0, elem0_port0_collector_output) = crossbeam_channel::unbounded();
        let elem0_port0_collector =
            ExhaustiveCollector::new(0, Box::new(elem0_link.egressors.pop().unwrap()), s0);

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
    #[should_panic(expected = "queue capacity must be non-zero")]
    fn empty_channel() {
        let default_channel_size = 0;
        let number_branches = 2;
        let packet_generator = immediate_stream(vec![]);

        let elem0 = ClassifyEvenness::new();

        let mut _elem0_link = ClassifyLink::new(
            Box::new(packet_generator),
            elem0,
            Box::new(|evenness| if evenness { 0 } else { 1 }),
            default_channel_size,
            number_branches,
        );
    }
}
