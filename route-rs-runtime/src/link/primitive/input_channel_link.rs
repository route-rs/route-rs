use crate::link::{Link, LinkBuilder, PacketStream};
use crossbeam::crossbeam_channel;
use futures::{Async, Poll, Stream};

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

    pub fn channel(self, channel_receiver: crossbeam::Receiver<Packet>) -> Self {
        InputChannelLink {
            channel_receiver: Some(channel_receiver),
        }
    }
}

impl<Packet: Send + 'static> LinkBuilder<(), Packet> for InputChannelLink<Packet> {
    fn ingressors(self, mut _in_streams: Vec<PacketStream<()>>) -> Self {
        panic!("InputChannelLink does not take stream ingressors")
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

impl<Packet> Stream for StreamFromChannel<Packet> {
    type Item = Packet;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.channel_receiver.try_recv() {
            Ok(packet) => Ok(Async::Ready(Some(packet))),
            Err(crossbeam_channel::TryRecvError::Empty) => Ok(Async::NotReady),
            Err(crossbeam_channel::TryRecvError::Disconnected) => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test::harness::run_link;
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
        let (send, recv) = crossbeam_channel::unbounded();

        let link = InputChannelLink::new().channel(recv).build_link();

        for p in packets.clone() {
            match send.send(p) {
                Ok(_) => (),
                Err(e) => panic!("could not send to channel! {}", e),
            }
        }
        drop(send);

        let results = run_link(link);
        assert_eq!(results[0], packets);
    }
}
