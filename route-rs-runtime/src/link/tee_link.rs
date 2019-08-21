use crate::link::task_park::*;
use crate::link::PacketStream;
use crossbeam::atomic::AtomicCell;
use crossbeam::crossbeam_channel;
use crossbeam::crossbeam_channel::{Receiver, Sender, TryRecvError};
use futures::{Async, Future, Poll, Stream};
use std::sync::Arc;

pub struct TeeLink<P: Sized + Clone> {
    pub ingressor: TeeIngressor<P>,
    pub egressors: Vec<TeeEgressor<P>>,
}

impl<P: Sized + Clone> TeeLink<P> {
    pub fn new(input_stream: PacketStream<P>, queue_capacity: usize, branches: usize) -> Self {
        assert!(
            branches <= 1000,
            format!("Tee Link branches: {} > 1000", branches)
        );
        assert!(
            queue_capacity <= 1000,
            format!("Tee Link queue_capacity: {} > 1000", queue_capacity)
        );

        let mut to_egressors: Vec<Sender<Option<P>>> = Vec::new();
        let mut egressors: Vec<TeeEgressor<P>> = Vec::new();

        let mut from_ingressors: Vec<Receiver<Option<P>>> = Vec::new();

        let mut task_parks: Vec<Arc<AtomicCell<TaskParkState>>> = Vec::new();

        for _ in 0..branches {
            let (to_egressor, from_ingressor) =
                crossbeam_channel::bounded::<Option<P>>(queue_capacity);
            let task_park = Arc::new(AtomicCell::new(TaskParkState::Empty));

            let egressor = TeeEgressor::new(from_ingressor.clone(), Arc::clone(&task_park));

            to_egressors.push(to_egressor);
            egressors.push(egressor);
            from_ingressors.push(from_ingressor);
            task_parks.push(task_park);
        }

        TeeLink {
            ingressor: TeeIngressor::new(input_stream, to_egressors, task_parks),
            egressors,
        }
    }
}

pub struct TeeIngressor<P> {
    input_stream: PacketStream<P>,
    to_egressors: Vec<Sender<Option<P>>>,
    task_parks: Vec<Arc<AtomicCell<TaskParkState>>>,
}

impl<P> TeeIngressor<P> {
    fn new(
        input_stream: PacketStream<P>,
        to_egressors: Vec<Sender<Option<P>>>,
        task_parks: Vec<Arc<AtomicCell<TaskParkState>>>,
    ) -> Self {
        TeeIngressor {
            input_stream,
            to_egressors,
            task_parks,
        }
    }
}

impl<P> Drop for TeeIngressor<P> {
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

impl<P: Sized + Clone> Future for TeeIngressor<P> {
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

/// Tee Element Egressor, exactly the same as AsyncElementEgressor, but
/// they have different trait bounds. Hence the reimplementaton. Would love
/// a PR that solves this problem.
pub struct TeeEgressor<P> {
    from_ingressor: crossbeam_channel::Receiver<Option<P>>,
    task_park: Arc<AtomicCell<TaskParkState>>,
}

impl<P> TeeEgressor<P> {
    fn new(
        from_ingressor: crossbeam_channel::Receiver<Option<P>>,
        task_park: Arc<AtomicCell<TaskParkState>>,
    ) -> Self {
        TeeEgressor {
            from_ingressor,
            task_park,
        }
    }
}

impl<P> Drop for TeeEgressor<P> {
    fn drop(&mut self) {
        die_and_notify(&self.task_park);
    }
}

impl<P: Sized + Clone> Stream for TeeEgressor<P> {
    type Item = P;
    type Error = ();

