use crate::element::Element;
use crate::link::task_park::*;
use crate::link::{ElementLinkBuilder, Link, LinkBuilder, PacketStream};
use crossbeam::atomic::AtomicCell;
use crossbeam::crossbeam_channel;
use crossbeam::crossbeam_channel::{Receiver, Sender, TryRecvError};
use futures::{Async, Future, Poll, Stream};
use std::sync::Arc;

#[derive(Default)]
pub struct QueueLink<E: Element> {
    in_stream: Option<PacketStream<E::Input>>,
    element: Option<E>,
    queue_capacity: usize,
}

impl<E: Element> QueueLink<E> {
    pub fn new() -> Self {
        QueueLink {
            in_stream: None,
            element: None,
            queue_capacity: 10,
        }
    }

    pub fn ingressor(self, in_stream: PacketStream<E::Input>) -> Self {
        QueueLink {
            in_stream: Some(in_stream),
            element: self.element,
            queue_capacity: self.queue_capacity,
        }
    }

    /// Changes queue_capacity, default value is 10.
    /// Valid range is 1..=1000
    pub fn queue_capacity(self, queue_capacity: usize) -> Self {
        assert!(
            queue_capacity <= 1000,
            format!("Async Element queue_capacity: {} > 1000", queue_capacity)
        );
        assert_ne!(queue_capacity, 0, "queue capacity must be non-zero");

        QueueLink {
            in_stream: self.in_stream,
            element: self.element,
            queue_capacity,
        }
    }
}

impl<E: Element + Send + 'static> LinkBuilder<E::Input, E::Output> for QueueLink<E> {
    fn ingressors(self, mut in_streams: Vec<PacketStream<E::Input>>) -> Self {
        assert_eq!(
            in_streams.len(),
            1,
            "AsyncLink may only take 1 input stream"
        );

        QueueLink {
            in_stream: Some(in_streams.remove(0)),
            element: self.element,
            queue_capacity: self.queue_capacity,
        }
    }

    fn build_link(self) -> Link<E::Output> {
        if self.in_stream.is_none() {
            panic!("Cannot build link! Missing input stream");
        } else if self.element.is_none() {
            panic!("Cannot build link! Missing element");
        } else {
            let (to_egressor, from_ingressor) =
                crossbeam_channel::bounded::<Option<E::Output>>(self.queue_capacity);
            let task_park: Arc<AtomicCell<TaskParkState>> =
                Arc::new(AtomicCell::new(TaskParkState::Empty));

            let ingresssor = QueueIngressor::new(
                self.in_stream.unwrap(),
                to_egressor,
                self.element.unwrap(),
                Arc::clone(&task_park),
            );
            let egressor = QueueEgressor::new(from_ingressor, task_park);

            (vec![Box::new(ingresssor)], vec![Box::new(egressor)])
        }
    }
}

impl<E: Element + Send + 'static> ElementLinkBuilder<E> for QueueLink<E> {
    fn element(self, element: E) -> Self {
        QueueLink {
            in_stream: self.in_stream,
            element: Some(element),
            queue_capacity: self.queue_capacity,
        }
    }
}

/// The QueueIngressor is responsible for polling its input stream,
/// processing them using the `element`s process function, and pushing the
/// output packet onto the to_egressor queue. It does work in batches, so it
/// will continue to pull packets as long as it can make forward progess,
/// after which it will return NotReady to sleep. This is handed to, and is
/// polled by the runtime.
pub struct QueueIngressor<E: Element> {
    input_stream: PacketStream<E::Input>,
    to_egressor: Sender<Option<E::Output>>,
    element: E,
    task_park: Arc<AtomicCell<TaskParkState>>,
}

impl<E: Element> QueueIngressor<E> {
    fn new(
        input_stream: PacketStream<E::Input>,
        to_egressor: Sender<Option<E::Output>>,
        element: E,
        task_park: Arc<AtomicCell<TaskParkState>>,
    ) -> Self {
        QueueIngressor {
            input_stream,
            to_egressor,
            element,
            task_park,
        }
    }
}

