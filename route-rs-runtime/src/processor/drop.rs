use crate::processor::Processor;
use std::marker::PhantomData;

/* DropProcessor
  This processor drops every packet that it receives
*/
#[derive(Default)]
pub struct Drop<A: Send + Clone> {
    phantom: PhantomData<A>,
}

impl<A: Send + Clone> Drop<A> {
    pub fn new() -> Drop<A> {
        Drop {
            phantom: PhantomData,
        }
    }
}

impl<A: Send + Clone> Processor for Drop<A> {
    type Input = A;
    type Output = A;

    fn process(&mut self, _packet: Self::Input) -> Option<Self::Output> {
        None
    }
}
