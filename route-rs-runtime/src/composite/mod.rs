/// Basic composite that take in M streams of a type, and outputs that same
/// type to N packets.
mod mton_composite;
pub use self::mton_composite::*;

/// More complex composite that will take in N ingress streams of type Input,
/// and transforms it using a user provided element to M Output streams.
mod m_transform_n_composite;
pub use self::m_transform_n_composite::*;
