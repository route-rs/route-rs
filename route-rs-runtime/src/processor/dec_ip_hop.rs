use crate::processor::Processor;
use route_rs_packets::{Ipv4Packet, Ipv6Packet};

/// Decrements the TTL of an IPv4 packet
#[derive(Default)]
pub struct DecIpv4HopLimit {}

impl DecIpv4HopLimit {
    pub fn new() -> DecIpv4HopLimit {
        DecIpv4HopLimit {}
    }
}

impl Processor for DecIpv4HopLimit {
    type Input = Ipv4Packet;
    type Output = Ipv4Packet;

    fn process(&mut self, mut packet: Self::Input) -> Option<Self::Output> {
        match packet.ttl() {
            0 => Some(packet),
            ttl => {
                packet.set_ttl(ttl - 1);
                Some(packet)
            }
        }
    }
}

/// Decrements the TTL of an IPv6 packet
#[derive(Default, Clone)]
pub struct DecIpv6HopLimit {}

impl DecIpv6HopLimit {
    pub fn new() -> DecIpv6HopLimit {
        DecIpv6HopLimit {}
    }
}

impl Processor for DecIpv6HopLimit {
    type Input = Ipv6Packet;
    type Output = Ipv6Packet;

    fn process(&mut self, mut packet: Self::Input) -> Option<Self::Output> {
        match packet.hop_limit() {
            0 => Some(packet),
            ttl => {
                packet.set_hop_limit(ttl - 1);
                Some(packet)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use route_rs_packets::EthernetFrame;
    use std::convert::TryFrom;

    #[test]
    fn test_dec_ipv4_hop_limit() {
        let init_ttl = 64;
        let mac_data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let ip_data: Vec<u8> = vec![
            0x45, 0, 0, 20, 0, 0, 0, 0, 64, 17, 0, 0, 192, 178, 128, 0, 10, 0, 0, 1,
        ];

        let mut frame = EthernetFrame::from_buffer(mac_data, 0).unwrap();
        frame.set_payload(&ip_data);

        let mut packet = Ipv4Packet::try_from(frame).unwrap();
        packet.set_ttl(init_ttl);

        let mut elem = DecIpv4HopLimit::new();

        let packet = elem.process(packet).unwrap();

        assert_eq!(packet.ttl(), init_ttl - 1);
    }

    #[test]
    fn test_dec_ipv4_hop_limit_expired() {
        let init_ttl = 0;
        let mac_data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let ip_data: Vec<u8> = vec![
            0x45, 0, 0, 20, 0, 0, 0, 0, 64, 17, 0, 0, 192, 178, 128, 0, 10, 0, 0, 1,
        ];

        let mut frame = EthernetFrame::from_buffer(mac_data, 0).unwrap();
        frame.set_payload(&ip_data);

        let mut packet = Ipv4Packet::try_from(frame).unwrap();
        packet.set_ttl(init_ttl);

        let mut elem = DecIpv4HopLimit::new();

        let packet = elem.process(packet).unwrap();

        assert_eq!(packet.ttl(), 0);
    }

    #[test]
    fn test_dec_ipv6_hop_limit() {
        let init_ttl = 64;
        let mac_data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let ip_data: Vec<u8> = vec![
            0x60, 0, 0, 0, 0, 4, 17, 64, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde,
            0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13,
            14, 15, 0xa, 0xb, 0xc, 0xd,
        ];

        let mut frame = EthernetFrame::from_buffer(mac_data, 0).unwrap();
        frame.set_payload(&ip_data);

        let mut packet = Ipv6Packet::try_from(frame).unwrap();
        packet.set_hop_limit(init_ttl);

        let mut elem = DecIpv6HopLimit::new();

        let packet = elem.process(packet).unwrap();

        assert_eq!(packet.hop_limit(), init_ttl - 1);
    }

    #[test]
    fn test_dec_ipv6_hop_limit_expired() {
        let init_ttl = 0;
        let mac_data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let ip_data: Vec<u8> = vec![
            0x60, 0, 0, 0, 0, 4, 17, 64, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde,
            0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13,
            14, 15, 0xa, 0xb, 0xc, 0xd,
        ];

        let mut frame = EthernetFrame::from_buffer(mac_data, 0).unwrap();
        frame.set_payload(&ip_data);

        let mut packet = Ipv6Packet::try_from(frame).unwrap();
        packet.set_hop_limit(init_ttl);

        let mut elem = DecIpv6HopLimit::new();

        let packet = elem.process(packet).unwrap();

        assert_eq!(packet.hop_limit(), 0);
    }
}
