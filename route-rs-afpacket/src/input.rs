use afpacket;
use futures::{
    self, ready,
    task::{Context, Poll},
};
use route_rs_runtime::link;
use std::{mem, pin::Pin};

/// This represents the maximum transmission unit (MTU) for a frame.
const MTU: usize = 1500;

/// This link is used as a source for packets. It receives packets from
/// an AF_PACKET socket attached to it and returns them as an incoming
/// packet stream.
#[derive(Default)]
pub struct AfPacketInput {
    ingress: Option<afpacket::RecvHalf>,
}

impl AfPacketInput {
    /// Creates a new empty `AfPacketInput` link.
    pub fn new() -> Self {
        Self { ingress: None }
    }

    /// Attaches an input socket (type `RecvHalf`) to this link.
    pub fn channel(self, ingress: afpacket::RecvHalf) -> Self {
        Self {
            ingress: Some(ingress),
        }
    }
}

impl link::LinkBuilder<(), Vec<u8>> for AfPacketInput {
    fn ingressors(self, _in_streams: Vec<link::PacketStream<()>>) -> Self {
        panic!("AfPacketInputLink does not take stream ingressors")
    }

    fn ingressor(self, _in_stream: link::PacketStream<()>) -> Self {
        panic!("AfPacketInputLink does not take any stream ingressors")
    }

    fn build_link(self) -> link::Link<Vec<u8>> {
        if self.ingress.is_none() {
            panic!("Cannot build link! Missing ingress");
        } else {
            (
                vec![],
                vec![Box::new(Stream {
                    ingress: self.ingress.unwrap(),
                    in_buf: vec![0; MTU],
                })],
            )
        }
    }
}

struct Stream {
    ingress: afpacket::RecvHalf,
    in_buf: Vec<u8>,
}

impl Unpin for Stream {}

impl futures::Stream for Stream {
    type Item = Vec<u8>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let me = self.get_mut();
        let (sz, _) =
            ready!(me.ingress.poll_recv(cx, &mut me.in_buf)).expect("failed to read packet");
        me.in_buf.resize(sz, 0);
        let buf = mem::replace(&mut me.in_buf, vec![0; MTU]);
        Poll::Ready(Some(buf))
    }
}