mod even;
pub use self::even::*;

mod fizz_buzz;
pub use self::fizz_buzz::*;

/// Used by a ClassifyLink to determine the kind of packet we have. Classifier::Class is then
/// consumed by the dispatcher on the ClassifyLink to send it down the appropriate path.
pub trait Classifier {
    type Packet: Send + Clone;
    type Class: Sized;

    fn classify(&self, packet: &Self::Packet) -> Self::Class;
}
