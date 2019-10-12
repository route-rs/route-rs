use crate::link::task_park::*;
use crate::link::{Link, LinkBuilder, PacketStream, TokioRunnable};
use crossbeam::atomic::AtomicCell;
use crossbeam::crossbeam_channel;
use crossbeam::crossbeam_channel::{Receiver, Sender};
use futures::{task, Async, Future, Poll, Stream};
use std::sync::Arc;

#[derive(Default)]
pub struct JoinLink<Packet: Sized + Send> {
    in_streams: Option<Vec<PacketStream<Packet>>>,
    queue_capacity: usize,
}

impl<Packet: Sized + Send> JoinLink<Packet> {
    pub fn new() -> Self {
        JoinLink {
            in_streams: None,
            queue_capacity: 10,
        }
    }

    /// Changes queue_capacity, default value is 10.
    /// Valid range is 1..=1000
    pub fn queue_capacity(self, queue_capacity: usize) -> Self {
        assert!(
            queue_capacity <= 1000,
            format!("queue_capacity: {} > 1000", queue_capacity)
        );
        assert_ne!(queue_capacity, 0, "queue capacity must be non-zero");

        JoinLink {
            in_streams: self.in_streams,
            queue_capacity,
        }
    }
}

impl<Packet: Sized + Send + 'static> LinkBuilder<Packet, Packet> for JoinLink<Packet> {
    fn ingressors(self, in_streams: Vec<PacketStream<Packet>>) -> Self {
        assert_ne!(in_streams.len(), 0, "Input stream vector can not be zero!");
        JoinLink {
            in_streams: Some(in_streams),
            queue_capacity: self.queue_capacity,
        }
    }

    fn build_link(self) -> Link<Packet> {
        if self.in_streams.is_none() {
            panic!("Cannot build link! Missing input streams");
        } else {
            let input_streams = self.in_streams.unwrap();
            let number_ingressors = input_streams.len();
            let mut ingressors: Vec<TokioRunnable> = Vec::new();
            let mut from_ingressors: Vec<Receiver<Option<Packet>>> = Vec::new();
            let mut task_parks: Vec<Arc<AtomicCell<TaskParkState>>> = Vec::new();

            for input_stream in input_streams {
                let (to_egressor, from_ingressor) =
                    crossbeam_channel::bounded::<Option<Packet>>(self.queue_capacity);
                let task_park = Arc::new(AtomicCell::new(TaskParkState::Empty));

                let ingressor =
                    JoinIngressor::new(input_stream, to_egressor, Arc::clone(&task_park));
                ingressors.push(Box::new(ingressor));
                from_ingressors.push(from_ingressor);
                task_parks.push(task_park);
            }

            let egressor = JoinEgressor::new(from_ingressors, task_parks, number_ingressors);

            (ingressors, vec![Box::new(egressor)])
        }
    }
}

pub struct JoinIngressor<Packet: Sized> {
    input_stream: PacketStream<Packet>,
    to_egressor: Sender<Option<Packet>>,
    task_park: Arc<AtomicCell<TaskParkState>>,
}

impl<Packet: Sized> JoinIngressor<Packet> {
    fn new(
        input_stream: PacketStream<Packet>,
        to_egressor: Sender<Option<Packet>>,
        task_park: Arc<AtomicCell<TaskParkState>>,
    ) -> Self {
        JoinIngressor {
            input_stream,
            to_egressor,
            task_park,
        }
    }
}

impl<Packet: Sized> Drop for JoinIngressor<Packet> {
    fn drop(&mut self) {
        self.to_egressor
            .try_send(None)
            .expect("JoinIngressor::Drop: try_send to_egressor shouldn't fail");
        die_and_notify(&self.task_park);
    }
}

impl<Packet: Sized> Future for JoinIngressor<Packet> {
    type Item = ();
    type Error = ();

    /// Implement Poll for Future for JoinIngressor
    ///
    /// Note that this function works a bit different, it continues to process
    /// packets off it's input queue until it reaches a point where it can not
    /// make forward progress. There are three cases:
    /// ###
    /// #1 The to_egressor queue is full, we notify the egressor that we need
    /// awaking when there is work to do, and go to sleep.
    ///
    /// #2 The input_stream returns a NotReady, we sleep, with the assumption
    /// that whomever produced the NotReady will awaken the task in the Future.
    ///
    /// #3 We get a Ready(None), in which case we push a None onto the to_egressor
    /// queue and then return Ready(()), which means we enter tear-down, since there
    /// is no futher work to complete.
    /// ###
    /// By Sleep, we mean we return a NotReady to the runtime which will sleep the task.
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            if self.to_egressor.is_full() {
                park_and_notify(&self.task_park);
                return Ok(Async::NotReady);
            }
            let input_packet_option: Option<Packet> = try_ready!(self.input_stream.poll());

            match input_packet_option {
                None => return Ok(Async::Ready(())),
                Some(packet) => {
                    self.to_egressor
                        .try_send(Some(packet))
                        .expect("JoinIngressor::Poll: try_send to_egressor shouldn't fail");
                    unpark_and_notify(&self.task_park);
                }
            }
        }
    }
}

