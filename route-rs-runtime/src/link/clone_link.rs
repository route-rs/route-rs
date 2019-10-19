use crate::link::task_park::*;
use crate::link::{Link, LinkBuilder, PacketStream, QueueEgressor};
use crossbeam::atomic::AtomicCell;
use crossbeam::crossbeam_channel;
use crossbeam::crossbeam_channel::{Receiver, Sender};
use futures::{Async, Future, Poll, Stream};
use std::sync::Arc;

#[derive(Default)]
pub struct CloneLink<Packet: Clone + Send> {
    in_stream: Option<PacketStream<Packet>>,
    queue_capacity: usize,
    num_egressors: Option<usize>,
}

impl<Packet: Clone + Send> CloneLink<Packet> {
    pub fn new() -> Self {
        CloneLink {
            in_stream: None,
            queue_capacity: 10,
            num_egressors: None,
        }
    }

    /// Changes queue_capacity, default value is 10.
    /// Valid range is 1..=1000
    pub fn queue_capacity(self, queue_capacity: usize) -> Self {
        assert!(
            (1..=1000).contains(&queue_capacity),
            format!(
                "queue_capacity: {}, must be in range 1..=1000",
                queue_capacity
            )
        );

        CloneLink {
            in_stream: self.in_stream,
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

        CloneLink {
            in_stream: self.in_stream,
            queue_capacity: self.queue_capacity,
            num_egressors: Some(num_egressors),
        }
    }

    pub fn ingressor(self, in_stream: PacketStream<Packet>) -> Self {
        CloneLink {
            in_stream: Some(in_stream),
            queue_capacity: self.queue_capacity,
            num_egressors: self.num_egressors,
        }
    }
}

impl<Packet: Send + Clone + 'static> LinkBuilder<Packet, Packet> for CloneLink<Packet> {
    fn ingressors(self, mut in_streams: Vec<PacketStream<Packet>>) -> Self {
        assert_eq!(
            in_streams.len(),
            1,
            "Clone links may only take one input stream!"
        );
        CloneLink {
            in_stream: Some(in_streams.remove(0)),
            queue_capacity: self.queue_capacity,
            num_egressors: self.num_egressors,
        }
    }

    fn build_link(self) -> Link<Packet> {
        if self.in_stream.is_none() {
            panic!("Cannot build link! Missing input stream");
        } else if self.num_egressors.is_none() {
            panic!("Cannot build link! Missing number of num_egressors");
        } else {
            let mut to_egressors: Vec<Sender<Option<Packet>>> = Vec::new();
            let mut egressors: Vec<PacketStream<Packet>> = Vec::new();

            let mut from_ingressors: Vec<Receiver<Option<Packet>>> = Vec::new();

            let mut task_parks: Vec<Arc<AtomicCell<TaskParkState>>> = Vec::new();

            for _ in 0..self.num_egressors.unwrap() {
                let (to_egressor, from_ingressor) =
                    crossbeam_channel::bounded::<Option<Packet>>(self.queue_capacity);
                let task_park = Arc::new(AtomicCell::new(TaskParkState::Empty));

                let egressor = QueueEgressor::new(from_ingressor.clone(), Arc::clone(&task_park));

                to_egressors.push(to_egressor);
                egressors.push(Box::new(egressor));
                from_ingressors.push(from_ingressor);
                task_parks.push(task_park);
            }

            let ingressor = CloneIngressor::new(self.in_stream.unwrap(), to_egressors, task_parks);

            (vec![Box::new(ingressor)], egressors)
        }
    }
}

pub struct CloneIngressor<P> {
    input_stream: PacketStream<P>,
    to_egressors: Vec<Sender<Option<P>>>,
    task_parks: Vec<Arc<AtomicCell<TaskParkState>>>,
}

impl<P> CloneIngressor<P> {
    fn new(
        input_stream: PacketStream<P>,
        to_egressors: Vec<Sender<Option<P>>>,
        task_parks: Vec<Arc<AtomicCell<TaskParkState>>>,
    ) -> Self {
        CloneIngressor {
            input_stream,
            to_egressors,
            task_parks,
        }
    }
}

impl<P> Drop for CloneIngressor<P> {
    fn drop(&mut self) {
        //TODO: do this with a closure or something, this could be a one-liner
        for to_egressor in self.to_egressors.iter() {
            if let Err(err) = to_egressor.try_send(None) {
                panic!("Ingressor: Drop: try_send to egressor, fail?: {:?}", err);
            }
        }
        for task_park in self.task_parks.iter() {
            die_and_notify(&task_park);
        }
    }
}

impl<P: Send + Clone> Future for CloneIngressor<P> {
    type Item = ();
    type Error = ();

    /// If any of the channels are full, we await that channel to clear before processing a new packet.
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            for (port, to_egressor) in self.to_egressors.iter().enumerate() {
                if to_egressor.is_full() {
                    park_and_notify(&self.task_parks[port]);
                    return Ok(Async::NotReady);
                }
            }
            let packet_option: Option<P> = try_ready!(self.input_stream.poll());

