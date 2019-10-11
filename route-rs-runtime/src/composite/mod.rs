/// Basic composite that take in M streams of a type, and outputs that same
/// type to N packets.
mod mton_composite;
pub use self::mton_composite::*;