use crate::api::ElementStream;
use crate::api::TaskParkState;
use crossbeam::atomic::AtomicCell;
use crossbeam::crossbeam_channel;
use crossbeam::crossbeam_channel::{Receiver, Sender, TryRecvError};
use futures::{task, Async, Future, Poll, Stream};
use std::sync::Arc;

pub trait AsyncElement {
    type Input: Sized;
    type Output: Sized;

    fn process(&mut self, packet: Self::Input) -> Self::Output;
}

/// The AsyncElementLink is a wrapper to create and contain both sides of the
/// link, the consumer, which intakes and processes packets, and the provider,
/// which provides an interface where the next element retrieves the output
/// packet.
pub struct AsyncElementLink<E: AsyncElement> {
    pub consumer: AsyncElementConsumer<E>,
    pub provider: AsyncElementProvider<E>,
}

impl<E: AsyncElement> AsyncElementLink<E> {
    pub fn new(input_stream: ElementStream<E::Input>, element: E, queue_capacity: usize) -> Self {
        if queue_capacity == 0 {
            panic!("queue capacity must be non-zero")
        }
        let (to_provider, from_consumer) =
            crossbeam_channel::bounded::<Option<E::Output>>(queue_capacity);
        let task_park: Arc<AtomicCell<TaskParkState>> =
            Arc::new(AtomicCell::new(TaskParkState::Empty));

        AsyncElementLink {
            consumer: AsyncElementConsumer::new(
                input_stream,
                to_provider,
                element,
                Arc::clone(&task_park),
            ),

            provider: AsyncElementProvider::new(from_consumer.clone(), task_park),
        }
    }
}

/// The AsyncElementConsumer is responsible for polling its input stream,
/// processing them using the `element`s process function, and pushing the
/// output packet onto the to_provider queue. It does work in batches, so it
/// will continue to pull packets as long as it can make forward progess,
/// after which it will return NotReady to sleep. This is handed to, and is
/// polled by the runtime.
pub struct AsyncElementConsumer<E: AsyncElement> {
    input_stream: ElementStream<E::Input>,
    to_provider: Sender<Option<E::Output>>,
    element: E,
    task_park: Arc<AtomicCell<TaskParkState>>,
}

impl<E: AsyncElement> AsyncElementConsumer<E> {
    fn new(
        input_stream: ElementStream<E::Input>,
        to_provider: Sender<Option<E::Output>>,
        element: E,
        task_park: Arc<AtomicCell<TaskParkState>>,
    ) -> Self {
        AsyncElementConsumer {
            input_stream,
            to_provider,
            element,
            task_park,
        }
    }
}

/// Special Drop for AsyncElementConsumer
///
/// When we are dropping the consumer, we want to send a message to the
/// provider so that it may drop as well. Additionally, we awaken the
/// provider, since an asleep provider counts on the consumer to awaken
/// it. This prevents a deadlock during consumer drops, or unexpected
/// teardown. We also place a `TaskParkState::Dead` in the task_park
/// so that the provider knows it can not rely on the consumer to awaken
/// it in the future.
impl<E: AsyncElement> Drop for AsyncElementConsumer<E> {
    fn drop(&mut self) {
        if let Err(err) = self.to_provider.try_send(None) {
            panic!("Consumer: Drop: try_send to_provider, fail?: {:?}", err);
        }
        if let TaskParkState::Full(provider) = self.task_park.swap(TaskParkState::Dead) {
            provider.notify();
        }
    }
}

impl<E: AsyncElement> Future for AsyncElementConsumer<E> {
    type Item = ();
    type Error = ();

    /// Implement Poll for Future for AsyncElementConsumer
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
                let consumer = task::current();
                match self.task_park.swap(TaskParkState::Full(consumer)) {
                    TaskParkState::Full(provider) => {
                        provider.notify();
                    }
                    TaskParkState::Empty => {}
                    // When TaskParkState is dead, we must self notify, and return the `task_park`
                    // to the `Dead` state. This ensures future calls to poll won't think the
                    // `task_park` is working.
                    TaskParkState::Dead => {
                        self.task_park.store(TaskParkState::Dead);
                        task::current().notify();
                    }
                }
                return Ok(Async::NotReady);
            }
            let input_packet_option: Option<E::Input> = try_ready!(self.input_stream.poll());

            match input_packet_option {
                None => return Ok(Async::Ready(())),
                Some(input_packet) => {
                    let output_packet: E::Output = self.element.process(input_packet);
                    if let Err(err) = self.to_provider.try_send(Some(output_packet)) {
                        panic!(
                            "Error in to_provider sender, have nowhere to put packet: {:?}",
                            err
                        );
                    }
                    if let TaskParkState::Full(provider) = self.task_park.swap(TaskParkState::Empty)
                    {
                        provider.notify();
                    }
                }
            }
        }
    }
}

