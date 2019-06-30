use crate::api::ElementStream;
use futures::{Stream, Async, Poll, Future};
use tokio::timer::Interval;
use std::time::Duration;
use std::fmt::Debug;

// TODO: move generators and Drains into their own modules

/*
Packet Generators
*/

#[allow(dead_code)]
pub struct LinearIntervalGenerator {
    interval: Interval,
    iterations: usize,
    seq_num: i32
}

#[allow(dead_code)]
impl LinearIntervalGenerator {
    pub fn new(duration: Duration, iterations: usize) -> Self {
        LinearIntervalGenerator {
            interval: Interval::new_interval(duration),
            iterations,
            seq_num: 0
        }
    }
}

#[allow(dead_code)]
impl Stream for LinearIntervalGenerator {
    type Item = i32;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, ()> {
        try_ready!(self.interval.poll().map_err(|_| ()));
        if self.seq_num as usize > self.iterations {
            Ok(Async::Ready(None))
        } else {
            self.seq_num += 1;
            Ok(Async::Ready(Some(self.seq_num)))
        }
    }
}

/*
Element Drains
*/

#[allow(dead_code)]
pub struct ExhaustiveDrain<T: Debug> {
    id: usize,
    stream: ElementStream<T>
}

#[allow(dead_code)]
impl<T: Debug> ExhaustiveDrain<T> {
    pub fn new(id: usize, stream: ElementStream<T>) -> Self {
        ExhaustiveDrain { id, stream }
    }
}

#[allow(dead_code)]
impl <T: Debug> Future for ExhaustiveDrain<T> {
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

#[allow(dead_code)]
pub struct ForeverDrain<T: Debug> {
    id: usize,
    stream: ElementStream<T>
}

#[allow(dead_code)]
impl<T: Debug> ForeverDrain<T> {
    pub fn new(id: usize, stream: ElementStream<T>) -> Self {
        ForeverDrain { id, stream }
    }
}

#[allow(dead_code)]
impl <T: Debug> Future for ForeverDrain<T> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        // println!("Drain #{} poll", self.id);

        loop {
            match self.stream.poll()? {
                Async::Ready(Some(value)) => {
                    println!("Drain #{} received value: {:?}", self.id, value);
                    // return Ok(Async::Ready(()))
                },
                Async::Ready(None) => {
                    // println!("Drain #{} received none. End of packet stream", self.id);
                    return Ok(Async::Ready(()))
                },
                Async::NotReady => {
                    // println!("Drain #{} received NotReady. Going to keep polling anyways", self.id);
                    // return Ok(Async::Ready(()))
                }
            }
        }
    }
}