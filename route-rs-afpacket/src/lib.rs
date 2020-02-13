//! This crate adds route-rs links to connect with the `afpacket` crate.
#![deny(missing_docs)]

mod input;
mod output;

pub use input::AfPacketInput;
pub use output::AfPacketOutput;
