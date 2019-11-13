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
    use route_rs_packets::{EthernetFrame, Ipv4Packet};
    use route_rs_runtime::link::primitive::ProcessLink;
    use route_rs_runtime::link::{LinkBuilder, ProcessLinkBuilder};
    use route_rs_runtime::utils::test::harness::run_link;
    use route_rs_runtime::utils::test::packet_generators::immediate_stream;

    #[test]
    fn decap_ipv4() {
        let data: Vec<u8> = vec![
            0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 08, 00, //MAC layer
            0x45, 0, 0, 20, 0, 0, 0, 0, 64, 17, 0, 0, 192, 178, 128, 0, 10, 0, 0,
            1, //IP Layer
        ];
        let frame = EthernetFrame::new(data.clone(), 0).unwrap();
        let packets = vec![frame];

        let link = ProcessLink::new()
            .ingressor(immediate_stream(packets))
            .processor(Ipv4Decap)
            .build_link();

        let results = run_link(link);

        let test_packet = Ipv4Packet::new(data, Some(0), 14).unwrap();
        assert_eq!(results[0][0], test_packet);
    }

    #[test]
    fn encap_ipv4() {
        let data: Vec<u8> = vec![
            0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 08, 00, //MAC layer
            0x45, 0, 0, 20, 0, 0, 0, 0, 64, 17, 0, 0, 192, 178, 128, 0, 10, 0, 0,
            1, //IP Layer
        ];
        let packet = Ipv4Packet::new(data.clone(), Some(0), 14).unwrap();
        let packets = vec![packet];

        let link = ProcessLink::new()
            .ingressor(immediate_stream(packets))
            .processor(Ipv4Encap)
            .build_link();

        let results = run_link(link);

        let test_frame = EthernetFrame::new(data, 0).unwrap();
        assert_eq!(results[0][0], test_frame);
    }

    #[test]
    fn decap_ipv6() {
        let data: Vec<u8> = vec![
            0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0x86, 0xDD, //MAC layer
            0x60, 0, 0, 0, 0, 4, 17, 64, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef,
            0xde,0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13,
            14, 15, 0xa, 0xb, 0xc, 0xd,
        ];
        let frame = EthernetFrame::new(data.clone(), 0).unwrap();
        let packets = vec![frame];

        let link = ProcessLink::new()
            .ingressor(immediate_stream(packets))
            .processor(Ipv6Decap)
            .build_link();

        let results = run_link(link);

        let test_packet = Ipv6Packet::new(data, Some(0), 14).unwrap();
        assert_eq!(results[0][0], test_packet);
    }

    #[test]
    fn encap_ipv6() {
        let data: Vec<u8> = vec![
            0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0x86, 0xDD, //MAC layer
            0x60, 0, 0, 0, 0, 4, 17, 64, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef,
            0xde,0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13,
            14, 15, 0xa, 0xb, 0xc, 0xd,
        ];
        let packet = Ipv6Packet::new(data.clone(), Some(0), 14).unwrap();
        let packets = vec![packet];

        let link = ProcessLink::new()
            .ingressor(immediate_stream(packets))
            .processor(Ipv6Encap)
            .build_link();

        let results = run_link(link);

        let test_frame = EthernetFrame::new(data, 0).unwrap();
        assert_eq!(results[0][0], test_frame);
    }
}
