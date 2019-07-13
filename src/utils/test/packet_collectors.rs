use crate::api::ElementStream;
use futures::{Async, Poll, Future};
use std::fmt::Debug;

pub struct ExhaustiveDrain<T: Debug> {
    id: usize,
    stream: ElementStream<T>
}

impl<T: Debug> ExhaustiveDrain<T> {
    pub fn new(id: usize, stream: ElementStream<T>) -> Self {
        ExhaustiveDrain { id, stream }
    }
}

impl<T: Debug> Future for ExhaustiveDrain<T> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        // println!("Drain #{} poll", self.id);

        loop {
            match try_ready!(self.stream.poll()) {
                Some(value) => {
                    println!("Drain #{} received packet: {:?}", self.id, value);
                },
                None => {
                    println!("Drain #{} received none. End of packet stream", self.id);
                    return Ok(Async::Ready(()))
                }
            }
        }
    }
}
