use route_rs_packets::{EthernetFrame, Ipv4Packet, Ipv6Packet};
use route_rs_runtime::processor::Processor;
use std::convert::TryFrom;

pub struct Ipv6Decap;

impl Processor for Ipv6Decap {
    type Input = EthernetFrame;
    type Output = Ipv6Packet;

    fn process(&mut self, frame: Self::Input) -> Option<Self::Output> {
        match Ipv6Packet::try_from(frame) {
            Ok(packet) => Some(packet),
            Err(_) => None,
        }
    }
}

pub struct Ipv6Encap;

impl Processor for Ipv6Encap {
    type Input = Ipv6Packet;
    type Output = EthernetFrame;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        match EthernetFrame::try_from(packet) {
            Ok(frame) => Some(frame),
            Err(_) => None,
        }
    }
}

pub struct Ipv4Decap;

impl Processor for Ipv4Decap {
    type Input = EthernetFrame;
    type Output = Ipv4Packet;

    fn process(&mut self, frame: Self::Input) -> Option<Self::Output> {
        match Ipv4Packet::try_from(frame) {
            Ok(packet) => Some(packet),
            Err(_) => None,
        }
    }
}

pub struct Ipv4Encap;

impl Processor for Ipv4Encap {
    type Input = Ipv4Packet;
    type Output = EthernetFrame;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        match EthernetFrame::try_from(packet) {
            Ok(frame) => Some(frame),
            Err(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use route_rs_packets::EthernetFrame;
    use route_rs_runtime::link::primitive::ProcessLink;
    use route_rs_runtime::link::{LinkBuilder, ProcessLinkBuilder};
    use route_rs_runtime::utils::test::harness::run_link;
    use route_rs_runtime::utils::test::packet_generators::immediate_stream;

    #[test]
    fn decap_ipv4() {
        let data: Vec<u8> = vec![
            0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 08, 00, //MAC layer
            0x45, 0, 0, 20, 0, 0, 0, 0, 64, 17, 0, 0, 192, 178, 128, 0, 10, 0, 0, 1, //IPLayer
        ];
        let frame = EthernetFrame::new(data.clone(), 0);
        let packets = vec![frame.unwrap()];

        let link = ProcessLink::new()
            .ingressor(immediate_stream(packets))
            .processor(Ipv4Decap)
            .build_link();

        let results = run_link(link);

        // I should implement Eq for all the packets, basically just check if they have the same underlying data.
        assert_eq!(results[0].len(), 1);
    }
}