/// Special Drop for QueueIngressor
///
/// When we are dropping the Ingressor, we want to send a message to the
/// Egressor so that it may drop as well. Additionally, we awaken the
/// Egressor, since an asleep Egressor counts on the Ingressor to awaken
/// it. This prevents a deadlock during Ingressor drops, or unexpected
/// teardown. We also place a `TaskParkState::Dead` in the task_park
/// so that the Egressor knows it can not rely on the Ingressor to awaken
/// it in the future.
impl<E: Element> Drop for QueueIngressor<E> {
    fn drop(&mut self) {
        self.to_egressor
            .try_send(None)
            .expect("QueueIngressor::Drop: try_send to_egressor shouldn't fail");
        die_and_notify(&self.task_park);
    }
}

impl<E: Element> Future for QueueIngressor<E> {
    type Item = ();
    type Error = ();

    /// Implement Poll for Future for QueueIngressor
    ///
    /// This function continues to process
    /// packets off it's input queue until it reaches a point where it can not
    /// make forward progress. There are several cases:
    /// ###
    /// #1 The to_egressor queue is full, we notify the Egressor that we need
    /// awaking when there is work to do, and go to sleep by returning `Async::NotReady`.
    ///
    /// #2 The input_stream returns a NotReady, we sleep, with the assumption
    /// that whomever produced the NotReady will awaken the task in the Future.
    ///
    /// #3 We get a Ready(None), in which case we push a None onto the to_Egressor
    /// queue and then return Ready(()), which means we enter tear-down, since there
    /// is no further work to complete.
    ///
    /// #4 If our upstream `PacketStream` has a packet for us, we pass it to our `element`
    /// for `process`ing. Most of the time, it will yield a `Some(output_packet)` that has
    /// been transformed in some way. We pass that on to our egress channel and notify
    /// our `Egressor` that it has work to do, and continue polling our upstream `PacketStream`.
    ///
    /// #5 `element`s may also choose to "drop" packets by returning `None`, so we do nothing
    /// and poll our upstream `PacketStream` again.
    ///
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            if self.to_egressor.is_full() {
                park_and_notify(&self.task_park);
                return Ok(Async::NotReady);
            }
            let input_packet_option: Option<E::Input> = try_ready!(self.input_stream.poll());

            match input_packet_option {
                None => return Ok(Async::Ready(())),
                Some(input_packet) => {
                    if let Some(output_packet) = self.element.process(input_packet) {
                        self.to_egressor
                            .try_send(Some(output_packet))
                            .expect("QueueIngressor::Poll: try_send to_egressor shouldn't fail");
                        unpark_and_notify(&self.task_park);
                    }
                }
            }
        }
    }
}

/// The Egressor side of the QueueLink is responsible to converting the
/// output queue of processed packets, which is a crossbeam channel, to a
/// Stream that can be polled for packets. It ends up being owned by the
/// element which is polling for packets.
pub struct QueueEgressor<Packet: Sized> {
    from_ingressor: Receiver<Option<Packet>>,
    task_park: Arc<AtomicCell<TaskParkState>>,
}

impl<Packet: Sized> QueueEgressor<Packet> {
    pub fn new(
        from_ingressor: Receiver<Option<Packet>>,
        task_park: Arc<AtomicCell<TaskParkState>>,
    ) -> Self {
        QueueEgressor {
            from_ingressor,
            task_park,
        }
    }
}

/// Special Drop for Egressor
///
/// This Drop notifies the Ingressor that is can no longer rely on the Egressor
/// to awaken it. It is not expected behavior that the Egressor dies while the
/// Ingressor is still alive. But it may happen in edge cases and we want to
/// ensure that a deadlock does not occur.
impl<Packet: Sized> Drop for QueueEgressor<Packet> {
    fn drop(&mut self) {
        die_and_notify(&self.task_park);
    }
}

impl<Packet: Sized> Stream for QueueEgressor<Packet> {
    type Item = Packet;
    type Error = ();

