use crate::element::Classifier;
use crate::link::task_park::*;
use crate::link::{Link, LinkBuilder, PacketStream, QueueEgressor};
use crossbeam::atomic::AtomicCell;
use crossbeam::crossbeam_channel;
use crossbeam::crossbeam_channel::{Receiver, Sender};
use futures::{Async, Future, Poll, Stream};
use std::sync::Arc;

#[derive(Default)]
pub struct ClassifyLink<C: Classifier> {
    in_stream: Option<PacketStream<C::Packet>>,
    classifier: Option<C>,
    dispatcher: Option<Box<dyn Fn(C::Class) -> usize + Send + Sync + 'static>>,
    queue_capacity: usize,
    num_egressors: Option<usize>,
}

impl<C: Classifier> ClassifyLink<C> {
    pub fn new() -> Self {
        ClassifyLink {
            in_stream: None,
            classifier: None,
            dispatcher: None,
            queue_capacity: 10,
            num_egressors: None,
        }
    }

    pub fn classifier(self, classifier: C) -> Self {
        ClassifyLink {
            in_stream: self.in_stream,
            classifier: Some(classifier),
            dispatcher: self.dispatcher,
            queue_capacity: self.queue_capacity,
            num_egressors: self.num_egressors,
        }
    }

    pub fn dispatcher(
        self,
        dispatcher: Box<dyn Fn(C::Class) -> usize + Send + Sync + 'static>,
    ) -> Self {
        ClassifyLink {
            in_stream: self.in_stream,
            classifier: self.classifier,
            dispatcher: Some(dispatcher),
            queue_capacity: self.queue_capacity,
            num_egressors: self.num_egressors,
        }
    }

    pub fn queue_capacity(self, queue_capacity: usize) -> Self {
        assert!(
            (1..=1000).contains(&queue_capacity),
            format!(
                "Queue capacity: {}, must be in range 1..=1000",
                queue_capacity
            )
        );
        ClassifyLink {
            in_stream: self.in_stream,
            classifier: self.classifier,
            dispatcher: self.dispatcher,
            queue_capacity,
            num_egressors: self.num_egressors,
        }
    }

    pub fn num_egressors(self, num_egressors: usize) -> Self {
        assert!(
            (1..=1000).contains(&num_egressors),
            format!(
                "num_egressors: {}, must be in range 1..=1000",
                num_egressors
            )
        );
        ClassifyLink {
            in_stream: self.in_stream,
            classifier: self.classifier,
            dispatcher: self.dispatcher,
            queue_capacity: self.queue_capacity,
            num_egressors: Some(num_egressors),
        }
    }

    pub fn ingressor(self, in_stream: PacketStream<C::Packet>) -> Self {
        ClassifyLink {
            in_stream: Some(in_stream),
            classifier: self.classifier,
            dispatcher: self.dispatcher,
            queue_capacity: self.queue_capacity,
            num_egressors: self.num_egressors,
        }
    }
}

