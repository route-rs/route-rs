use crate::link::{Link, LinkBuilder, PacketStream};
use crossbeam::crossbeam_channel;
use futures::prelude::*;
use futures::task::{Context, Poll};
use std::pin::Pin;

#[derive(Default)]
pub struct InputChannelLink<Packet> {
    channel_receiver: Option<crossbeam::Receiver<Packet>>,
}

impl<Packet> InputChannelLink<Packet> {
    pub fn new() -> Self {
        InputChannelLink {
            channel_receiver: None,
        }
    }

    pub fn channel(mut self, channel_receiver: crossbeam::Receiver<Packet>) -> Self {
        self.channel_receiver = Some(channel_receiver);
        self
    }
}

impl<Packet: Send + 'static> LinkBuilder<(), Packet> for InputChannelLink<Packet> {
    fn ingressors(self, mut _in_streams: Vec<PacketStream<()>>) -> Self {
        panic!("InputChannelLink does not take stream ingressors")
    }

    fn ingressor(self, _in_stream: PacketStream<()>) -> Self {
        panic!("InputChannelLink does not take any stream ingressors")
    }

    fn build_link(self) -> Link<Packet> {
        if self.channel_receiver.is_none() {
            panic!("Cannot build link! Missing channel");
        } else {
            (
                vec![],
                vec![Box::new(StreamFromChannel {
                    channel_receiver: self.channel_receiver.unwrap(),
                })],
            )
        }
    }
}

struct StreamFromChannel<Packet> {
    channel_receiver: crossbeam::Receiver<Packet>,
}

impl<Packet> Unpin for StreamFromChannel<Packet> {}

impl<Packet> Stream for StreamFromChannel<Packet> {
    type Item = Packet;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Option<Self::Item>> {
        match self.channel_receiver.try_recv() {
            Ok(packet) => Poll::Ready(Some(packet)),
            Err(crossbeam_channel::TryRecvError::Empty) => Poll::Pending,
            Err(crossbeam_channel::TryRecvError::Disconnected) => Poll::Ready(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test::harness::{initialize_runtime, test_link};
    use crate::utils::test::packet_generators::immediate_stream;

    #[test]
    #[should_panic]
    fn panics_when_built_with_ingressors() {
        InputChannelLink::<()>::new()
            .ingressors(vec![immediate_stream(vec![])])
            .build_link();
    }

    #[test]
    #[should_panic]
    fn panics_when_built_without_channel() {
        InputChannelLink::<()>::new().build_link();
    }

    #[test]
    fn immediate_packets() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];

        let mut runtime = initialize_runtime();
        let results = runtime.block_on(async {
            let (send, recv) = crossbeam_channel::unbounded();

            let link = InputChannelLink::new().channel(recv).build_link();

            for p in packets.clone() {
                match send.send(p) {
                    Ok(_) => (),
                    Err(e) => panic!("could not send to channel! {}", e),
                }
            }
            drop(send);

            test_link(link, None).await
        });
        assert_eq!(results[0], packets);
    }
}