    /// Implement Poll for Stream for QueueEgressor
    ///
    /// This function, tries to retrieve a packet off the `from_ingressor`
    /// channel, there are four cases:
    /// ###
    /// #1 Ok(Some(Packet)): Got a packet. If the Ingressor needs (likely due to
    /// an until now full channel) to be awoken, wake them. Return the Async::Ready(Option(Packet))
    ///
    /// #2 Ok(None): this means that the Ingressor is in tear-down, and we
    /// will no longer be receivig packets. Return Async::Ready(None) to forward propagate teardown
    ///
    /// #3 Err(TryRecvError::Empty): Packet queue is empty, await the Ingressor to awaken us with more
    /// work, by returning Async::NotReady to signal to runtime to sleep this task.
    ///
    /// #4 Err(TryRecvError::Disconnected): Ingressor is in teardown and has dropped its side of the
    /// from_ingressor channel; we will no longer receive packets. Return Async::Ready(None) to forward
    /// propagate teardown.
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
    use crate::element::{DropElement, IdentityElement, TransformElement};
    use crate::link::sync_link::SyncLinkBuilder;
    use crate::link::{ElementLinkBuilder, LinkBuilder, TokioRunnable};
    use crate::utils::test::packet_collectors::ExhaustiveCollector;
    use crate::utils::test::packet_generators::{immediate_stream, PacketIntervalGenerator};
    use core::time;
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
        let identity_element: IdentityElement<i32> = IdentityElement::new();

