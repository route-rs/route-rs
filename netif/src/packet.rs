use std::{
    sync::Arc,
    ops::Deref};

#[derive(Clone)]
pub struct PacketOwned {
    pkt: Vec<u8>,
}

impl Deref for PacketOwned {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.pkt
    }
}

#[derive(Clone)]
pub struct PacketRef {
    inner: Arc<PacketOwned>,
}

impl Deref for PacketRef {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}
