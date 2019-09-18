///The common datatype that all packet structures share to repreasent their data
pub type PacketData<'packet> = &'packet mut Vec<u8>;

mod packet;
pub use self::packet::*;

mod ethernet;
pub use self::ethernet::*;

mod ipv4;
pub use self::ipv4::*;

mod ipv6;
pub use self::ipv6::*;

mod udp;
pub use self::udp::*;

mod tcp;
pub use self::tcp::*;
