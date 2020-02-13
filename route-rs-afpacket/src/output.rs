use afpacket;
use futures::{
    self, ready,
    task::{Context, Poll},
    Future, Stream,
};
use route_rs_runtime::link;
use std::pin::Pin;

#[derive(Default)]
pub struct AfPacketOutputLink {
    in_stream: Option<link::PacketStream<Vec<u8>>>,
    egress: Option<afpacket::SendHalf>,
}

impl AfPacketOutputLink {
    pub fn new() -> Self {
        Self {
            in_stream: None,
            egress: None,
        }
    }

    pub fn channel(self, egress: afpacket::SendHalf) -> Self {
        Self {
            in_stream: self.in_stream,
            egress: Some(egress),
        }
    }
}

impl link::LinkBuilder<Vec<u8>, ()> for AfPacketOutputLink {
    fn ingressors(self, mut in_streams: Vec<link::PacketStream<Vec<u8>>>) -> Self {
        assert_eq!(
            in_streams.len(),
            1,
            "AfPacketOutputLink may only take 1 input stream"
        );

        if self.in_stream.is_some() {
            panic!("AfPacketOutputLink may only take 1 input stream");
        }

        Self {
            in_stream: Some(in_streams.remove(0)),
            egress: self.egress,
        }
    }

    fn ingressor(self, in_stream: link::PacketStream<Vec<u8>>) -> Self {
        if self.in_stream.is_some() {
            panic!("AfPacketOutputLink may only take 1 input stream");
        }

        Self {
            in_stream: Some(in_stream),
            egress: self.egress,
        }
    }

    fn build_link(self) -> link::Link<()> {
        match (self.in_stream, self.egress) {
            (None, _) => panic!("Cannot build link! Missing input streams"),
            (_, None) => panic!("Cannot build link! Missing channel"),
            (Some(in_stream), Some(egress)) => (
                vec![Box::new(StreamToChannel {
                    stream: in_stream,
                    egress,
                })],
                vec![],
            ),
        }
    }
}

struct StreamToChannel {
    stream: link::PacketStream<Vec<u8>>,
    egress: afpacket::SendHalf,
}

impl Unpin for StreamToChannel {}

impl Future for StreamToChannel {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        loop {
            let mut lock =
                ready!(me.egress.poll_lock_tx(cx)).expect("AfPacketOutputLink: failed to write");
            match ready!(Pin::new(&mut me.stream).poll_next(cx)) {
                Some(pkt) => {
                    lock.try_send(&pkt)
                        .expect("AfPacketOutputLink: failed to send packet");
                }
                None => return Poll::Ready(()),
            }
        }
    }
}
