use crate::element::{AsyncElement, Element};
use std::fmt::Debug;
use std::marker::PhantomData;
use tracing::instrument;

/* IdentityElement
  This is an element that passes what it has received
*/
#[derive(Default)]
pub struct IdentityElement<A: Sized + Debug> {
    phantom: PhantomData<A>,
}

impl<A: Debug> IdentityElement<A> {
    pub fn new() -> IdentityElement<A> {
        IdentityElement {
            phantom: PhantomData,
        }
    }
}

impl<A: Debug> Element for IdentityElement<A> {
    type Input = A;
    type Output = A;

    #[instrument]
    fn process(&mut self, packet: Self::Input) -> Self::Output {
        packet
    }
}

/* AsyncIdentityElement
  This is an async element that passes what it has received
*/
#[derive(Default)]
pub struct AsyncIdentityElement<A: Sized> {
    phantom: PhantomData<A>,
}

impl<A> AsyncIdentityElement<A> {
    pub fn new() -> AsyncIdentityElement<A> {
        AsyncIdentityElement {
            phantom: PhantomData,
        }
    }
}

impl<A> AsyncElement for AsyncIdentityElement<A> {
    type Input = A;
    type Output = A;

    fn process(&mut self, packet: Self::Input) -> Self::Output {
        packet
    }
}
