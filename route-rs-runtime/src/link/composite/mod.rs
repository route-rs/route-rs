/// Basic composite that take in M streams of a type, and outputs that same
/// type to N packets.
mod mton_link;
pub use self::mton_link::*;

/// More complex composite that will take in N ingress streams of type Input,
/// and transforms it using a user provided processor to M Output streams.
mod m_transform_n_link;
pub use self::m_transform_n_link::*;

/// Drops packets with weighted randomness.
mod drop_link;
pub use self::drop_link::*;
