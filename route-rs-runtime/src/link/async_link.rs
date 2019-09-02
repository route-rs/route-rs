use crate::element::AsyncElement;
use crate::link::task_park::*;
use crate::link::PacketStream;
use crossbeam::atomic::AtomicCell;
use crossbeam::crossbeam_channel;
use crossbeam::crossbeam_channel::{Receiver, Sender, TryRecvError};
use futures::{Async, Future, Poll, Stream};
use std::sync::Arc;

/// The AsyncLink is a wrapper to create and contain both sides of the
/// link, the Ingressor, which intakes and processes packets, and the Egressor,
/// which provides an interface where the next element retrieves the output
/// packet.
pub struct AsyncLink<E: AsyncElement> {
    pub ingressor: AsyncIngressor<E>,
    pub egressor: AsyncEgressor<E::Output>,
}

impl<E: AsyncElement> AsyncLink<E> {
    pub fn new(input_stream: PacketStream<E::Input>, element: E, queue_capacity: usize) -> Self {
        assert!(
            queue_capacity <= 1000,
            format!("Async Element queue_capacity: {} > 1000", queue_capacity)
        );
        assert_ne!(queue_capacity, 0, "queue capacity must be non-zero");

        let (to_egressor, from_ingressor) =
            crossbeam_channel::bounded::<Option<E::Output>>(queue_capacity);
        let task_park: Arc<AtomicCell<TaskParkState>> =
            Arc::new(AtomicCell::new(TaskParkState::Empty));

        AsyncLink {
            ingressor: AsyncIngressor::new(
                input_stream,
                to_egressor,
                element,
                Arc::clone(&task_park),
            ),

            egressor: AsyncEgressor::new(from_ingressor.clone(), task_park),
        }
    }
}

/// The AsyncIngressor is responsible for polling its input stream,
/// processing them using the `element`s process function, and pushing the
/// output packet onto the to_egressor queue. It does work in batches, so it
/// will continue to pull packets as long as it can make forward progess,
/// after which it will return NotReady to sleep. This is handed to, and is
/// polled by the runtime.
pub struct AsyncIngressor<E: AsyncElement> {
    input_stream: PacketStream<E::Input>,
    to_egressor: Sender<Option<E::Output>>,
    element: E,
    task_park: Arc<AtomicCell<TaskParkState>>,
}

impl<E: AsyncElement> AsyncIngressor<E> {
    fn new(
        input_stream: PacketStream<E::Input>,
        to_egressor: Sender<Option<E::Output>>,
        element: E,
        task_park: Arc<AtomicCell<TaskParkState>>,
    ) -> Self {
        AsyncIngressor {
            input_stream,
            to_egressor,
            element,
            task_park,
        }
    }
}

/// Special Drop for AsyncIngressor
///
/// When we are dropping the Ingressor, we want to send a message to the
/// Egressor so that it may drop as well. Additionally, we awaken the
/// Egressor, since an asleep Egressor counts on the Ingressor to awaken
/// it. This prevents a deadlock during Ingressor drops, or unexpected
/// teardown. We also place a `TaskParkState::Dead` in the task_park
/// so that the Egressor knows it can not rely on the Ingressor to awaken
/// it in the future.
impl<E: AsyncElement> Drop for AsyncIngressor<E> {
    fn drop(&mut self) {
        self.to_egressor
            .try_send(None)
            .expect("AsyncIngressor::Drop: try_send to_egressor shouldn't fail");
        die_and_notify(&self.task_park);
    }
}

impl<E: AsyncElement> Future for AsyncIngressor<E> {
    type Item = ();
    type Error = ();

    /// Implement Poll for Future for AsyncIngressor
    ///
    /// Note that this function works a bit different, it continues to process
    /// packets off it's input queue until it reaches a point where it can not
    /// make forward progress. There are three cases:
    /// ###
    /// #1 The to_egressor queue is full, we notify the Egressor that we need
    /// awaking when there is work to do, and go to sleep.
    ///
    /// #2 The input_stream returns a NotReady, we sleep, with the assumption
    /// that whomever produced the NotReady will awaken the task in the Future.
    ///
    /// #3 We get a Ready(None), in which case we push a None onto the to_Egressor
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
            let input_packet_option: Option<E::Input> = try_ready!(self.input_stream.poll());

