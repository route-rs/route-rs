use crate::element::Element;
use crate::packet::tuple::*;
use crate::state::*;
use smoltcp::wire::*;
use std::sync::Arc;

/// Rewrites outbound (lan to wan) Ipv4 packets based on entries in a Nat Table
/// Should be paired with a WanRewriteIpv4 with access to the same table
pub struct LanRewriterIpv4 {
    external_ip: Ipv4Address,
    nat_table: Arc<NatTable>,
}

impl LanRewriterIpv4 {
    pub fn new(external_ip: Ipv4Address, nat_table: Arc<NatTable>) -> LanRewriterIpv4 {
        LanRewriterIpv4 {
            external_ip,
            nat_table,
        }
    }

    fn get_tuple(&self, int_tuple: LookupTupleIpv4) -> LookupTupleIpv4 {
        if let Some(ext_tuple) = self.nat_table.get_external(&int_tuple) {
            ext_tuple.reverse_tuple()
        } else {
            let mut candidate_tuple = int_tuple.reverse_tuple_nat(self.external_ip);
            loop {
                // TODO: limit number of iterations, eventually give up and drop packet
                if !self.nat_table.contains_external(&candidate_tuple) {
                    if self
                        .nat_table
                        .insert(int_tuple.clone(), candidate_tuple.clone())
                        .is_ok()
                    {
                        break;
                    }
                }
                candidate_tuple.randomize_dst_port(); // Entry exists, try a different port
            }
            candidate_tuple.reverse_tuple()
        }
    }
}

impl Element for LanRewriterIpv4 {
    type Input = Ipv4Packet<Vec<u8>>;
    type Output = Ipv4Packet<Vec<u8>>;

    fn process(&mut self, mut packet: Ipv4Packet<Vec<u8>>) -> Ipv4Packet<Vec<u8>> {
        if let Ok(tuple) = LookupTupleIpv4::new_from_packet(&mut packet) {
            let rewrite_tuple = self.get_tuple(tuple);
            rewrite_tuple.rewrite_packet(&mut packet);
        };
        packet
    }
}

/// Rewrites inbound (wan to lan) Ipv4 packets based on entries in a Nat Table
/// Should be paired with a LanRewriteIpv4 with access to the same table
pub struct WanRewriterIpv4 {
    nat_table: Arc<NatTable>,
}

impl WanRewriterIpv4 {
    pub fn new(nat_table: Arc<NatTable>) -> WanRewriterIpv4 {
        WanRewriterIpv4 { nat_table }
    }

    fn get_tuple(&mut self, ext_tuple: LookupTupleIpv4) -> LookupTupleIpv4 {
        if let Some(int_tuple) = self.nat_table.get_internal(&ext_tuple) {
            int_tuple.reverse_tuple()
        } else {
            // TODO: somehow drop instead of passing through unchanged
            ext_tuple
        }
    }
}

impl Element for WanRewriterIpv4 {
    type Input = Ipv4Packet<Vec<u8>>;
    type Output = Ipv4Packet<Vec<u8>>;

    fn process(&mut self, mut packet: Ipv4Packet<Vec<u8>>) -> Ipv4Packet<Vec<u8>> {
        if let Ok(tuple) = LookupTupleIpv4::new_from_packet(&mut packet) {
            let rewrite_tuple = self.get_tuple(tuple);
            rewrite_tuple.rewrite_packet(&mut packet);
        };
        packet
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smoltcp::phy::ChecksumCapabilities;

    fn ipv4_tcp_packet(ip_tuple: &LookupTupleIpv4) -> Ipv4Packet<Vec<u8>> {
        let tcp_hdr = TcpRepr {
            src_port: ip_tuple.src_port,
            dst_port: ip_tuple.dst_port,
            control: TcpControl::None,
            seq_number: Default::default(),
            ack_number: None,
            window_len: 0,
            window_scale: None,
            max_seg_size: None,
            payload: &[],
        };
        let ip_hdr = Ipv4Repr {
            src_addr: ip_tuple.src_ip,
            dst_addr: ip_tuple.dst_ip,
            protocol: IpProtocol::Tcp,
            payload_len: tcp_hdr.buffer_len(),
            hop_limit: 64,
        };

        let buffer = vec![0; ip_hdr.buffer_len() + ip_hdr.payload_len];
        let mut packet = Ipv4Packet::new_unchecked(buffer);
        ip_hdr.emit(&mut packet, &ChecksumCapabilities::default());
        let mut tcp_pkt = TcpPacket::new_unchecked(packet.payload_mut());
        tcp_hdr.emit(
            &mut tcp_pkt,
            &ip_tuple.src_ip.into(),
            &ip_tuple.dst_ip.into(),
            &ChecksumCapabilities::default(),
        );
        packet
    }

    #[test]
    fn test_nat_bidirectional_flow() {
        let table = Arc::new(NatTable::new());
        let external_ip = Ipv4Address::new(9, 0, 0, 1);
        let mut lan_rw = LanRewriterIpv4::new(external_ip, Arc::clone(&table));
        let mut wan_rw = WanRewriterIpv4::new(Arc::clone(&table));

        let internal_tuple = LookupTupleIpv4 {
            proto: IpProtocol::Tcp,
            src_ip: Ipv4Address::new(10, 0, 0, 2),
            dst_ip: Ipv4Address::new(8, 8, 8, 8),
            src_port: 1025,
            dst_port: 80,
        };

        // Check that first packet gets through and gets rewritten
        let out1_internal = ipv4_tcp_packet(&internal_tuple);
        let mut out1_external = lan_rw.process(out1_internal);

        let out1_external = LookupTupleIpv4::new_from_packet(&mut out1_external).unwrap();
        assert_eq!(internal_tuple.dst_ip, out1_external.dst_ip);
        assert_eq!(external_ip, out1_external.src_ip);
        assert_eq!(internal_tuple.proto, out1_external.proto);
        assert_eq!(internal_tuple.dst_port, out1_external.dst_port);

        let mut external_tuple = out1_external.reverse_tuple();
        assert!(table.contains_internal(&internal_tuple));
        assert!(table.contains_external(&external_tuple));

        // Check that return packet is properly returned to original LAN IP
        let in1_external = ipv4_tcp_packet(&external_tuple);
        let mut in1_internal = wan_rw.process(in1_external);
        let in1_internal = LookupTupleIpv4::new_from_packet(&mut in1_internal).unwrap();
        assert_eq!(internal_tuple.reverse_tuple(), in1_internal);

        // Check that second outbound packet works too (different code path because it uses an existing table entry
        let out2_internal = ipv4_tcp_packet(&internal_tuple);
        let mut out2_external = lan_rw.process(out2_internal);
        let out2_external = LookupTupleIpv4::new_from_packet(&mut out2_external).unwrap();
        assert_eq!(out1_external, out2_external);
    }

    // TODO: test that inbound packets w/o matching rule are dropped
}
