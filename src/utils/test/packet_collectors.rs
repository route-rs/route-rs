use crate::api::ElementStream;
use futures::{Async, Poll, Future};
use std::fmt::Debug;
use crossbeam::crossbeam_channel;

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

/// Exhaustive Collector works like Exhaustive Drain in that it continously polls for packets until it
/// receives a None, but it also will write that value out to the provided channel, so that the packet
/// may be compared in a test.
pub struct ExhaustiveCollector<T: Debug> {
    id: usize,
    stream: ElementStream<T>,
    packet_dump: crossbeam_channel::Sender<T>
}

impl<T: Debug> ExhaustiveCollector<T> {
    pub fn new(id: usize, stream: ElementStream<T>, packet_dump: crossbeam_channel::Sender<T>) -> Self {
        ExhaustiveCollector { id, stream, packet_dump }
    }
}

impl<T: Debug> Future for ExhaustiveCollector<T> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            match try_ready!(self.stream.poll()) {
                Some(value) => {
                    if let Err(err) = self.packet_dump.try_send(value) {
                        println!("Exhaustive Collector: Error sending to packet dump: {:?}", err);
                    };
                },
                None => {
                    println!("Collector #{} received none. End of packet stream", self.id);
                    return Ok(Async::Ready(()))
                }
            }
        }
    }
}
