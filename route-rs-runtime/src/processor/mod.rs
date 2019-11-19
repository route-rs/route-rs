//! # What are they for?
//!
//! Processors are the unit of transformation in route-rs. The Processor is defined by a trait that requires that
//! creators of processors implement a process function, that the underlying Link will call on every packet that
//! moves through it. You can use processors to modify headers, count packets, update state, create new packets,
//! join packets, whatever you want! As long as an processor conforms to the trait specification, the processor can be run inside a route-rs.
//! While there are many provided processors that can be used to implement a router, users of route-rs that need specifc functionality
//! in their router most likely will implement their own custom processors, conforming to the laid out processor standard.

mod identity;
pub use self::identity::*;

mod transform_from;
pub use self::transform_from::*;

mod drop;
pub use self::drop::*;

mod dec_ip_hop;
pub use self::dec_ip_hop::*;

/// In order for a struct to become a Processor, it must implement this Trait.
pub trait Processor {
    type Input: Send + Clone;
    type Output: Send + Clone;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output>;
}
