use futures::{Stream, Async, Poll};
use crossbeam::{Receiver, Sender};

pub type ElementStream<Input> = Box<dyn Stream<Item = Input, Error = ()> + Send>;

pub trait Element {
    type Input: Sized;
    type Output: Sized;

    fn process(&mut self, packet: Self::Input) -> Self::Output;
}

pub struct ElementLink<E: Element> {
    input_stream: ElementStream<E::Input>,
    core: E
}

impl<E: Element> ElementLink<E> {
    pub fn new(input_stream: ElementStream<E::Input>, core: E) -> Self {
        ElementLink {
            input_stream,
            core
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
                let output_packet: E::Output = self.core.process(input_packet);
                Ok(Async::Ready(Some(output_packet)))
            },
        }
    }
}

pub trait AsyncElement {
    type Input: Sized;
    type Output: Sized;

    fn process(&mut self, packet: Self::Input) -> ElementStream<Self::Output>;
}

pub struct AsyncElementLink<E: AsyncElement> {
    input_channel: Receiver<E::Input>,
    output_channel: Sender<E::Output>
}