    /// Implement Poll for Stream for TeeEgressor
    ///
    /// This function, tries to retrieve a packet off the `from_ingressor`
    /// channel, there are four cases:
    /// ###
    /// #1 Ok(Some(Packet)): Got a packet.
    /// #2 Ok(None): this means that the ingressor is in tear-down
    /// #3 Err(TryRecvError::Empty): Packet queue is empty, await the ingressor to awaken us
    /// #4 Err(TryRecvError::Disconnected): Ingressor is in teardown
    /// ###
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.from_ingressor.try_recv() {
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
    use crate::utils::test::packet_collectors::ExhaustiveCollector;
    use crate::utils::test::packet_generators::immediate_stream;
    use crossbeam::crossbeam_channel;

    use futures::future::lazy;

    #[test]
    fn bringup_teardown() {
        let default_channel_size = 5;
        let number_branches = 1;
        let packet_generator: PacketStream<i32> = immediate_stream(vec![]);

        let mut elem0_link = TeeLink::new(
            Box::new(packet_generator),
            default_channel_size,
            number_branches,
        );
        let elem0_drain = elem0_link.ingressor;

        let (s0, elem0_port0_collector_output) = crossbeam_channel::unbounded();
        let elem0_port0_collector =
            ExhaustiveCollector::new(0, Box::new(elem0_link.egressors.pop().unwrap()), s0);

        tokio::run(lazy(|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem0_port0_collector);
            Ok(())
        }));

        let elem0_port0_output: Vec<_> = elem0_port0_collector_output.iter().collect();
        assert!(elem0_port0_output.is_empty());
    }

    #[test]
    fn single_clone() {
        //TODO: find a way to detect branches all have a ingressor
        let default_channel_size = 5;
        let number_branches = 1;
        let packet_generator = immediate_stream(vec![1, 1337, 3, 5, 7, 9]);

        let mut elem0_link = TeeLink::new(
            Box::new(packet_generator),
            default_channel_size,
            number_branches,
        );
        let elem0_drain = elem0_link.ingressor;

        let (s0, elem0_port0_collector_output) = crossbeam_channel::unbounded();
        let elem0_port0_collector =
            ExhaustiveCollector::new(0, Box::new(elem0_link.egressors.pop().unwrap()), s0);

        tokio::run(lazy(|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem0_port0_collector);
            Ok(())
        }));

        let elem0_port0_output: Vec<_> = elem0_port0_collector_output.iter().collect();
        assert_eq!(elem0_port0_output, vec![1, 1337, 3, 5, 7, 9]);
    }

    #[test]
    fn basic_two_way_duplicate() {
        let default_channel_size = 10;
        let number_branches = 2;
        let packet_generator = immediate_stream(vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9]);

        let mut elem0_link = TeeLink::new(
            Box::new(packet_generator),
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
        assert_eq!(
            elem0_port0_output,
            vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9]
        );

        let elem0_port1_output: Vec<_> = elem0_port1_collector_output.iter().collect();
        assert_eq!(
            elem0_port1_output,
            vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9]
        );
    }

    #[test]
    fn basic_three_way_duplicate() {
        let default_channel_size = 10;
        let number_branches = 3;
        let packet_generator = immediate_stream(vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9]);

        let mut elem0_link = TeeLink::new(
            Box::new(packet_generator),
            default_channel_size,
            number_branches,
        );
        let elem0_drain = elem0_link.ingressor;

        let (s2, elem0_port2_collector_output) = crossbeam_channel::unbounded();
        let elem0_port2_collector =
            ExhaustiveCollector::new(0, Box::new(elem0_link.egressors.pop().unwrap()), s2);

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
            tokio::spawn(elem0_port2_collector);
            Ok(())
        }));

        let elem0_port0_output: Vec<_> = elem0_port0_collector_output.iter().collect();
        assert_eq!(
            elem0_port0_output,
            vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9]
        );

        let elem0_port1_output: Vec<_> = elem0_port1_collector_output.iter().collect();
        assert_eq!(
            elem0_port1_output,
            vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9]
        );

        let elem0_port2_output: Vec<_> = elem0_port2_collector_output.iter().collect();
        assert_eq!(
            elem0_port2_output,
            vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9]
        );
    }
}
