use crate::processor::Processor;
use std::marker::PhantomData;

/// Processor that logs packets flowing through it
#[derive(Default)]
pub struct Trace<A: Send + Clone> {
    phantom: PhantomData<A>,
}

impl<A: Send + Clone> Trace<A> {
    pub fn new() -> Trace<A> {
        Trace {
            phantom: PhantomData,
        }
    }
}

impl<A: Send + Clone> Processor for Trace<A> {
    type Input = A;
    type Output = A;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        // TODO: Write trace logs somewhere
        Some(packet)
    }
}
