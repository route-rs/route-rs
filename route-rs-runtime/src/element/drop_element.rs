use crate::element::Element;
use std::marker::PhantomData;

/* DropElement
  This element drops every packet that it receives
*/
#[derive(Default)]
pub struct DropElement<A: Send + Clone> {
    phantom: PhantomData<A>,
}

impl<A: Send + Clone> DropElement<A> {
    pub fn new() -> DropElement<A> {
        DropElement {
            phantom: PhantomData,
        }
    }
}

impl<A: Send + Clone> Element for DropElement<A> {
    type Input = A;
    type Output = A;

    fn process(&mut self, _packet: Self::Input) -> Option<Self::Output> {
        None
    }
}
