use crate::element::Element;
use smoltcp::wire::*;

/// Decrements the TTL of an IPv4 packet
#[derive(Default)]
pub struct DecIpv4HopLimit {}

impl DecIpv4HopLimit {
    pub fn new() -> DecIpv4HopLimit {
        DecIpv4HopLimit {}
    }
}

impl Element for DecIpv4HopLimit {
    type Input = Ipv4Packet<Vec<u8>>;
    type Output = Ipv4Packet<Vec<u8>>;

    fn process(&mut self, mut packet: Self::Input) -> Self::Output {
        match packet.hop_limit() {
            0 => packet,
            ttl => {
                packet.set_hop_limit(ttl - 1);
                packet
            }
        }
    }
}

/// Decrements the TTL of an IPv6 packet
#[derive(Default)]
pub struct DecIpv6HopLimit {}

impl DecIpv6HopLimit {
    pub fn new() -> DecIpv6HopLimit {
        DecIpv6HopLimit {}
    }
}

impl Element for DecIpv6HopLimit {
    type Input = Ipv6Packet<Vec<u8>>;
    type Output = Ipv6Packet<Vec<u8>>;

    fn process(&mut self, mut packet: Self::Input) -> Self::Output {
        match packet.hop_limit() {
            0 => packet,
            ttl => {
                packet.set_hop_limit(ttl - 1);
                packet
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smoltcp::phy::ChecksumCapabilities;

    #[test]
    fn test_dec_ipv4_hop_limit() {
        let init_ttl = 64;
        let repr = Ipv4Repr {
            src_addr: Ipv4Address::new(10, 0, 0, 1),
            dst_addr: Ipv4Address::new(10, 0, 0, 2),
            protocol: IpProtocol::Tcp,
            payload_len: 10,
            hop_limit: init_ttl,
        };
        let buffer = vec![0; repr.buffer_len() + repr.payload_len];
        let mut packet = Ipv4Packet::new_unchecked(buffer);
        repr.emit(&mut packet, &ChecksumCapabilities::default());

        let mut elem = DecIpv4HopLimit::new();

        let packet = elem.process(packet);

        assert_eq!(packet.hop_limit(), init_ttl - 1);
    }

    #[test]
    fn test_dec_ipv4_hop_limit_expired() {
        let repr = Ipv4Repr {
            src_addr: Ipv4Address::new(10, 0, 0, 1),
            dst_addr: Ipv4Address::new(10, 0, 0, 2),
            protocol: IpProtocol::Tcp,
            payload_len: 10,
            hop_limit: 0,
        };
        let buffer = vec![0; repr.buffer_len() + repr.payload_len];
        let mut packet = Ipv4Packet::new_unchecked(buffer);
        repr.emit(&mut packet, &ChecksumCapabilities::default());

        let mut elem = DecIpv4HopLimit::new();

        let packet = elem.process(packet);

        assert_eq!(packet.hop_limit(), 0);
    }

    #[test]
    fn test_dec_ipv6_hop_limit() {
        let init_ttl = 64;
        let repr = Ipv6Repr {
            src_addr: Ipv6Address::new(0xfdaa, 0, 0, 0, 0, 0, 0, 1),
            dst_addr: Ipv6Address::new(0xfdaa, 0, 0, 0, 0, 0, 0, 1),
            next_header: IpProtocol::Tcp,
            payload_len: 10,
            hop_limit: init_ttl,
        };
        let buffer = vec![0; repr.buffer_len() + repr.payload_len];
        let mut packet = Ipv6Packet::new_unchecked(buffer);
        repr.emit(&mut packet);

        let mut elem = DecIpv6HopLimit::new();

        let packet = elem.process(packet);

        assert_eq!(packet.hop_limit(), init_ttl - 1);
    }

    #[test]
    fn test_dec_ipv6_hop_limit_expired() {
        let repr = Ipv6Repr {
            src_addr: Ipv6Address::new(0xfdaa, 0, 0, 0, 0, 0, 0, 1),
            dst_addr: Ipv6Address::new(0xfdaa, 0, 0, 0, 0, 0, 0, 1),
            next_header: IpProtocol::Tcp,
            payload_len: 10,
            hop_limit: 0,
        };
        let buffer = vec![0; repr.buffer_len() + repr.payload_len];
        let mut packet = Ipv6Packet::new_unchecked(buffer);
        repr.emit(&mut packet);

        let mut elem = DecIpv6HopLimit::new();

        let packet = elem.process(packet);

        assert_eq!(packet.hop_limit(), 0);
    }
}
