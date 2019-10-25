mod identity_elements;
pub use self::identity_elements::*;

mod transform_elements;
pub use self::transform_elements::*;

mod drop_element;
pub use self::drop_element::*;

mod dec_ip_hop;
pub use self::dec_ip_hop::*;

pub trait Element {
    type Input: Send + Clone;
    type Output: Send + Clone;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output>;
}
