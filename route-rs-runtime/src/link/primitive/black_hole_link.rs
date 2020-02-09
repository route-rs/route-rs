use crate::link::{Link, LinkBuilder, PacketStream};
use futures::task::{Context, Poll};
use futures::{Future, Stream};
use std::pin::Pin;

#[derive(Default)]
pub struct BlackHoleLink<Input> {
    in_stream: Option<PacketStream<Input>>,
}

impl<Input> BlackHoleLink<Input> {
    pub fn new() -> Self {
        BlackHoleLink { in_stream: None }
    }
}

impl<Input: Send + Unpin + 'static> LinkBuilder<Input, ()> for BlackHoleLink<Input> {
    fn ingressors(self, mut in_streams: Vec<PacketStream<Input>>) -> Self {
        assert_eq!(
            in_streams.len(),
            1,
            "BlackHoleLink may only take 1 input stream"
        );
        assert!(
            self.in_stream.is_none(),
            "BlackHoleLink may only take 1 input stream"
        );

        BlackHoleLink {
            in_stream: Some(in_streams.remove(0)),
        }
    }

    fn ingressor(self, in_stream: PacketStream<Input>) -> Self {
        assert!(
            self.in_stream.is_none(),
            "BlackHoleLink may only take 1 input stream"
        );

        BlackHoleLink {
            in_stream: Some(in_stream),
        }
    }

    fn build_link(self) -> Link<()> {
        assert!(
            self.in_stream.is_some(),
            "Cannot build link! Missing input stream"
        );

        let ingressor = BlackHoleIngressor::new(self.in_stream.unwrap());
        (vec![Box::new(ingressor)], vec![])
    }
}

pub struct BlackHoleIngressor<Input> {
    input_stream: PacketStream<Input>,
}

impl<Input> BlackHoleIngressor<Input> {
    fn new(input_stream: PacketStream<Input>) -> Self {
        BlackHoleIngressor { input_stream }
    }
}

impl<Input> Unpin for BlackHoleIngressor<Input> {}

impl<Input> Future for BlackHoleIngressor<Input> {
    type Output = ();

    /// Implement Poll for Future for BlackHoleIngressor
    ///
    /// This function accepts and ignores all packets from its input queue, until it cannot make
    /// any forward progress, at which point it returns a NotReady and waits for the stream to send
    /// more packets. If we get a Ready(None), the upstream has stopped and so we can, too.
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        loop {
            let input_packet_option: Option<Input> =
                ready!(Pin::new(&mut self.input_stream).poll_next(cx));

            match input_packet_option {
                None => {
                    return Poll::Ready(());
                }
                Some(_) => {
                    // Do nothing, allowing the runtime to drop the packet
                }
            }
        }
    }
}