/// The Provider side of the AsyncElement is responsible to converting the
/// output queue of processed packets, which is a crossbeam channel, to a
/// Stream that can be polled for packets. It ends up being owned by the
/// element which is polling for packets.
pub struct AsyncElementProvider<E: AsyncElement> {
    from_consumer: Receiver<Option<E::Output>>,
    task_park: Arc<AtomicCell<TaskParkState>>,
}

impl<E: AsyncElement> AsyncElementProvider<E> {
    fn new(
        from_consumer: Receiver<Option<E::Output>>,
        task_park: Arc<AtomicCell<TaskParkState>>,
    ) -> Self {
        AsyncElementProvider {
            from_consumer,
            task_park,
        }
    }
}

/// Special Drop for Provider
///
/// This Drop notifies the consumer that is can no longer rely on the provider
/// to awaken it. It is not expected behavior that the provider dies while the
/// consumer is still alive. But it may happen in edge cases and we want to
/// ensure that a deadlock does not occur.
impl<E: AsyncElement> Drop for AsyncElementProvider<E> {
    fn drop(&mut self) {
        if let TaskParkState::Full(consumer) = self.task_park.swap(TaskParkState::Dead) {
            consumer.notify();
        }
    }
}

impl<E: AsyncElement> Stream for AsyncElementProvider<E> {
    type Item = E::Output;
    type Error = ();