#[allow(dead_code)]
pub struct JoinEgressor<Packet: Sized> {
    from_ingressors: Vec<Receiver<Option<Packet>>>,
    task_parks: Vec<Arc<AtomicCell<TaskParkState>>>,
    ingressors_alive: usize,
    next_pull_ingressor: usize,
}

impl<Packet: Sized> JoinEgressor<Packet> {
    fn new(
        from_ingressors: Vec<Receiver<Option<Packet>>>,
        task_parks: Vec<Arc<AtomicCell<TaskParkState>>>,
        ingressors_alive: usize,
    ) -> Self {
        let next_pull_ingressor = 0;
        JoinEgressor {
            from_ingressors,
            task_parks,
            ingressors_alive,
            next_pull_ingressor,
        }
    }
}

impl<Packet: Sized> Drop for JoinEgressor<Packet> {
    fn drop(&mut self) {
        for task_park in self.task_parks.iter() {
            die_and_notify(&task_park);
        }
    }
}

impl<Packet: Sized> Stream for JoinEgressor<Packet> {
    type Item = Packet;
    type Error = ();

    /// Iterate over all the channels, pull the first packet that is available.
    /// This starts at the next index after the last successful recv
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        //rotate_slice exists in 1.22 nightly experimental
        let rotated_iter = self
            .from_ingressors
            .iter()
            .enumerate()
            .cycle()
            .skip(self.next_pull_ingressor)
            .take(self.from_ingressors.len());
        for (port, from_ingressor) in rotated_iter {
            match from_ingressor.try_recv() {
                Ok(Some(packet)) => {
                    unpark_and_notify(&self.task_parks[port]);
                    self.next_pull_ingressor = port + 1;
                    return Ok(Async::Ready(Some(packet)));
                }
                Ok(None) => {
                    //Got a none from a consumer that has shutdown
                    self.ingressors_alive -= 1;
                    if self.ingressors_alive == 0 {
                        return Ok(Async::Ready(None));
                    }
                }
                Err(_) => {
                    //On an error go to next channel.
                }
            }
        }

        // We could not get a packet from any of our ingressors, this means we will park our task in a
        // common location, and then hand out Arcs to all the ingressors to the common location. The first
        // one to access the egressor task will awaken us, so we can continue providing packets.
        let mut parked_egressor_task = false;
        let egressor_task = Arc::new(AtomicCell::new(Some(task::current())));
        for task_park in self.task_parks.iter() {
            if indirect_park_and_notify(&task_park, Arc::clone(&egressor_task)) {
                parked_egressor_task = true;
            }
        }
        //we were unable to park task, so we must self notify, presumably all the ingressors are dead.
        if !parked_egressor_task {
            task::current().notify();
        }
        Ok(Async::NotReady)
    }
}

