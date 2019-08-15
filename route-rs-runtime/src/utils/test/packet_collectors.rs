use crate::link::PacketStream;
use crossbeam::crossbeam_channel::Sender;
use futures::{Async, Future, Poll};
use std::fmt::Debug;

pub struct ExhaustiveDrain<T: Debug> {
    id: usize,
    stream: PacketStream<T>,
}

impl<T: Debug> ExhaustiveDrain<T> {
    pub fn new(id: usize, stream: PacketStream<T>) -> Self {
        ExhaustiveDrain { id, stream }
    }
}

impl<T: Debug> Future for ExhaustiveDrain<T> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            match try_ready!(self.stream.poll()) {
                Some(_value) => {}
                None => return Ok(Async::Ready(())),
            }
        }
    }
}

/// Exhaustive Collector works like Exhaustive Drain in that it continously polls for packets until it
/// receives a None, but it also will write that value out to the provided channel, so that the packet
/// may be compared in a test.
pub struct ExhaustiveCollector<T: Debug> {
    id: usize,
    stream: PacketStream<T>,
    packet_dump: Sender<T>,
}

impl<T: Debug> ExhaustiveCollector<T> {
    pub fn new(id: usize, stream: PacketStream<T>, packet_dump: Sender<T>) -> Self {
        ExhaustiveCollector {
            id,
            stream,
            packet_dump,
        }
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
                        panic!(
                            "Exhaustive Collector: Error sending to packet dump: {:?}",
                            err
                        );
                    };
                }
                None => return Ok(Async::Ready(())),
            }
        }
    }
}