            match packet_option {
                None => return Ok(Async::Ready(())),
                Some(packet) => {
                    //TODO: should packet but put in an iterator? or only cloned? or last one reused?
                    assert!(self.to_egressors.len() == self.task_parks.len());
                    for port in 0..self.to_egressors.len() {
                        if let Err(err) = self.to_egressors[port].try_send(Some(packet.clone())) {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::link::TokioRunnable;
    use crate::utils::test::packet_collectors::ExhaustiveCollector;
    use crate::utils::test::packet_generators::immediate_stream;
    use crossbeam::crossbeam_channel;

    use futures::future::lazy;

    fn run_tokio(runnables: Vec<TokioRunnable>) {
        tokio::run(lazy(|| {
            for runnable in runnables {
                tokio::spawn(runnable);
            }
            Ok(())
        }));
    }

    #[test]
    #[should_panic]
    fn panics_when_built_without_input_streams() {
        CloneLink::<i32>::new().num_egressors(10).build_link();
    }

    #[test]
    #[should_panic]
    fn panics_when_built_without_num_egressors() {
        let packets: Vec<i32> = vec![];
        let packet_generator: PacketStream<i32> = immediate_stream(packets.clone());

        CloneLink::<i32>::new()
            .ingressors(vec![Box::new(packet_generator)])
            .build_link();
    }

    #[test]
    fn builder_methods_work_in_any_order() {
        let packets: Vec<i32> = vec![];

        let packet_generator0 = immediate_stream(packets.clone());

        CloneLink::new()
            .ingressor(packet_generator0)
            .num_egressors(2)
            .build_link();

        let packet_generator1 = immediate_stream(packets.clone());

        CloneLink::new()
            .num_egressors(2)
            .ingressor(packet_generator1)
            .build_link();
    }

    #[test]
    fn bringup_teardown() {
        let queue_size = 5;
        let number_num_egressors = 1;
        let packet_generator: PacketStream<i32> = immediate_stream(vec![]);

        let (mut runnables, mut egressors) = CloneLink::new()
            .num_egressors(number_num_egressors)
            .queue_capacity(queue_size)
            .ingressor(packet_generator)
            .build_link();

        let (s0, collector_output) = crossbeam_channel::unbounded();
        let collector = ExhaustiveCollector::new(0, Box::new(egressors.remove(0)), s0);

        runnables.push(Box::new(collector));

        run_tokio(runnables);

        let output: Vec<_> = collector_output.iter().collect();
        assert!(output.is_empty());
    }

    #[test]
    fn one_way() {
        let queue_size = 5;
        let number_num_egressors = 1;
        let packet_generator = immediate_stream(vec![1, 1337, 3, 5, 7, 9]);

        let (mut runnables, mut egressors) = CloneLink::new()
            .num_egressors(number_num_egressors)
            .queue_capacity(queue_size)
            .ingressor(packet_generator)
            .build_link();

        let (s0, collector_output) = crossbeam_channel::unbounded();
        let collector = ExhaustiveCollector::new(0, Box::new(egressors.remove(0)), s0);

        runnables.push(Box::new(collector));

        run_tokio(runnables);

        let output: Vec<_> = collector_output.iter().collect();
        assert_eq!(output, vec![1, 1337, 3, 5, 7, 9]);
    }

    #[test]
    fn two_way() {
        let queue_size = 5;
        let number_num_egressors = 2;
        let packet_generator = immediate_stream(vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9]);

        let (mut runnables, mut egressors) = CloneLink::new()
            .num_egressors(number_num_egressors)
            .queue_capacity(queue_size)
            .ingressor(packet_generator)
            .build_link();

        let (s1, collector1_output) = crossbeam_channel::unbounded();
        let collector1 = ExhaustiveCollector::new(0, Box::new(egressors.pop().unwrap()), s1);
        runnables.push(Box::new(collector1));

        let (s0, collector0_output) = crossbeam_channel::unbounded();
        let collector0 = ExhaustiveCollector::new(0, Box::new(egressors.pop().unwrap()), s0);
        runnables.push(Box::new(collector0));

        run_tokio(runnables);

        let output0: Vec<_> = collector0_output.iter().collect();
        assert_eq!(output0, vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9]);

        let output1: Vec<_> = collector1_output.iter().collect();
        assert_eq!(output1, vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9]);
    }

    #[test]
    fn three_way() {
        let queue_size = 5;
        let number_num_egressors = 3;
        let packet_generator = immediate_stream(vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9]);

        let (mut runnables, mut egressors) = CloneLink::new()
            .num_egressors(number_num_egressors)
            .queue_capacity(queue_size)
            .ingressor(packet_generator)
            .build_link();

        let (s2, collector2_output) = crossbeam_channel::unbounded();
        let collector2 = ExhaustiveCollector::new(0, Box::new(egressors.pop().unwrap()), s2);
        runnables.push(Box::new(collector2));

        let (s1, collector1_output) = crossbeam_channel::unbounded();
        let collector1 = ExhaustiveCollector::new(0, Box::new(egressors.pop().unwrap()), s1);
        runnables.push(Box::new(collector1));

        let (s0, collector0_output) = crossbeam_channel::unbounded();
        let collector0 = ExhaustiveCollector::new(0, Box::new(egressors.pop().unwrap()), s0);
        runnables.push(Box::new(collector0));

        run_tokio(runnables);

        let output0: Vec<_> = collector0_output.iter().collect();
        assert_eq!(output0, vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9]);

        let output1: Vec<_> = collector1_output.iter().collect();
        assert_eq!(output1, vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9]);

        let output2: Vec<_> = collector2_output.iter().collect();
        assert_eq!(output2, vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9]);
    }
}
