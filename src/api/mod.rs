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

pub struct AsyncElementFrontend<E: AsyncElement> {
    input_stream: ElementStream<E::Input>,
    channel: Sender<E::Output>,
    element: E
}

pub struct AsyncElementBackend<E: AsyncElement> {
    channel: Receiver<E::Output>
}

pub struct AsyncElementLink< E: AsyncElement> {
    pub frontend: AsyncElementFrontend<E>,
    pub backend: AsyncElementBackend<E>
}

impl<E: AsyncElement> AsyncElementFrontend<E> {
    fn new(input_stream: ElementStream<E::Input>, channel: Sender<E::Output>, element: E) -> Self {
        AsyncElementFrontend {
            input_stream,
            channel,
            element
        }
    }
}

impl<E: AsyncElement> AsyncElementBackend<E> {
    fn new(channel: Receiver<E::Output>) -> Self {
        AsyncElementBackend {
            channel
        }
    }
}

impl<E: AsyncElement> AsyncElementLink<E> {
    pub fn new(input_stream: ElementStream<E::Input>, element: E, queue_capacity: usize) -> Self {
        let (sender, receiver) = bounded::<E::Output>(queue_capacity); 
        AsyncElementLink {
            frontend: AsyncElementFrontend::new(input_stream, sender, element),
            backend: AsyncElementBackend::new(receiver)
        }
    }
}

/*
*/
impl<E: AsyncElement> Future for AsyncElementFrontend<E> {
    type Item = ();
    type Error = ();

    /*
    //Documentation
    */
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if self.channel.is_full() {
            //Queue is full, immediately reschedule task.
            task::current().notify();
            return Ok(Async::NotReady);
        }
        //Pull packets off input stream unti we run out of packets
        loop {
            //try_ready! will bubble up the Async::NotReady if receives one
            let input_packet_option: Option<E::Input> = try_ready!(self.input_stream.poll());
            match input_packet_option {
                None => {
                    println!("packet option was none");
                    return Ok(Async::Ready(()));
                }
                Some(input_packet) => {
                    let output_packet: E::Output = self.element.process(input_packet);
                    //Assume that channel cannot be filled since we checked
                    if let Err(err) = self.channel.send(output_packet) {
                        println!("{:?}", err);
                        return Ok(Async::Ready(()));
                    }
                },
            }
        }
    }
}

impl<E: AsyncElement> Stream for AsyncElementBackend<E> {
    type Item = E::Output;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        // Check associated channel receiver to see if we have any packets available to send along
        match self.channel.try_recv() {
            Ok(packet) => Ok(Async::Ready(Some(packet))),
            Err(TryRecvError::Empty) => {
                //Nothing in queue, immediately tell Tokio to reschedule this task.
                task::current().notify();
                Ok(Async::NotReady)
                },
            Err(TryRecvError::Disconnected) => {
                println!("receiving channel disconnected");
                Ok(Async::Ready(None))
            }
        }
    }
}