use crate::link::PacketStream;
use futures::prelude::*;
use futures::task::{Context, Poll};
use std::pin::Pin;
use tokio::time::{interval, Duration, Interval};

/// Immediately yields a collection of packets to be poll'd.
/// Thin wrapper around iter_ok.
pub fn immediate_stream<I>(collection: I) -> PacketStream<I::Item>
where
    I: IntoIterator,
    I::IntoIter: Send + 'static,
{
    Box::new(stream::iter(collection))
}

/*
    LinearIntervalGenerator

    Generates a series of monotonically increasing integers, starting at 0.
    `iterations` "packets" are generated in the stream. One is yielded every
    `duration`.
*/

pub struct LinearIntervalGenerator {
    interval: Interval,
    iterations: usize,
    seq_num: i32,
}

impl LinearIntervalGenerator {
    pub fn new(duration: Duration, iterations: usize) -> Self {
        LinearIntervalGenerator {
            interval: interval(duration),
            iterations,
            seq_num: 0,
        }
    }
}

impl Stream for LinearIntervalGenerator {
    type Item = i32;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        ready!(Pin::new(&mut self.interval).poll_next(cx));
        if self.seq_num as usize > self.iterations {
            Poll::Ready(None)
        } else {
            let next_packet = Poll::Ready(Some(self.seq_num));
            self.seq_num += 1;
            next_packet
        }
    }
}

/// Packet Interval Generator procduces a Stream of packets on a defined interval.AsMut
///
/// Which packet is next sent is determined by the Iterator, provided during creation. This
/// is intended to be a full fledged packet eventually, but the trait bound is only set to
/// something that is Sized. The Iterator is polled until it runs out of values, at which
/// point we close the Stream by sending a Ready(None).
pub struct PacketIntervalGenerator<Iterable, Packet>
where
    Iterable: Iterator<Item = Packet>,
    Packet: Sized,
{
    interval: Interval,
    packets: Iterable,
}

impl<Iterable, Packet> Unpin for PacketIntervalGenerator<Iterable, Packet>
where
    Iterable: Iterator<Item = Packet>,
    Packet: Sized,
{
}

impl<Iterable, Packet> PacketIntervalGenerator<Iterable, Packet>
where
    Iterable: Iterator<Item = Packet>,
    Packet: Sized,
{
    pub fn new(duration: Duration, packets: Iterable) -> Self {
        PacketIntervalGenerator {
            interval: interval(duration),
            packets,
        }
    }
}

impl<Iterable, Packet> Stream for PacketIntervalGenerator<Iterable, Packet>
where
    Iterable: Iterator<Item = Packet>,
    Packet: Sized,
{
    type Item = Packet;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let interval_generator = Pin::into_inner(self);
        ready!(Pin::new(&mut interval_generator.interval).poll_next(cx));
        match interval_generator.packets.next() {
            Some(packet) => Poll::Ready(Some(packet)),
            None => Poll::Ready(None),
        }
    }
}
