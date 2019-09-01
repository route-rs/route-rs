use smoltcp::phy::ChecksumCapabilities;
use smoltcp::wire::*;
use std::cmp;

/// The size of an ICMP "header", which precedes the data field
const ICMP_HDR_LEN: usize = 8;

/// Helper to generate ICMPv4 error messages
pub struct Icmpv4ErrorGenerator {
    src_ip: Ipv4Address,
    mtu: usize,
    ttl: u8,
    bad_source_addrs: Vec<Ipv4Address>,
}

impl Icmpv4ErrorGenerator {
    /// Create an ICMPv4 error generator
    ///
    /// # Arguments
    ///
    /// * `src_ip` - The source IPv4 address of the generated errors
    /// * `bad_source_addrs` - Packets from these IP addresses will generate a None instead of an
    /// an ICMP error. Intended to contain the broadcast address of a subnet
    pub fn new(src_ip: Ipv4Address, bad_source_addrs: Vec<Ipv4Address>) -> Icmpv4ErrorGenerator {
        Icmpv4ErrorGenerator {
            src_ip,
            mtu: IPV4_MIN_MTU,
            ttl: 64, // TODO: larger to ensure it reaches original host?
            bad_source_addrs,
        }
    }

    /// Create an ICMPv4 TTL exceeded error message. Return `None` if an ICMP error message should
    /// not be generated, and the packet should be silently discarded instead
    ///
    /// # Arguments
    ///
    /// * `packet` - The offending packet, the contents of which will be copied into the returned
    /// error packet
    pub fn ttl_exceeded_error(
        &self,
        packet: &mut Ipv4Packet<Vec<u8>>,
    ) -> Option<Ipv4Packet<Vec<u8>>> {
        self.generic_error(
            packet,
            Icmpv4Message::TimeExceeded,
            Icmpv4TimeExceeded::TtlExpired.into(),
            &[0; 4],
        )
    }

    // Common way of constructing and ICMPv4 error message, including the 8 byte ICMP header and
    // as many bytes as possible of the offending IPv4 packet up to the configured MTU size
    fn generic_error(
        &self,
        packet: &mut Ipv4Packet<Vec<u8>>,
        msg_type: Icmpv4Message,
        msg_code: u8,
        _optional: &[u8; 4],
    ) -> Option<Ipv4Packet<Vec<u8>>> {
        if !self.should_generate_error(packet) {
            return None;
        }

        assert!(
            Icmpv4ErrorGenerator::is_error_type(msg_type),
            "Provided ICMP message type must be an error type"
        );

        // TODO: this will need to vary if we add support for source routing
        // The size of the IPv4 header plus the overhead for the ICMP header
        let min_header_size = 20 + ICMP_HDR_LEN;

        // How much of the offending packet will be copied into the ICMP error
        let copied_payload_len = cmp::min(packet.total_len() as usize, self.mtu - min_header_size);

        let error_pkt_repr = Ipv4Repr {
            src_addr: self.src_ip,
            dst_addr: packet.src_addr(),
            protocol: IpProtocol::Icmp,
            payload_len: copied_payload_len + ICMP_HDR_LEN,
            hop_limit: self.ttl,
        };

        // Generate the layer 3 header
        let buffer: Vec<u8> = vec![0x00; error_pkt_repr.buffer_len() + error_pkt_repr.payload_len];
        let mut error_pkt = Ipv4Packet::new_unchecked(buffer);
        error_pkt_repr.emit(&mut error_pkt, &ChecksumCapabilities::ignored());
        error_pkt.set_dscp(packet.dscp());
        error_pkt.fill_checksum();

        // Generate the ICMP message
        let mut icmp_error = Icmpv4Packet::new_unchecked(error_pkt.payload_mut());
        icmp_error.set_msg_type(msg_type);
        icmp_error.set_msg_code(msg_code);
        // TODO: fill in bytes 5 - 8 with _optional data for ICMP errors that require it
        icmp_error
            .data_mut()
            .clone_from_slice(&packet.clone().into_inner()[..copied_payload_len]);
        icmp_error.fill_checksum();

        Some(error_pkt)
    }

    // Performs checks based on RFC 1812 4.3.2.7 (When Not to Send ICMP Errors)
    fn should_generate_error(&self, packet: &mut Ipv4Packet<Vec<u8>>) -> bool {
        // Only the first fragment
        if packet.frag_offset() != 0 {
            return false;
        }
        // Avoid infinite loops, no errors from errors
        if packet.protocol() == IpProtocol::Icmp {
            if let Ok(icmp_packet) = Icmpv4Packet::new_checked(packet.payload_mut()) {
                if Icmpv4ErrorGenerator::is_error_type(icmp_packet.msg_type()) {
                    return false;
                }
            } else {
                return false;
            }
        }
        // No broadcast or loopback addresses
        if !packet.src_addr().is_unicast() || packet.src_addr().is_loopback() {
            return false;
        }
        if self.bad_source_addrs.contains(&packet.src_addr()) {
            return false;
        }
        /*
         TODO: Check link-layer is not broadcast or multicast (may need Click-style annotations),
            or guarantee link-layer broadcast frames have broad/multicast L3 addresses earlier in the path
        */
        return true;
    }

