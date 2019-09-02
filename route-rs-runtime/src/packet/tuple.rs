use smoltcp::wire::*;

/// The 5-tuple of an IPv4 packet commonly used for NAT translation, firewall rules, etc.
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
            },
            IpProtocol::Udp => {
                if let Ok(udp_packet) = UdpPacket::new_checked(packet.payload_mut()) {
                    (udp_packet.src_port(), udp_packet.dst_port())
                } else {
                    return Err("Invalid UDP packet");
                }
            },
            _ => return Err("Unsupported IP protocol"),
        };
        Ok(LookupTupleIpv4{proto, src_ip, dst_ip, src_port, dst_port})
    }
}