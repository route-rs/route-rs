use futures::{Stream, Async, Poll};
use tokio::timer::Interval;
use std::time::Duration;

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
