use crate::api::ElementStream;
use futures::{stream, Async, Poll, Stream};
use std::time::Duration;
use tokio::timer::Interval;

// Immediately yields a collection of packets to be poll'd.
// Thin wrapper around iter_ok.
pub fn immediate_stream<I>(collection: I) -> ElementStream<I::Item>
where
    I: IntoIterator,
    I::IntoIter: Send + 'static,
{
    Box::new(stream::iter_ok::<_, ()>(collection))
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
            interval: Interval::new_interval(duration),
            iterations,
            seq_num: 0,
        }
    }
}

impl Stream for LinearIntervalGenerator {
    type Item = i32;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, ()> {
        try_ready!(self.interval.poll().map_err(|_| ()));
        if self.seq_num as usize > self.iterations {
            Ok(Async::Ready(None))
        } else {
            let next_packet = Ok(Async::Ready(Some(self.seq_num)));
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

impl<Iterable, Packet> PacketIntervalGenerator<Iterable, Packet>
where
    Iterable: Iterator<Item = Packet>,
    Packet: Sized,
{
    pub fn new(duration: Duration, packets: Iterable) -> Self {
        PacketIntervalGenerator {
            interval: Interval::new_interval(duration),
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
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, ()> {
        try_ready!(self.interval.poll().map_err(|_| ()));
        match self.packets.next() {
            Some(packet) => Ok(Async::Ready(Some(packet))),
            None => Ok(Async::Ready(None)),
        }
    }
}
