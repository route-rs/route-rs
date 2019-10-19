use crate::element::Element;
use std::convert::From;
use std::marker::PhantomData;
use std::marker::Send;

/*
  TransformElement
  This is an element that takes type A and passes the equivalent type B using From
*/
#[derive(Default)]
pub struct TransformElement<A: Send + Clone, B: Send + Clone> {
    phantom_in: PhantomData<A>,
    phantom_out: PhantomData<B>,
}

impl<A: Send + Clone, B: Send + Clone> TransformElement<A, B> {
    pub fn new() -> TransformElement<A, B> {
        TransformElement {
            phantom_in: PhantomData,
            phantom_out: PhantomData,
        }
    }
}

impl<A: Send + Clone, B: From<A> + Send + Clone> Element for TransformElement<A, B> {
    type Input = A;
    type Output = B;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        Some(Self::Output::from(packet))
    }
}
