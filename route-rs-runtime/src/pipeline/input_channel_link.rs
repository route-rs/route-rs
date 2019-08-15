use crossbeam::crossbeam_channel;
use futures::{Async, Poll, Stream};

pub struct InputChannelLink<Input> {
    input_channel: crossbeam::Receiver<Input>,
}

impl<Input> InputChannelLink<Input> {
    pub fn new(input_channel: crossbeam::Receiver<Input>) -> Self {
        InputChannelLink { input_channel }
    }
}

impl<Input> Stream for InputChannelLink<Input> {
    type Item = Input;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.input_channel.try_recv() {
            Ok(packet) => Ok(Async::Ready(Some(packet))),
            Err(crossbeam_channel::TryRecvError::Empty) => Ok(Async::NotReady),
            Err(crossbeam_channel::TryRecvError::Disconnected) => Err(()),
        }
    }
}
