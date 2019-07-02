//! This crate defines an interface for accessing a raw network interface.
//! Actual implementations are found in sister crates.
#![feature(async_await)]

mod memory;

pub use memory::{Packet, PacketMut};
use futures::{Stream, Sink};

// TODO a real error type
pub use failure::Error;

pub trait Receiver<'memory> : Stream<Item=Packet<'memory>, Error=Error> {
}

pub trait Sender<'memory> : Sink<SinkItem=PacketMut<'memory>, SinkError=Error> {
}

pub trait NetIf {
    fn open<'m>(&'m self) -> Result<(
        Box<dyn Receiver<'m>>,
        Box<dyn Sender<'m>>), Error>;

    fn create_packet<'m>(&'m self) -> Result<PacketMut<'m>, Error>;
}
