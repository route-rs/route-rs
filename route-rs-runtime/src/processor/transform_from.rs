use crate::processor::Processor;
use std::convert::From;
use std::marker::PhantomData;
use std::marker::Send;

/// Transform Processor
///
/// A generic processor that tranforms A -> B by calling B::from(A)
#[derive(Default)]
pub struct TransformFrom<A: Send + Clone, B: Send + Clone> {
    phantom_in: PhantomData<A>,
    phantom_out: PhantomData<B>,
}

impl<A: Send + Clone, B: Send + Clone> TransformFrom<A, B> {
    pub fn new() -> TransformFrom<A, B> {
        TransformFrom {
            phantom_in: PhantomData,
            phantom_out: PhantomData,
        }
    }
}

impl<A: Send + Clone, B: From<A> + Send + Clone> Processor for TransformFrom<A, B> {
    type Input = A;
    type Output = B;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        Some(Self::Output::from(packet))
    }
}
