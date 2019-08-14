use crate::api::{AsyncElement, Element};
use std::marker::PhantomData;

/* IdentityElement
  This is an element that passes what it has received
*/
pub struct IdentityElement<A: Sized> {
    id: i32,
    phantom: PhantomData<A>,
}

impl<A> IdentityElement<A> {
    pub fn new(id: i32) -> IdentityElement<A> {
        IdentityElement {
            id,
            phantom: PhantomData,
        }
    }
}

impl<A> Element for IdentityElement<A> {
    type Input = A;
    type Output = A;

    fn process(&mut self, packet: Self::Input) -> Self::Output {
        packet
    }
}

/* AsyncIdentityElement
  This is an async element that passes what it has received
*/
pub struct AsyncIdentityElement<A: Sized> {
    id: i32,
    phantom: PhantomData<A>,
}

impl<A> AsyncIdentityElement<A> {
    pub fn new(id: i32) -> AsyncIdentityElement<A> {
        AsyncIdentityElement {
            id,
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
