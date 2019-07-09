use futures::{Future, Stream, Async, Poll, task};
use crossbeam::crossbeam_channel::{bounded, Sender, Receiver, TryRecvError};

pub type ElementStream<Input> = Box<dyn Stream<Item = Input, Error = ()> + Send>;

pub trait Element {
    type Input: Sized;
    type Output: Sized;

    fn process(&mut self, packet: Self::Input) -> Self::Output;
}

pub struct ElementLink<E: Element> {
    input_stream: ElementStream<E::Input>,
    element: E
}

impl<E: Element> ElementLink<E> {
    pub fn new(input_stream: ElementStream<E::Input>, element: E) -> Self {
        ElementLink {
            input_stream,
            element
        }
    }
}

impl<E: Element> Stream for ElementLink<E> {
    type Item = E::Output;
    type Error = ();

    /*
    4 cases: Async::Ready(Some), Async::Ready(None), Async::NotReady, Err

    Async::Ready(Some): We have a packet ready to process from the upstream element. It's passed to
    our core's process function for... processing

    Async::Ready(None): The input_stream doesn't have anymore input. Semantically, it's like an
    iterator has exhausted it's input. We should return "Ok(Async::Ready(None))" to signify to our
    downstream components that there's no more input to process. Our Elements should rarely
    return "Async::Ready(None)" since it will effectively kill the Stream chain.

    Async::NotReady: There is more input for us to process, but we can't make any more progress right
    now. The contract for Streams asks us to register with a Reactor so we will be woken up again by
    an Executor, but we will be relying on Tokio to do that for us. This case is handled by the
    "try_ready!" macro, which will automatically return "Ok(Async::NotReady)" if the input stream
    gives us NotReady.

    Err: is also handled by the "try_ready!" macro.
    */
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let input_packet_option: Option<E::Input> = try_ready!(self.input_stream.poll());
        match input_packet_option {
            None => Ok(Async::Ready(None)),
            Some(input_packet) => {
                let output_packet: E::Output = self.element.process(input_packet);
                Ok(Async::Ready(Some(output_packet)))
            },
        }
    }
}

pub trait AsyncElement {
    type Input: Sized;
    type Output: Sized;

    fn process(&mut self, packet: Self::Input) -> Self::Output;
}

/// The AsyncElementLink is a wrapper to create and contain both sides of the
/// link, the consumer, which intakes and processes packets, and the provider,
/// which provides an interface where the next element retrieves the output
/// packet.
pub struct AsyncElementLink< E: AsyncElement> {
    pub consumer: AsyncElementConsumer<E>,
    pub provider: AsyncElementProvider<E>
}

impl<E: AsyncElement> AsyncElementLink<E> {
    pub fn new(input_stream: ElementStream<E::Input>, element: E, queue_capacity: usize) -> Self {

        let (to_provider, from_consumer) = bounded::<Option<E::Output>>(queue_capacity);
        let (await_provider, wake_provider) = bounded::<task::Task>(1);
        let (await_consumer, wake_consumer) = bounded::<task::Task>(1);

        AsyncElementLink {
            consumer: AsyncElementConsumer::new(input_stream, to_provider, element, await_consumer, wake_provider),
            provider: AsyncElementProvider::new(from_consumer, await_provider, wake_consumer)
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
    await_provider: Sender<task::Task>,
    wake_provider: Receiver<task::Task>
}

impl<E: AsyncElement> AsyncElementConsumer<E> {
    fn new(
        input_stream: ElementStream<E::Input>, 
        to_provider: Sender<Option<E::Output>>, 
        element: E,
        await_provider: Sender<task::Task>,
        wake_provider: Receiver<task::Task>) 
    -> Self {
        AsyncElementConsumer {
            input_stream,
            to_provider,
            element,
            await_provider,
            wake_provider
        }
    }
}

impl<E: AsyncElement> Drop for AsyncElementConsumer<E> {
    fn drop(&mut self) {
        if let Err(err) = self.to_provider.try_send(None) {
            panic!("Consumer: Drop: try_send to_provider, fail?: {:?}", err);
        }
        if let Ok(task) = self.wake_provider.try_recv() {
            task.notify();
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
        loop{
            if self.to_provider.is_full() {
                let task = task::current();
                if let Err(_) = self.await_provider.try_send(task) {
                    task::current().notify();
                }
                return Ok(Async::NotReady)
            }
            let input_packet_option: Option<E::Input> = try_ready!(self.input_stream.poll());

            match input_packet_option {
                None => {
                    return Ok(Async::Ready(()))
                }
                Some(input_packet) => {
                    let output_packet: E::Output = self.element.process(input_packet);
                    if let Err(err) = self.to_provider.send(Some(output_packet)) {
                        panic!("Error in to_provider sender, have nowhere to put packet: {:?}", err);
                    }
                    if let Ok(task) = self.wake_provider.try_recv() {
                        task.notify();
                    }
                },
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
    await_consumer: Sender<task::Task>,
    wake_consumer: Receiver<task::Task>
}

impl<E: AsyncElement> AsyncElementProvider<E> {
    fn new(from_consumer: Receiver<Option<E::Output>>, await_consumer: Sender<task::Task>, wake_consumer: Receiver<task::Task>) -> Self {
        AsyncElementProvider {
            from_consumer,
            await_consumer,
            wake_consumer
        }
    }
}

impl<E: AsyncElement> Drop for AsyncElementProvider<E> {
    fn drop(&mut self) {
        if let Ok(task) = self.wake_consumer.try_recv() {
            task.notify();
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