        QueueLink::new().element(identity_element).build_link();
    }

    #[test]
    #[should_panic]
    fn panics_when_built_without_element() {
        let packets: Vec<i32> = vec![];
        let packet_generator: PacketStream<i32> = immediate_stream(packets.clone());

        QueueLink::<IdentityElement<i32>>::new()
            .ingressor(packet_generator)
            .build_link();
    }

    #[test]
    fn builder_methods_work_in_any_order() {
        let packets: Vec<i32> = vec![];

        let packet_generator0 = immediate_stream(packets.clone());
        let identity_element0 = IdentityElement::new();

        QueueLink::new()
            .ingressor(packet_generator0)
            .element(identity_element0)
            .build_link();

        let packet_generator1 = immediate_stream(packets.clone());
        let identity_element1 = IdentityElement::new();

        QueueLink::new()
            .element(identity_element1)
            .ingressor(packet_generator1)
            .build_link();
    }

    #[test]
    fn queue_link_works() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = immediate_stream(packets.clone());

        let elem = IdentityElement::new();

        let (mut runnables, mut egressors) = QueueLink::new()
            .ingressor(packet_generator)
            .element(elem)
            .build_link();

        let (s, r) = crossbeam_channel::unbounded();
        let collector = ExhaustiveCollector::new(0, Box::new(egressors.remove(0)), s);

        runnables.push(Box::new(collector));

        run_tokio(runnables);

        let output: Vec<_> = r.iter().collect();
        assert_eq!(output, packets);
    }

    #[test]
    fn long_stream() {
        let packet_generator = immediate_stream(0..2000);

        let elem = IdentityElement::new();

        let (mut runnables, mut egressors) = QueueLink::new()
            .ingressor(packet_generator)
            .element(elem)
            .build_link();

        let (s, r) = crossbeam_channel::unbounded();
        let collector = ExhaustiveCollector::new(0, Box::new(egressors.remove(0)), s);

        runnables.push(Box::new(collector));

        run_tokio(runnables);

        let output: Vec<_> = r.iter().collect();
        assert_eq!(output.len(), 2000);
    }

    #[test]
    #[should_panic(expected = "queue capacity must be non-zero")]
    fn empty_channel() {
        let queue_size = 0;
        let packets: Vec<i32> = vec![];
        let packet_generator = immediate_stream(packets);

        let elem = IdentityElement::new();

        let (_, _) = QueueLink::new()
            .ingressor(packet_generator)
            .element(elem)
            .queue_capacity(queue_size)
            .build_link();
    }

    #[test]
    fn small_channel() {
        let queue_size = 1;
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = immediate_stream(packets.clone());

        let elem = IdentityElement::new();

        let (mut runnables, mut egressors) = QueueLink::new()
            .ingressor(packet_generator)
            .element(elem)
            .queue_capacity(queue_size)
            .build_link();

        let (s, r) = crossbeam_channel::unbounded();
        let collector = ExhaustiveCollector::new(0, Box::new(egressors.remove(0)), s);

        runnables.push(Box::new(collector));

        run_tokio(runnables);

        let output: Vec<_> = r.iter().collect();
        assert_eq!(output, packets);
    }

    #[test]
    fn empty_stream() {
        let packets: Vec<i32> = vec![];
        let packet_generator = immediate_stream(packets.clone());

        let elem = IdentityElement::new();

        let (mut runnables, mut egressors) = QueueLink::new()
            .ingressor(packet_generator)
            .element(elem)
            .build_link();

        let (s, r) = crossbeam_channel::unbounded();
        let collector = ExhaustiveCollector::new(0, Box::new(egressors.remove(0)), s);

        runnables.push(Box::new(collector));

        run_tokio(runnables);

        let output: Vec<_> = r.iter().collect();
        assert_eq!(output, packets);
    }

    #[test]
    fn two_links() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = immediate_stream(packets.clone());

        let elem0 = IdentityElement::new();
        let elem1 = IdentityElement::new();

        let (runnables0, mut egressors0) = QueueLink::new()
            .ingressor(packet_generator)
            .element(elem0)
            .build_link();

        let (mut runnables1, mut egressors1) = QueueLink::new()
            .ingressor(egressors0.remove(0))
            .element(elem1)
            .build_link();

        let (s, r) = crossbeam_channel::unbounded();
        let collector = ExhaustiveCollector::new(0, Box::new(egressors1.remove(0)), s);

        runnables1.extend(runnables0);
        runnables1.push(Box::new(collector));

        run_tokio(runnables1);

        let output: Vec<_> = r.iter().collect();
        assert_eq!(output, packets);
    }

    #[test]
    fn series_of_sync_and_queue_links() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = immediate_stream(packets.clone());

        let elem0 = IdentityElement::new();
        let elem1 = IdentityElement::new();
        let elem2 = IdentityElement::new();
        let elem3 = IdentityElement::new();

        let (_, mut egressors0) = SyncLinkBuilder::new()
            .ingressor(packet_generator)
            .element(elem0)
            .build_link();

        let (runnables1, mut egressors1) = QueueLink::new()
            .ingressor(egressors0.remove(0))
            .element(elem1)
            .build_link();

        let (_, mut egressors2) = SyncLinkBuilder::new()
            .ingressor(egressors1.remove(0))
            .element(elem2)
            .build_link();

        let (mut runnables3, mut egressors3) = QueueLink::new()
            .ingressor(egressors2.remove(0))
            .element(elem3)
            .build_link();

        let (s, r) = crossbeam_channel::unbounded();
        let collector = ExhaustiveCollector::new(0, Box::new(egressors3.remove(0)), s);

        runnables3.extend(runnables1);
        runnables3.push(Box::new(collector));

        run_tokio(runnables3);

        let output: Vec<_> = r.iter().collect();
        assert_eq!(output, packets);
    }

    #[test]
    fn wait_between_packets() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = PacketIntervalGenerator::new(
            time::Duration::from_millis(10),
            packets.clone().into_iter(),
        );

        let elem = IdentityElement::new();

        let (mut runnables, mut egressors) = QueueLink::new()
            .ingressor(Box::new(packet_generator))
            .element(elem)
            .build_link();

        let (s, r) = crossbeam_channel::unbounded();
        let collector = ExhaustiveCollector::new(0, Box::new(egressors.remove(0)), s);

        runnables.push(Box::new(collector));

        run_tokio(runnables);

        let output: Vec<_> = r.iter().collect();
        assert_eq!(output, packets);
    }

    #[test]
    fn transform_element() {
        let packets = "route-rs".chars();
        let packet_generator = immediate_stream(packets.clone());

        let elem = TransformElement::new();

        let (mut runnables, mut egressors) = QueueLink::new()
            .ingressor(packet_generator)
            .element(elem)
            .build_link();

        let (s, r) = crossbeam_channel::unbounded();
        let collector = ExhaustiveCollector::new(0, Box::new(egressors.remove(0)), s);

        runnables.push(Box::new(collector));

        run_tokio(runnables);

        let output: Vec<u32> = r.iter().collect();
        let expected: Vec<u32> = packets.map(|p| p.into()).collect();
        assert_eq!(output, expected);
    }

    #[test]
    fn drop_element() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = immediate_stream(packets.clone());

        let elem = DropElement::new();

        let (mut runnables, mut egressors) = QueueLink::new()
            .ingressor(packet_generator)
            .element(elem)
            .build_link();

        let (s, r) = crossbeam_channel::unbounded();
        let collector = ExhaustiveCollector::new(0, Box::new(egressors.remove(0)), s);

        runnables.push(Box::new(collector));

        run_tokio(runnables);

        let router_output: Vec<u32> = r.iter().collect();
        assert_eq!(router_output, []);
    }
}
