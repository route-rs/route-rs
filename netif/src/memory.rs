use std::marker::PhantomData;
use delegate::delegate;
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// Represents a raw packet received from a network interface.
pub struct Packet<'memblock> {
    inner: Bytes,
    phantom: PhantomData<&'memblock ()>,
}

// XXX this is only necessary because Bytes does not impl Buf as of today.
// Future releases of Bytes will impl Buf.
impl Buf for Packet<'_> {
    #[inline]
    fn remaining(&self) -> usize {
        self.inner.len()
    }

    #[inline]
    fn bytes(&self) -> &[u8] {
        self.inner.as_ref()
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        self.inner.advance(cnt)
    }
}

/// Represents a packet to be transmitted on a network interface.
pub struct PacketMut<'memblock> {
    inner: BytesMut,
    phantom: PhantomData<&'memblock ()>
}

impl BufMut for PacketMut<'_> {
    delegate! {
        target self.inner {
            fn remaining_mut(&self) -> usize;
            unsafe fn advance_mut(&mut self, cnt: usize);
            unsafe fn bytes_mut(&mut self) -> &mut [u8];
        }
    }
}
=======
use std::{
    slice,
    borrow::{Borrow, ToOwned, Cow},
    marker::PhantomData};

/// A Chunk is an owned block of memory.
#[derive(Clone)]
pub struct Chunk {
    inner: Vec<u8>, //<- TODO is this right?
}

impl Chunk {
    pub fn with_capacity(capacity: usize) -> Chunk {
        Chunk {
            inner: vec![0; capacity],
        }
    }

    pub fn as_span(&self) -> &Span {
        Span::new(&self.inner)
    }
}

/// A Span is a reference to a region of memory.
pub struct Span {
    inner: [u8], //<- TODO is this right?
}

impl Span {
    pub fn new<S>(s: &S) -> &Span
        where S : AsRef<Span> + ?Sized
    {
        s.as_ref()
    }
}

type Packet<'a> = Cow<'a, Span>;
