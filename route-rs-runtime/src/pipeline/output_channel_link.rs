use crate::link::PacketStream;
use futures::{Async, Future, Poll};

pub struct OutputChannelLink<Output> {
    input_stream: PacketStream<Output>,
    output_channel: crossbeam::Sender<Output>,
}

impl<Output> OutputChannelLink<Output> {
    pub fn new(
        input_stream: PacketStream<Output>,
        output_channel: crossbeam::Sender<Output>,
    ) -> Self {
        OutputChannelLink {
            input_stream,
            output_channel,
        }
    }
}

impl<Output> Future for OutputChannelLink<Output> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            if self.output_channel.is_full() {
                return Ok(Async::NotReady);
            }

            match try_ready!(self.input_stream.poll()) {
                Some(packet) => self
                    .output_channel
                    .try_send(packet)
                    .expect("OutputChannelLink::Poll: try_send to_egressor shouldn't fail"),
                None => return Ok(Async::Ready(())),
            }
        }
    }
}
