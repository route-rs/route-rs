//! # What are they for?
//!
//! Classifiers are very similar to processors, but are used to differentiate a stream of packets. As such, they take each packet by reference,
//! and are not able to modify it. They are only used in the ClassifyLink. Classifiers are able to return any type, but generally return an Enum
//! that will inform the Dispatch section of the ClassifyLink which group each packet belongs to. The Dispatch then moves each packet to a port
//! based on its classification.
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