    fn is_error_type(msg: Icmpv4Message) -> bool {
        let error_types = vec![
            Icmpv4Message::DstUnreachable,
            Icmpv4Message::Redirect,
            Icmpv4Message::TimeExceeded,
            Icmpv4Message::ParamProblem,
        ];
        error_types.contains(&msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_generator() -> Icmpv4ErrorGenerator {
        let own_addr = Ipv4Address::new(10, 0, 0, 1);
        let bad_ips = vec![Ipv4Address::new(10, 0, 0, 255)];
        Icmpv4ErrorGenerator::new(own_addr, bad_ips)
    }

    #[test]
    fn test_icmp_error_ttl_exceeded() {
        let generator = create_generator();

        let repr = Ipv4Repr {
            src_addr: Ipv4Address::new(10, 0, 0, 2),
            dst_addr: Ipv4Address::new(10, 0, 0, 3),
            protocol: IpProtocol::Tcp,
            payload_len: 10,
            hop_limit: 0,
        };
        let orig_len = repr.buffer_len() + repr.payload_len;
        let buffer = vec![0; orig_len];
        let mut packet = Ipv4Packet::new_unchecked(buffer);
        repr.emit(&mut packet, &ChecksumCapabilities::ignored());
        let dscp = 40;
        packet.set_dscp(dscp);
        packet.fill_checksum();

        let icmp_msg = generator.ttl_exceeded_error(&mut packet).unwrap();

        assert!(icmp_msg.check_len().is_ok());
        assert!(icmp_msg.verify_checksum());
        assert_eq!(
            icmp_msg.total_len() as usize,
            icmp_msg.header_len() as usize + ICMP_HDR_LEN + packet.total_len() as usize
        );
        assert_eq!(icmp_msg.dscp(), dscp);
    }

    #[test]
    fn test_icmp_error_truncated_packet() {
        let generator = create_generator();

        let repr = Ipv4Repr {
            src_addr: Ipv4Address::new(10, 0, 0, 2),
            dst_addr: Ipv4Address::new(10, 0, 0, 3),
            protocol: IpProtocol::Tcp,
            payload_len: 1000,
            hop_limit: 0,
        };
        let orig_len = repr.buffer_len() + repr.payload_len;
        let buffer = vec![0; orig_len];
        let mut packet = Ipv4Packet::new_unchecked(buffer);
        repr.emit(&mut packet, &ChecksumCapabilities::default());

        let icmp_msg = generator.ttl_exceeded_error(&mut packet).unwrap();

        assert!(icmp_msg.check_len().is_ok());
        assert!(icmp_msg.verify_checksum());
        assert_eq!(icmp_msg.total_len() as usize, IPV4_MIN_MTU);
    }

    #[test]
    fn test_icmp_error_bad_address() {
        let generator = create_generator();

        let repr = Ipv4Repr {
            src_addr: generator.bad_source_addrs[0],
            dst_addr: Ipv4Address::new(10, 0, 0, 3),
            protocol: IpProtocol::Tcp,
            payload_len: 1000,
            hop_limit: 0,
        };
        let orig_len = repr.buffer_len() + repr.payload_len;
        let buffer = vec![0; orig_len];
        let mut packet = Ipv4Packet::new_unchecked(buffer);
        repr.emit(&mut packet, &ChecksumCapabilities::default());

        assert_eq!(generator.ttl_exceeded_error(&mut packet), None);
    }

    #[test]
    fn test_no_icmp_error_from_icmp_error() {
        let generator = create_generator();

        // Payload of the packet that caused the destination unreachable message
        let old_pkt_repr = Ipv4Repr {
            src_addr: Ipv4Address::new(10, 0, 0, 3),
            dst_addr: Ipv4Address::new(10, 0, 0, 2),
            protocol: IpProtocol::Tcp,
            payload_len: 6,
            hop_limit: 0,
        };
        let icmp_unreachable_repr = Icmpv4Repr::DstUnreachable {
            reason: Icmpv4DstUnreachable::HostUnreachable,
            header: old_pkt_repr,
            data: &vec![1, 2, 3, 4, 5, 6],
        };
        let resp_pkt_repr = Ipv4Repr {
            src_addr: Ipv4Address::new(10, 0, 0, 2),
            dst_addr: Ipv4Address::new(10, 0, 0, 3),
            protocol: IpProtocol::Icmp,
            payload_len: icmp_unreachable_repr.buffer_len(),
            hop_limit: 0,
        };

        let buffer = vec![0x00; icmp_unreachable_repr.buffer_len() + resp_pkt_repr.buffer_len()];
        let mut resp_packet = Ipv4Packet::new_unchecked(buffer);
        resp_pkt_repr.emit(&mut resp_packet, &ChecksumCapabilities::default());

        let mut icmp_unreachable = Icmpv4Packet::new_unchecked(resp_packet.payload_mut());
        icmp_unreachable_repr.emit(&mut icmp_unreachable, &ChecksumCapabilities::default());

        assert_eq!(generator.ttl_exceeded_error(&mut resp_packet), None);
    }
}