            match input_packet_option {
                None => return Ok(Async::Ready(())),
                Some(input_packet) => {
                    let output_packet: E::Output = self.element.process(input_packet);
                    self.to_egressor
                        .try_send(Some(output_packet))
                        .expect("AsyncIngressor::Poll: try_send to_egressor shouldn't fail");
                    unpark_and_notify(&self.task_park);
                }
            }
        }
    }
}

/// The Egressor side of the AsyncLink is responsible to converting the
/// output queue of processed packets, which is a crossbeam channel, to a
/// Stream that can be polled for packets. It ends up being owned by the
/// element which is polling for packets.
pub struct AsyncEgressor<Packet: Sized> {
    from_ingressor: Receiver<Option<Packet>>,
    task_park: Arc<AtomicCell<TaskParkState>>,
}

impl<Packet: Sized> AsyncEgressor<Packet> {
    fn new(
        from_ingressor: Receiver<Option<Packet>>,
        task_park: Arc<AtomicCell<TaskParkState>>,
    ) -> Self {
        AsyncEgressor {
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
impl<Packet: Sized> Drop for AsyncEgressor<Packet> {
    fn drop(&mut self) {
        die_and_notify(&self.task_park);
    }
}

impl<Packet: Sized> Stream for AsyncEgressor<Packet> {
    type Item = Packet;
    type Error = ();

    /// Implement Poll for Stream for AsyncEgressor
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
    use crate::element::{AsyncIdentityElement, IdentityElement, TransformElement};
    use crate::link::sync_link::SyncLink;
    use crate::utils::test::packet_collectors::ExhaustiveCollector;
    use crate::utils::test::packet_generators::{immediate_stream, PacketIntervalGenerator};
    use core::time;
    use futures::future::lazy;

    #[test]
    fn one_async_element() {
        let default_channel_size = 10;
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = immediate_stream(packets.clone());

        let elem0 = AsyncIdentityElement::new();

        let elem0_link = AsyncLink::new(Box::new(packet_generator), elem0, default_channel_size);

        let (s, r) = crossbeam_channel::unbounded();
        let elem0_drain = elem0_link.ingressor;
        let elem0_collector = ExhaustiveCollector::new(0, Box::new(elem0_link.egressor), s);

        tokio::run(lazy(|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem0_collector);
            Ok(())
        }));

        let router_output: Vec<_> = r.iter().collect();
        assert_eq!(router_output, packets);
    }

    #[test]
    fn one_async_element_long_stream() {
        let default_channel_size = 10;
        let packet_generator = immediate_stream(0..2000);

        let elem0 = AsyncIdentityElement::new();

        let elem0_link = AsyncLink::new(Box::new(packet_generator), elem0, default_channel_size);

        let (s, r) = crossbeam_channel::unbounded();
        let elem0_drain = elem0_link.ingressor;
        let elem0_collector = ExhaustiveCollector::new(0, Box::new(elem0_link.egressor), s);

        tokio::run(lazy(|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem0_collector);
            Ok(())
        }));

