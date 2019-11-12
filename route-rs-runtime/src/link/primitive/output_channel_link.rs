use crate::link::{Link, LinkBuilder, PacketStream};
use futures::{task, Async, Future, Poll};

#[derive(Default)]
pub struct OutputChannelLink<Packet> {
    in_stream: Option<PacketStream<Packet>>,
    channel_sender: Option<crossbeam::Sender<Packet>>,
}

impl<Packet> OutputChannelLink<Packet> {
    pub fn new() -> Self {
        OutputChannelLink {
            in_stream: None,
            channel_sender: None,
        }
    }

    pub fn channel(self, channel_sender: crossbeam::Sender<Packet>) -> Self {
        OutputChannelLink {
            in_stream: self.in_stream,
            channel_sender: Some(channel_sender),
        }
    }

    pub fn ingressor(self, in_stream: PacketStream<Packet>) -> Self {
        OutputChannelLink {
            in_stream: Some(in_stream),
            channel_sender: self.channel_sender,
        }
    }
}

impl<Packet: Send + 'static> LinkBuilder<Packet, ()> for OutputChannelLink<Packet> {
    fn ingressors(self, mut in_streams: Vec<PacketStream<Packet>>) -> Self {
        assert_eq!(
            in_streams.len(),
            1,
            "OutputChannelLink may only take 1 input stream"
        );

        OutputChannelLink {
            in_stream: Some(in_streams.remove(0)),
            channel_sender: self.channel_sender,
        }
    }

    fn build_link(self) -> Link<()> {
        match (self.in_stream, self.channel_sender) {
            (None, _) => panic!("Cannot build link! Missing input streams"),
            (_, None) => panic!("Cannot build link! Missing channel"),
            (Some(in_stream), Some(sender)) => (
                vec![Box::new(StreamToChannel {
                    stream: in_stream,
                    channel_sender: sender,
                })],
                vec![],
            ),
        }
    }
}

struct StreamToChannel<Packet> {
    stream: PacketStream<Packet>,
    channel_sender: crossbeam::Sender<Packet>,
}

impl<Packet> Future for StreamToChannel<Packet> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            if self.channel_sender.is_full() {
                // Since we don't know anything about the other side of our channel, we have to
                // self-notify and just hope that the other side empties it eventually.
                task::current().notify();
                return Ok(Async::NotReady);
            }

            match try_ready!(self.stream.poll()) {
                Some(packet) => self
                    .channel_sender
                    .try_send(packet)
                    .expect("OutputChannelLink::poll: try_send shouldn't fail"),
                None => return Ok(Async::Ready(())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test::harness::run_link;
    use crate::utils::test::packet_generators::immediate_stream;
    use crossbeam::crossbeam_channel;
    use std::thread;

    #[test]
    #[should_panic]
    fn panics_when_built_without_ingressor() {
        let (s, _r) = crossbeam::unbounded();

        OutputChannelLink::<()>::new().channel(s).build_link();
    }

    #[test]
    #[should_panic]
    fn panics_when_built_without_channel() {
        let packet_generator = immediate_stream(vec![]);

        OutputChannelLink::<()>::new()
            .ingressor(packet_generator)
            .build_link();
    }

    #[test]
    #[should_panic]
    fn panics_when_built_with_multiple_ingressors() {
        let (s, _r) = crossbeam::unbounded();
        let packet_generator_1 = immediate_stream(vec![]);
        let packet_generator_2 = immediate_stream(vec![]);

        OutputChannelLink::<()>::new()
            .ingressors(vec![packet_generator_1, packet_generator_2])
            .channel(s)
            .build_link();
    }

    #[test]
    fn immediate_packets() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let (send, recv) = crossbeam_channel::unbounded::<i32>();

        let link = OutputChannelLink::new()
            .ingressor(immediate_stream(packets.clone()))
            .channel(send)
            .build_link();

        let results = run_link(link);
        assert!(results.is_empty());

        assert_eq!(recv.iter().collect::<Vec<i32>>(), packets);
    }

    #[test]
    fn small_queue() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let (send, recv) = crossbeam_channel::bounded::<i32>(2);

        let recv_thread = thread::spawn(move || {
            let mut outputs = vec![];
            while let Ok(n) = recv.recv() {
                outputs.push(n);
            }
            outputs
        });

        let link = OutputChannelLink::new()
            .ingressor(immediate_stream(packets.clone()))
            .channel(send)
            .build_link();

        let results = run_link(link);
        assert!(results.is_empty());

        let outputs = recv_thread.join().unwrap();
        assert_eq!(outputs, packets);
    }
}
