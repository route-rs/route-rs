use crate::element::Element;
use std::marker::PhantomData;

/* IdentityElement
  This is an element that passes what it has received
*/
#[derive(Default)]
pub struct IdentityElement<A: Send + Clone> {
    phantom: PhantomData<A>,
}

impl<A: Send + Clone> IdentityElement<A> {
    pub fn new() -> IdentityElement<A> {
        IdentityElement {
            phantom: PhantomData,
        }
    }
}

impl<A: Send + Clone> Element for IdentityElement<A> {
    type Input = A;
    type Output = A;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        Some(packet)
    }
}