        let router_output: Vec<_> = r.iter().collect();
        assert_eq!(router_output.len(), 2000);
    }

    #[test]
    #[should_panic(expected = "queue capacity must be non-zero")]
    fn one_async_element_empty_channel() {
        let default_channel_size = 0;
        let packets: Vec<i32> = vec![];
        let packet_generator = immediate_stream(packets);

        let elem0 = AsyncIdentityElement::new();

        let _elem0_link = AsyncLink::new(Box::new(packet_generator), elem0, default_channel_size);
    }

    #[test]
    fn one_async_element_small_channel() {
        let default_channel_size = 1;
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = immediate_stream(packets.clone());

        let elem0 = AsyncIdentityElement::new();

        let elem0_link = AsyncLink::new(Box::new(packet_generator), elem0, default_channel_size);

        let (s, r) = crossbeam_channel::unbounded();
        let elem0_drain = elem0_link.ingressor;
        let elem0_collector = ExhaustiveCollector::new(0, Box::new(elem0_link.egressor), s);

        tokio::run(lazy(|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem0_collector);
            Ok(())
        }));

        let router_output: Vec<_> = r.iter().collect();
        assert_eq!(router_output, packets);
    }

    #[test]
    fn one_async_element_empty_stream() {
        let default_channel_size = 10;
        let packets: Vec<i32> = vec![];
        let packet_generator = immediate_stream(packets.clone());

        let elem0 = AsyncIdentityElement::new();

        let elem0_link = AsyncLink::new(Box::new(packet_generator), elem0, default_channel_size);

        let (s, r) = crossbeam_channel::unbounded();
        let elem0_drain = elem0_link.ingressor;
        let elem0_collector = ExhaustiveCollector::new(0, Box::new(elem0_link.egressor), s);

        tokio::run(lazy(|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem0_collector);
            Ok(())
        }));

        let router_output: Vec<_> = r.iter().collect();
        assert_eq!(router_output, packets);
    }

    #[test]
    fn two_async_elements() {
        let default_channel_size = 10;
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = immediate_stream(packets.clone());

        let elem0 = AsyncIdentityElement::new();
        let elem1 = AsyncIdentityElement::new();

        let elem0_link = AsyncLink::new(Box::new(packet_generator), elem0, default_channel_size);
        let elem1_link = AsyncLink::new(Box::new(elem0_link.egressor), elem1, default_channel_size);

        let elem0_drain = elem0_link.ingressor;
        let elem1_drain = elem1_link.ingressor;

        let (s, r) = crossbeam_channel::unbounded();
        let elem1_collector = ExhaustiveCollector::new(0, Box::new(elem1_link.egressor), s);

        tokio::run(lazy(|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem1_drain);
            tokio::spawn(elem1_collector);
            Ok(())
        }));

        let router_output: Vec<_> = r.iter().collect();
        assert_eq!(router_output, packets);
    }

    #[test]
    fn series_sync_and_async() {
        let default_channel_size = 10;
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = immediate_stream(packets.clone());

        let elem0 = IdentityElement::new();
        let elem1 = AsyncIdentityElement::new();
        let elem2 = IdentityElement::new();
        let elem3 = AsyncIdentityElement::new();

        let elem0_link = SyncLink::new(Box::new(packet_generator), elem0);
        let elem1_link = AsyncLink::new(Box::new(elem0_link), elem1, default_channel_size);
        let elem2_link = SyncLink::new(Box::new(elem1_link.egressor), elem2);
        let elem3_link = AsyncLink::new(Box::new(elem2_link), elem3, default_channel_size);

        let elem1_drain = elem1_link.ingressor;
        let elem3_drain = elem3_link.ingressor;

        let (s, r) = crossbeam_channel::unbounded();
        let elem3_collector = ExhaustiveCollector::new(0, Box::new(elem3_link.egressor), s);

        tokio::run(lazy(|| {
            tokio::spawn(elem1_drain);
            tokio::spawn(elem3_drain);
            tokio::spawn(elem3_collector);
            Ok(())
        }));

        let router_output: Vec<_> = r.iter().collect();
        assert_eq!(router_output, packets);
    }

    #[test]
    fn one_async_element_wait_between_packets() {
        let default_channel_size = 10;
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = PacketIntervalGenerator::new(
            time::Duration::from_millis(10),
            packets.clone().into_iter(),
        );

        let elem0 = AsyncIdentityElement::new();

        let elem0_link = AsyncLink::new(Box::new(packet_generator), elem0, default_channel_size);

        let (s, r) = crossbeam_channel::unbounded();
        let elem0_drain = elem0_link.ingressor;
        let elem0_collector = ExhaustiveCollector::new(0, Box::new(elem0_link.egressor), s);

        tokio::run(lazy(|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem0_collector);
            Ok(())
        }));

        let router_output: Vec<_> = r.iter().collect();
        assert_eq!(router_output, packets);
    }

    #[test]
    fn one_async_transform_element() {
        let default_channel_size = 10;
        let packets = "route-rs".chars();
        let packet_generator = immediate_stream(packets.clone());

        let elem0 = TransformElement::new();

        let elem0_link = AsyncLink::new(Box::new(packet_generator), elem0, default_channel_size);

        let (s, r) = crossbeam_channel::unbounded();
        let elem0_drain = elem0_link.ingressor;
        let elem0_collector = ExhaustiveCollector::new(0, Box::new(elem0_link.egressor), s);

        tokio::run(lazy(|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem0_collector);
            Ok(())
        }));

        let router_output: Vec<u32> = r.iter().collect();
        let expected_output: Vec<u32> = packets.map(|p| p.into()).collect();
        assert_eq!(router_output, expected_output);
    }
}
