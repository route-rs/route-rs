use crate::link::PacketStream;
use crossbeam::crossbeam_channel::Sender;
use futures::prelude::*;
use futures::ready;
use futures::task::{Context, Poll};
use std::fmt::Debug;
use std::pin::Pin;

/// A structure that may be handed an input stream that it will exhaustively drain from until it
/// recieves a None. Useful for testing purposes.
pub struct ExhaustiveDrain<T: Debug> {
    id: usize,
    stream: PacketStream<T>,
}

impl<T: Debug> Unpin for ExhaustiveDrain<T> {}

impl<T: Debug> ExhaustiveDrain<T> {
    pub fn new(id: usize, stream: PacketStream<T>) -> Self {
        ExhaustiveDrain { id, stream }
    }
}

impl<T: Debug> Future for ExhaustiveDrain<T> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let drain = Pin::into_inner(self);
        loop {
            match ready!(Pin::new(&mut drain.stream).poll_next(cx)) {
                Some(_value) => {}
                None => return Poll::Ready(()),
            }
        }
    }
}

/// Exhaustive Collector works like Exhaustive Drain in that it continuously polls for packets until it
/// receives a None, but it also will write that value out to the provided channel, so that the packet
/// may be compared in a test.
pub struct ExhaustiveCollector<T: Debug> {
    id: usize,
    stream: PacketStream<T>,
    packet_dump: Sender<T>,
}

impl<T: Debug> Unpin for ExhaustiveCollector<T> {}

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
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let collector = Pin::into_inner(self);
        loop {
            match ready!(Pin::new(&mut collector.stream).poll_next(cx)) {
                Some(value) => {
                    collector
                        .packet_dump
                        .try_send(value)
                        .expect("Exhaustive Collector: Error sending to packet dump");
                }
                None => return Poll::Ready(()),
            }
        }
    }
}
