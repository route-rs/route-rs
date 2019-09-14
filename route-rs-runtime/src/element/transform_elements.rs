use crate::element::{AsyncElement, Element};
use std::convert::From;
use std::marker::PhantomData;

/*
  TransformElement
  This is an element that takes type A and passes the equivalent type B using From
*/
#[derive(Default)]
pub struct TransformElement<A: Sized, B: Sized> {
    phantom_in: PhantomData<A>,
    phantom_out: PhantomData<B>,
}

impl<A, B> TransformElement<A, B> {
    pub fn new() -> TransformElement<A, B> {
        TransformElement {
            phantom_in: PhantomData,
            phantom_out: PhantomData,
        }
    }
}

impl<A, B: From<A>> Element for TransformElement<A, B> {
    type Input = A;
    type Output = B;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        Some(Self::Output::from(packet))
    }
}

impl<A, B: From<A>> AsyncElement for TransformElement<A, B> {
    type Input = A;
    type Output = B;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        Some(Self::Output::from(packet))
    }
}
