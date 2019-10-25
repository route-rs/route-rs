use crate::processor::Processor;
use std::marker::PhantomData;

/* IdentityProcessor
  This is an processor that passes what it has received
*/
#[derive(Default)]
pub struct Identity<A: Send + Clone> {
    phantom: PhantomData<A>,
}

impl<A: Send + Clone> Identity<A> {
    pub fn new() -> Identity<A> {
        Identity {
            phantom: PhantomData,
        }
    }
}

impl<A: Send + Clone> Processor for Identity<A> {
    type Input = A;
    type Output = A;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        Some(packet)
    }
}