impl<C: Classifier + Send + 'static> LinkBuilder<C::Packet, C::Packet> for ClassifyLink<C> {
    fn ingressors(self, mut in_streams: Vec<PacketStream<C::Packet>>) -> Self {
        assert_eq!(
            in_streams.len(),
            1,
            "ClassifyLink may only take 1 input stream"
        );

        ClassifyLink {
            in_stream: Some(in_streams.remove(0)),
            classifier: self.classifier,
            dispatcher: self.dispatcher,
            queue_capacity: self.queue_capacity,
            num_egressors: self.num_egressors,
        }
    }

    fn build_link(self) -> Link<C::Packet> {
        if self.in_stream.is_none() {
            panic!("Cannot build link! Missing input streams");
        } else if self.classifier.is_none() {
            panic!("Cannot build link! Missing classifier");
        } else if self.dispatcher.is_none() {
            panic!("Cannot build link! Missing dispatcher");
        } else if self.num_egressors.is_none() {
            panic!("Cannot build link! Missing num_egressors");
        } else {
            let mut to_egressors: Vec<Sender<Option<C::Packet>>> = Vec::new();
            let mut egressors: Vec<PacketStream<C::Packet>> = Vec::new();

            let mut from_ingressors: Vec<Receiver<Option<C::Packet>>> = Vec::new();

            let mut task_parks: Vec<Arc<AtomicCell<TaskParkState>>> = Vec::new();

            for _ in 0..self.num_egressors.unwrap() {
                let (to_egressor, from_ingressor) =
                    crossbeam_channel::bounded::<Option<C::Packet>>(self.queue_capacity);
                let task_park = Arc::new(AtomicCell::new(TaskParkState::Empty));

                let provider = QueueEgressor::new(from_ingressor.clone(), Arc::clone(&task_park));

                to_egressors.push(to_egressor);
                egressors.push(Box::new(provider));
                from_ingressors.push(from_ingressor);
                task_parks.push(task_park);
            }
            let ingressor = ClassifyIngressor::new(
                self.in_stream.unwrap(),
                self.dispatcher.unwrap(),
                to_egressors,
                self.classifier.unwrap(),
                task_parks,
            );
            (vec![Box::new(ingressor)], egressors)
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

    /// Same logic as QueueEgressor, except if any of the channels are full we
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
    use crate::utils::test::harness::run_link;
    use crate::utils::test::packet_generators::{immediate_stream, PacketIntervalGenerator};
    use core::time;

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
    #[should_panic]
    fn panics_when_built_without_input_streams() {
        ClassifyLink::new()
            .num_egressors(10)
            .classifier(ClassifyEvenness::new())
            .dispatcher(Box::new(|evenness| if evenness { 0 } else { 1 }))
            .build_link();
    }

    #[test]
    #[should_panic]
    fn panics_when_built_without_branches() {
        let packets: Vec<i32> = vec![];
        let packet_generator: PacketStream<i32> = immediate_stream(packets.clone());

        ClassifyLink::new()
            .ingressor(packet_generator)
            .classifier(ClassifyEvenness::new())
            .dispatcher(Box::new(|evenness| if evenness { 0 } else { 1 }))
            .build_link();
    }

    #[test]
    #[should_panic]
    fn panics_when_built_without_classifier() {
        let packets: Vec<i32> = vec![];
        let packet_generator: PacketStream<i32> = immediate_stream(packets.clone());

        ClassifyLink::<ClassifyEvenness>::new()
            .ingressor(packet_generator)
            .num_egressors(10)
            .dispatcher(Box::new(|evenness| if evenness { 0 } else { 1 }))
            .build_link();
    }

    #[test]
    #[should_panic]
    fn panics_when_built_without_dispatcher() {
        let packets: Vec<i32> = vec![];
        let packet_generator: PacketStream<i32> = immediate_stream(packets.clone());

        ClassifyLink::new()
            .ingressor(packet_generator)
            .classifier(ClassifyEvenness::new())
            .build_link();
    }

    fn even_classifier(stream: PacketStream<i32>) -> Link<i32> {
        ClassifyLink::new()
            .ingressor(stream)
            .num_egressors(2)
            .classifier(ClassifyEvenness::new())
            .dispatcher(Box::new(|evenness| if evenness { 0 } else { 1 }))
            .build_link()
    }

    #[test]
    fn even_odd() {
        let packet_generator = immediate_stream(vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9]);

        let results = run_link(even_classifier(packet_generator));
        assert_eq!(results[0], vec![0, 2, 420, 4, 6, 8]);
        assert_eq!(results[1], vec![1, 1337, 3, 5, 7, 9]);
    }

    #[test]
    fn even_odd_wait_between_packets() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = PacketIntervalGenerator::new(
            time::Duration::from_millis(10),
            packets.clone().into_iter(),
        );

        let results = run_link(even_classifier(Box::new(packet_generator)));
        assert_eq!(results[0], vec![0, 2, 420, 4, 6, 8]);
        assert_eq!(results[1], vec![1, 1337, 3, 5, 7, 9]);
    }

    #[test]
    fn only_odd() {
        let packet_generator = immediate_stream(vec![1, 1337, 3, 5, 7, 9]);

        let results = run_link(even_classifier(packet_generator));
        assert_eq!(results[0], []);
        assert_eq!(results[1], vec![1, 1337, 3, 5, 7, 9]);
    }

    #[test]
    fn even_odd_long_stream() {
        let packet_generator = immediate_stream(0..2000);

        let results = run_link(even_classifier(packet_generator));
        assert_eq!(results[0].len(), 1000);
        assert_eq!(results[1].len(), 1000);
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

    fn fizz_buzz_classifier(stream: PacketStream<i32>) -> Link<i32> {
        ClassifyLink::new()
            .ingressor(stream)
            .num_egressors(4)
            .classifier(ClassifyFizzBuzz::new())
            .dispatcher(Box::new(|fb| match fb {
                FizzBuzz::FizzBuzz => 0,
                FizzBuzz::Fizz => 1,
                FizzBuzz::Buzz => 2,
                FizzBuzz::None => 3,
            }))
            .build_link()
    }

    #[test]
    fn fizz_buzz() {
        let packet_generator = immediate_stream(0..=30);

        let results = run_link(fizz_buzz_classifier(packet_generator));

        let expected_fizz_buzz = vec![0, 15, 30];
        assert_eq!(results[0], expected_fizz_buzz);

        let expected_fizz = vec![3, 6, 9, 12, 18, 21, 24, 27];
        assert_eq!(results[1], expected_fizz);

        let expected_buzz = vec![5, 10, 20, 25];
        assert_eq!(results[2], expected_buzz);

        let expected_other = vec![1, 2, 4, 7, 8, 11, 13, 14, 16, 17, 19, 22, 23, 26, 28, 29];
        assert_eq!(results[3], expected_other);
    }

    #[test]
    fn fizz_buzz_to_even_odd() {
        let packet_generator = immediate_stream(0..=30);

        let (mut fb_runnables, mut fb_egressors) = fizz_buzz_classifier(packet_generator);

        let (mut eo_runnables, eo_egressors) = even_classifier(fb_egressors.pop().unwrap());

        fb_runnables.append(&mut eo_runnables);

        let link = (fb_runnables, eo_egressors);
        let results = run_link(link);
        assert_eq!(results[0], vec![2, 4, 8, 14, 16, 22, 26, 28]);
        assert_eq!(results[1], vec![1, 7, 11, 13, 17, 19, 23, 29]);
    }
}
