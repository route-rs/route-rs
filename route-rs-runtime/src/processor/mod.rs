mod identity;
pub use self::identity::*;

mod transform_from;
pub use self::transform_from::*;

mod drop;
pub use self::drop::*;

mod dec_ip_hop;
pub use self::dec_ip_hop::*;

pub trait Processor {
    type Input: Send + Clone;
    type Output: Send + Clone;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output>;
}