#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use super::*;
    use crate::link::{LinkBuilder, TokioRunnable};
    use crate::utils::test::packet_collectors::ExhaustiveCollector;
    use crate::utils::test::packet_generators::{immediate_stream, PacketIntervalGenerator};
    use core::time;
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
        JoinLink::<i32>::new().build_link();
    }

    #[test]
    #[should_panic]
    fn panics_when_input_streams_is_empty() {
        let input_streams = Vec::new();
        JoinLink::<i32>::new()
            .ingressors(input_streams)
            .build_link();
    }

    #[test]
    fn builder_methods_work_in_any_order() {
        let packets: Vec<i32> = vec![];

        let packet_generator0 = immediate_stream(packets.clone());

        JoinLink::new()
            .ingressors(vec![Box::new(packet_generator0)])
            .queue_capacity(4)
            .build_link();

        let packet_generator1 = immediate_stream(packets.clone());

        JoinLink::new()
            .queue_capacity(4)
            .ingressors(vec![Box::new(packet_generator1)])
            .build_link();
    }

    #[test]
    fn join_link() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9, 11];
        let packet_generator0 = immediate_stream(packets.clone());
        let packet_generator1 = immediate_stream(packets.clone());

        let mut input_streams: Vec<PacketStream<usize>> = Vec::new();
        input_streams.push(Box::new(packet_generator0));
        input_streams.push(Box::new(packet_generator1));

        let (mut runnables, mut egressors) = JoinLink::new()
            .ingressors(input_streams)
            .build_link();

        let (s, collector_output) = crossbeam_channel::unbounded();
        let collector = ExhaustiveCollector::new(0, Box::new(egressors.remove(0)), s);

        runnables.push(Box::new(collector));

        run_tokio(runnables);

        let link_output: Vec<_> = collector_output.iter().collect();
        assert_eq!(link_output.len(), packets.len() * 2);
    }

    #[test]
    fn long_stream() {
        let packet_generator0 = immediate_stream(0..=2000);
        let packet_generator1 = immediate_stream(0..=2000);

        let mut input_streams: Vec<PacketStream<usize>> = Vec::new();
        input_streams.push(Box::new(packet_generator0));
        input_streams.push(Box::new(packet_generator1));

        let (mut runnables, mut egressors) = JoinLink::new()
            .ingressors(input_streams)
            .build_link();

        let (s, collector_output) = crossbeam_channel::unbounded();
        let collector = ExhaustiveCollector::new(0, Box::new(egressors.remove(0)), s);

        runnables.push(Box::new(collector));

        run_tokio(runnables);

        let link_output: Vec<_> = collector_output.iter().collect();
        assert_eq!(link_output.len(), 4002);
    }

    #[test]
    fn five_inputs() {
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

        let (mut runnables, mut egressors) = JoinLink::new()
            .ingressors(input_streams)
            .build_link();

        let (s, collector_output) = crossbeam_channel::unbounded();
        let collector = ExhaustiveCollector::new(0, Box::new(egressors.remove(0)), s);

        runnables.push(Box::new(collector));

        run_tokio(runnables);

        let link_output: Vec<_> = collector_output.iter().collect();
        assert_eq!(link_output.len(), 10005);
    }

    #[test]
    fn wait_between_packets() {
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

        let (mut runnables, mut egressors) = JoinLink::new()
            .ingressors(input_streams)
            .build_link();

        let (s, collector_output) = crossbeam_channel::unbounded();
        let collector = ExhaustiveCollector::new(0, Box::new(egressors.remove(0)), s);

        runnables.push(Box::new(collector));

        run_tokio(runnables);

        let join_output: Vec<_> = collector_output.iter().collect();
        assert_eq!(join_output.len(), packets.len() * 2);
    }

    #[test]
    fn fairness_test() {
        //If fairness changes, may need to update test
        let packets_heavy = vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let packets_light = vec![1, 1, 1, 1];
        let packet_generator0 = immediate_stream(packets_heavy.into_iter());
        let packet_generator1 = immediate_stream(packets_light.into_iter());

        let mut input_streams: Vec<PacketStream<usize>> = Vec::new();
        input_streams.push(Box::new(packet_generator0));
        input_streams.push(Box::new(packet_generator1));

        let (mut runnables, mut egressors) = JoinLink::new()
            .ingressors(input_streams)
            .build_link();

        let (s, collector_output) = crossbeam_channel::unbounded();
        let collector = ExhaustiveCollector::new(0, Box::new(egressors.remove(0)), s);

        runnables.push(Box::new(collector));

        run_tokio(runnables);

        let join0_output: Vec<_> = collector_output.iter().collect();

        // Test early elements contain an even mix of 1s and 0s
        // expect four (all) 1s within first 10 elements
        assert_eq!(join0_output[0..10].iter().sum::<usize>(), 4);
    }

    #[test]
    fn small_channel() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9, 11];
        let packet_generator0 = immediate_stream(packets.clone());
        let packet_generator1 = immediate_stream(packets.clone());

        let mut input_streams: Vec<PacketStream<usize>> = Vec::new();
        input_streams.push(Box::new(packet_generator0));
        input_streams.push(Box::new(packet_generator1));

        let (mut runnables, mut egressors) = JoinLink::new()
            .ingressors(input_streams)
            .build_link();

        let (s, collector_output) = crossbeam_channel::unbounded();
        let collector = ExhaustiveCollector::new(0, Box::new(egressors.remove(0)), s);

        runnables.push(Box::new(collector));

        run_tokio(runnables);

        let join_output: Vec<_> = collector_output.iter().collect();
        assert_eq!(join_output.len(), packets.len() * 2);
    }

    #[test]
    fn empty_stream() {
        let packets = vec![];
        let packet_generator0 = immediate_stream(packets.clone());
        let packet_generator1 = immediate_stream(packets.clone());

        let mut input_streams: Vec<PacketStream<usize>> = Vec::new();
        input_streams.push(Box::new(packet_generator0));
        input_streams.push(Box::new(packet_generator1));

        let (mut runnables, mut egressors) = JoinLink::new()
            .ingressors(input_streams)
            .build_link();

        let (s, collector_output) = crossbeam_channel::unbounded();
        let collector = ExhaustiveCollector::new(0, Box::new(egressors.remove(0)), s);

        runnables.push(Box::new(collector));

        run_tokio(runnables);

        let join_output: Vec<_> = collector_output.iter().collect();
        assert_eq!(join_output.len(), packets.len() * 2);
    }

    #[test]
    #[should_panic(expected = "queue capacity must be non-zero")]
    fn empty_channel() {
        let queue_size = 0;
        let packets = vec![];
        let packet_generator0 = immediate_stream(packets.clone());
        let packet_generator1 = immediate_stream(packets.clone());

        let mut input_streams: Vec<PacketStream<usize>> = Vec::new();
        input_streams.push(Box::new(packet_generator0));
        input_streams.push(Box::new(packet_generator1));

        let (_, _) = JoinLink::new()
            .ingressors(input_streams)
            .queue_capacity(queue_size)
            .build_link();
    }
}