    ///Implement Poll for Stream for AsyncElementProvider
    ///
    /// This function, tries to retrieve a packet off the `from_consumer`
    /// channel, there are four cases:
    /// ###
    /// #1 Ok(Some(Packet)): Got a packet.if the consumer needs (likely due to
    /// an until now full channel) to be awoken, wake them. Return the Async::Ready(Option(Packet))
    ///
    /// #2 Ok(None): this means that the consumer is in tear-down, and we
    /// will no longer be receivig packets. Return Async::Ready(None) to forward propagate teardown
    ///
    /// #3 Err(TryRecvError::Empty): Packet queue is empty, await the consumer to awaken us with more
    /// work, by returning Async::NotReady to signal to runtime to sleep this task.
    ///
    /// #4 Err(TryRecvError::Disconnected): Consumer is in teardown and has dropped its side of the
    /// from_consumer channel; we will no longer receive packets. Return Async::Ready(None) to forward
    /// propagate teardown.
    /// ###
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.from_consumer.try_recv() {
            Ok(Some(packet)) => {
                match self.task_park.swap(TaskParkState::Empty) {
                    TaskParkState::Full(consumer) => {
                        consumer.notify();
                    }
                    TaskParkState::Empty => {}
                    // Similar to the consumer, if the `task_park` is dead, we must self notify
                    // and retain the Dead state to prevent deadlocks.
                    TaskParkState::Dead => {
                        self.task_park.store(TaskParkState::Dead);
                    }
                }
                Ok(Async::Ready(Some(packet)))
            }
            Ok(None) => Ok(Async::Ready(None)),
            Err(TryRecvError::Empty) => {
                let provider = task::current();
                match self.task_park.swap(TaskParkState::Full(provider)) {
                    TaskParkState::Full(consumer) => {
                        consumer.notify();
                    }
                    TaskParkState::Empty => {}
                    // Similar to the consumer, if the `task_park` is dead, we must self notify
                    // and retain the Dead state to prevent deadlocks.
                    TaskParkState::Dead => {
                        self.task_park.store(TaskParkState::Dead);
                        task::current().notify();
                    }
                }
                Ok(Async::NotReady)
            }
            Err(TryRecvError::Disconnected) => Ok(Async::Ready(None)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::element::ElementLink;
    use crate::utils::test::identity_elements::{AsyncIdentityElement, IdentityElement};
    use crate::utils::test::packet_collectors::ExhaustiveCollector;
    use crate::utils::test::packet_generators::{immediate_stream, PacketIntervalGenerator};
    use core::time;
    use futures::future::lazy;

    #[test]
    fn one_async_element() {
        let default_channel_size = 10;
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = immediate_stream(packets.clone());

        let elem0 = AsyncIdentityElement::new(0);

        let elem0_link =
            AsyncElementLink::new(Box::new(packet_generator), elem0, default_channel_size);

        let (s, r) = crossbeam_channel::unbounded();
        let elem0_drain = elem0_link.consumer;
        let elem0_collector = ExhaustiveCollector::new(0, Box::new(elem0_link.provider), s);

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

        let elem0 = AsyncIdentityElement::new(0);

        let elem0_link =
            AsyncElementLink::new(Box::new(packet_generator), elem0, default_channel_size);

        let (s, r) = crossbeam_channel::unbounded();
        let elem0_drain = elem0_link.consumer;
        let elem0_collector = ExhaustiveCollector::new(0, Box::new(elem0_link.provider), s);

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

        let elem0 = AsyncIdentityElement::new(0);

        let _elem0_link =
            AsyncElementLink::new(Box::new(packet_generator), elem0, default_channel_size);
    }

    #[test]
    fn one_async_element_small_channel() {
        let default_channel_size = 1;
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = PacketIntervalGenerator::new(
            time::Duration::from_millis(100),
            packets.clone().into_iter(),
        );

        let elem0 = AsyncIdentityElement::new(0);

        let elem0_link =
            AsyncElementLink::new(Box::new(packet_generator), elem0, default_channel_size);

        let (s, r) = crossbeam_channel::unbounded();
        let elem0_drain = elem0_link.consumer;
        let elem0_collector = ExhaustiveCollector::new(0, Box::new(elem0_link.provider), s);

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

        let elem0 = AsyncIdentityElement::new(0);

        let elem0_link =
            AsyncElementLink::new(Box::new(packet_generator), elem0, default_channel_size);

        let (s, r) = crossbeam_channel::unbounded();
        let elem0_drain = elem0_link.consumer;
        let elem0_collector = ExhaustiveCollector::new(0, Box::new(elem0_link.provider), s);

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

        let elem0 = AsyncIdentityElement::new(0);
        let elem1 = AsyncIdentityElement::new(1);

        let elem0_link =
            AsyncElementLink::new(Box::new(packet_generator), elem0, default_channel_size);
        let elem1_link =
            AsyncElementLink::new(Box::new(elem0_link.provider), elem1, default_channel_size);

        let elem0_drain = elem0_link.consumer;
        let elem1_drain = elem1_link.consumer;

        let (s, r) = crossbeam_channel::unbounded();
        let elem1_collector = ExhaustiveCollector::new(0, Box::new(elem1_link.provider), s);

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

        let elem0 = IdentityElement::new(0);
        let elem1 = AsyncIdentityElement::new(1);
        let elem2 = IdentityElement::new(2);
        let elem3 = AsyncIdentityElement::new(3);

        let elem0_link = ElementLink::new(Box::new(packet_generator), elem0);
        let elem1_link = AsyncElementLink::new(Box::new(elem0_link), elem1, default_channel_size);
        let elem2_link = ElementLink::new(Box::new(elem1_link.provider), elem2);
        let elem3_link = AsyncElementLink::new(Box::new(elem2_link), elem3, default_channel_size);

        let elem1_drain = elem1_link.consumer;
        let elem3_drain = elem3_link.consumer;

        let (s, r) = crossbeam_channel::unbounded();
        let elem3_collector = ExhaustiveCollector::new(0, Box::new(elem3_link.provider), s);

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

        let elem0 = AsyncIdentityElement::new(0);

        let elem0_link =
            AsyncElementLink::new(Box::new(packet_generator), elem0, default_channel_size);

        let (s, r) = crossbeam_channel::unbounded();
        let elem0_drain = elem0_link.consumer;
        let elem0_collector = ExhaustiveCollector::new(0, Box::new(elem0_link.provider), s);

        tokio::run(lazy(|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem0_collector);
            Ok(())
        }));

        let router_output: Vec<_> = r.iter().collect();
        assert_eq!(router_output, packets);
    }
}
