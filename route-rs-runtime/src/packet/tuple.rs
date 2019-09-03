use smoltcp::wire::*;
use std::hash::{Hash, Hasher};

impl Hash for LookupTupleIpv4 {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(self.proto.into());
        self.src_ip.hash(state);
        self.dst_ip.hash(state);
        self.src_port.hash(state);
        self.dst_port.hash(state);
    }
}

/// The 5-tuple of an IPv4 packet commonly used for NAT translation, firewall rules, etc.
#[derive(PartialEq, Eq, Clone)]
pub struct LookupTupleIpv4 {
    proto: IpProtocol,
    src_ip: Ipv4Address,
    dst_ip: Ipv4Address,
    src_port: u16,
    dst_port: u16,
}

impl LookupTupleIpv4 {
    /// Takes a packet and returns a result with the 5-tuple if the packet is a valid TCP or UDP, Err
    /// otherwise (ICMP support will be added eventually...)
    pub fn new(packet: &mut Ipv4Packet<Vec<u8>>) -> Result<LookupTupleIpv4, &'static str> {
        let src_ip = packet.src_addr();
        let dst_ip = packet.dst_addr();
        let proto = packet.protocol();

        let (src_port, dst_port) = match proto {
            IpProtocol::Tcp => {
                if let Ok(tcp_packet) = TcpPacket::new_checked(packet.payload_mut()) {
                    (tcp_packet.src_port(), tcp_packet.dst_port())
                } else {
                    return Err("Invalid TCP packet");
                }
            }
            IpProtocol::Udp => {
                if let Ok(udp_packet) = UdpPacket::new_checked(packet.payload_mut()) {
                    (udp_packet.src_port(), udp_packet.dst_port())
                } else {
                    return Err("Invalid UDP packet");
                }
            }
            _ => return Err("Unsupported IP protocol"),
        };
        Ok(LookupTupleIpv4 {
            proto,
            src_ip,
            dst_ip,
            src_port,
            dst_port,
        })
    }

    pub fn rewrite_packet(&self, packet: &mut Ipv4Packet<Vec<u8>>) {
        // This may change when we add ICMP support
        assert_eq!(
            self.proto,
            packet.protocol(),
            "Packet and Tuple protocol must match!"
        );

        packet.set_dst_addr(self.dst_ip);
        packet.set_src_addr(self.src_ip);

        match packet.protocol() {
            IpProtocol::Tcp => {
                if let Ok(mut tcp_packet) = TcpPacket::new_checked(packet.payload_mut()) {
                    tcp_packet.set_dst_port(self.dst_port);
                    tcp_packet.set_src_port(self.src_port);
                }
            }
            IpProtocol::Udp => {
                if let Ok(mut udp_packet) = UdpPacket::new_checked(packet.payload_mut()) {
                    udp_packet.set_dst_port(self.dst_port);
                    udp_packet.set_src_port(self.src_port);
                }
            }
            _ => panic!("Unsupported IP protocol"),
        }
    }
}
