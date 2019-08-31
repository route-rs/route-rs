use crate::link::PacketStream;
use futures::{Async, Future, Poll};

/// Link that drops all packets ingressed.
pub struct BlackHoleLink<Packet: Sized> {
    input_stream: PacketStream<Packet>,
}

impl<Packet: Sized> BlackHoleLink<Packet> {
    pub fn new(input_stream: PacketStream<Packet>) -> Self {
        BlackHoleLink { input_stream }
    }
}

impl<Packet: Sized> Future for BlackHoleLink<Packet> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            let input_packet_option: Option<Packet> = try_ready!(self.input_stream.poll());
            match input_packet_option {
                None => return Ok(Async::Ready(())),
                Some(_) => {}
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::Classifier;
    use crate::link::ClassifyLink;
    use crate::utils::test::packet_collectors::ExhaustiveCollector;
    use crate::utils::test::packet_generators::{immediate_stream, PacketIntervalGenerator};
    use core::time;
    use crossbeam::crossbeam_channel;
    use futures::future::lazy;

    struct ClassifyEvenness {}

    impl ClassifyEvenness {
        pub fn new() -> Self {
            ClassifyEvenness {}
        }
    }

    impl Classifier for ClassifyEvenness {
        type Packet = i32;
        type Class = bool;

        fn classify(&self, packet: &Self::Packet) -> Self::Class {
            packet % 2 == 0
        }
    }

    #[test]
    fn blackhole_link_finishes() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = immediate_stream(packets.clone());

        let link1 = BlackHoleLink::new(Box::new(packet_generator));

        tokio::run(link1);

        //In this test, we just ensure that it finishes.
    }

    #[test]
    fn blackhole_link_with_wait_finishes() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = PacketIntervalGenerator::new(
            time::Duration::from_millis(10),
            packets.clone().into_iter(),
        );

        let link1 = BlackHoleLink::new(Box::new(packet_generator));

        tokio::run(link1);

        //In this test, we just ensure that it finishes.
    }

    #[test]
    fn blackhole_odd_packets() {
        let default_channel_size = 10;
        let number_branches = 2;
        let packet_generator = immediate_stream(vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9]);

        let elem0 = ClassifyEvenness::new();

        let mut link0 = ClassifyLink::new(
            packet_generator,
            elem0,
            Box::new(|evenness| if evenness { 0 } else { 1 }),
            default_channel_size,
            number_branches,
        );

        let drain0 = link0.ingressor;

        let link1 = BlackHoleLink::new(Box::new(link0.egressors.pop().unwrap()));

        let (s0, link0_port0_collector_output) = crossbeam_channel::unbounded();
        let link0_port0_collector =
            ExhaustiveCollector::new(0, Box::new(link0.egressors.pop().unwrap()), s0);

        tokio::run(lazy(|| {
            tokio::spawn(drain0);
            tokio::spawn(link0_port0_collector);
            tokio::spawn(link1);
            Ok(())
        }));

        let elem0_port0_output: Vec<_> = link0_port0_collector_output.iter().collect();
        assert_eq!(elem0_port0_output, vec![0, 2, 420, 4, 6, 8]);
    }
}
