use crate::packet::*;
use std::borrow::Cow;

#[allow(dead_code)]
pub struct Ipv6Packet<'packet> {
    pub data: PacketData<'packet>,
    // There may be various "Extension Headers", so we should figure out the actual offset and store it here for
    // easy access in the helper functions.
    payload_offset: usize,
}

impl<'packet> Ipv6Packet<'packet> {
    fn new(packet: PacketData) -> Result<Ipv6Packet, &'static str> {
        //Header of Ethernet Frame: 14bytes
        //Haeder of IPv6 Frame: 40bytes minimum
        if packet.len() < 14 + 40 {
            return Err("Packet is too short to be an Ipv6Packet");
        }

        //Check version number
        let version = (packet[14] & 0xF0) >> 4;
        if version != 6 {
            return Err("Packet has incorrect version, is not Ipv6Packet");
        }

        // Note, there is a special unhandled edge case here, if the payload len is 0, there may
        // be a hop-by-hop extension header, that means we may have a jumbo packet. Not going to
        // handle this edge case for now, but it is needed before shipping.
        // This may also not be true if there are extension headers, so we just check that we will not
        // overrun our array trying to access the entire payload.
        let payload_len = u16::from_be_bytes([packet[14 + 4], packet[14 + 5]]) as usize;
        let packet_len = packet.len();
        if payload_len + 14 + 40 > packet_len {
            return Err("Packet has invalid payload len field");
        }

        Ok(Ipv6Packet {
            data: packet,
            payload_offset: packet_len - payload_len,
        })
    }

    pub fn traffic_class(&self) -> u8 {
        ((self.data[14] & 0x0F) << 4) + (self.data[14 + 1] & 0xF0 >> 4)
    }

    pub fn flow_label(&self) -> u32 {
        u32::from_be_bytes([
            0,
            self.data[14 + 1] & 0x0F,
            self.data[14 + 2],
            self.data[14 + 3],
        ])
    }

    pub fn payload_length(&self) -> u16 {
        u16::from_be_bytes([self.data[14 + 4], self.data[14 + 5]])
    }

    pub fn next_header(&self) -> IpProtocol {
        IpProtocol::from(self.data[14 + 6])
    }

    pub fn set_next_header(&mut self, header: u8) {
        self.data[14+6] = header;
    }

    pub fn hop_limit(&self) -> u8 {
        self.data[14 + 7]
    }

    pub fn payload(&self) -> Cow<[u8]> {
        Cow::from(&self.data[self.payload_offset..])
    }

    pub fn set_payload(&mut self, payload: &[u8]) {
        let new_payload_len = payload.len();
        self.data.truncate(self.payload_offset);

        let payload_len = (new_payload_len as u16).to_be_bytes();
        self.data[14 + 4..14 + 6].copy_from_slice(&payload_len);

        self.data.reserve_exact(new_payload_len);
        self.data.extend(payload);
        self.payload_offset = self.data.len() - payload.len();
    }

    pub fn src_addr(&self) -> Ipv6Addr {
        Ipv6Addr::from_byte_slice(&self.data[14 + 8..14 + 24]).unwrap()
    }

    pub fn dest_addr(&self) -> Ipv6Addr {
        Ipv6Addr::from_byte_slice(&self.data[14 + 24..14 + 40]).unwrap()
    }

    pub fn set_src_addr(&mut self, addr: Ipv6Addr) {
        self.data[14 + 8..14 + 24].copy_from_slice(&addr.bytes()[..]);
    }

    pub fn set_dest_addr(&mut self, addr: Ipv6Addr) {
        self.data[14 + 24..14 + 40].copy_from_slice(&addr.bytes()[..]);
    }

    //TODO: Test the get and set for extension headers.
    pub fn extension_headers(&self) -> Vec<Cow<[u8]>> {
        let mut headers = Vec::<Cow<[u8]>>::new();
        let mut next_header = self.next_header();
        let mut header_ext_len;
        let mut offset = 14 + 40; //First byte of first header
        loop {
            match next_header {
                IpProtocol::HOPOPT
                | IpProtocol::IPv6_Opts
                | IpProtocol::IPv6_route
                | IpProtocol::IPv6_frag
                | IpProtocol::AH
                | IpProtocol::ESP
                | IpProtocol::Mobility_Header
                | IpProtocol::HIP
                | IpProtocol::Shim6
                | IpProtocol::Use_for_expiramentation_and_testing => {
                    header_ext_len = self.data[offset + 1];
                    if header_ext_len == 0 {
                        //fragments have the minimum of 8, but it set to zero for some dumb reason
                        header_ext_len = 8;
                    }
                    headers.push(Cow::from(
                        &self.data[offset..offset + header_ext_len as usize],
                    ));
                    next_header = IpProtocol::from(self.data[offset]);
                    offset += header_ext_len as usize;
                }
                _ => {
                    return headers;
                }
            }
        }
    }

    /// This function sets new extension headers. Because we are inserting into the middle
    /// of the vector, this is not a particularly performant operation. The caller should prvoide
    /// both the vector of headers, the type of the first header, as an IpProtocol. The caller is also
    /// required to ensure that the next_header field of their last extension header is a set to the 
    /// IpProtocol of the payload.
    /// If the provided vector does not contain any headers, the header field is cleared, and the
    /// first_header field should be the IpProtocol of the payload.
    pub fn set_extension_headers(&mut self, headers: Vec<&[u8]>, first_header: IpProtocol) {
        let payload = self.data.split_off(self.payload_offset);
        self.data.truncate(14+40);
        for header in headers.iter() {
            self.data.extend(*header);
        }
        self.data.extend(payload);
        if headers.len() > 0 {
            self.set_next_header(first_header as u8);
        }
    }
}

pub type Ipv6PacketResult<'packet> = Result<Ipv6Packet<'packet>, &'static str>;

impl<'packet> From<EthernetFrame<'packet>> for Ipv6PacketResult<'packet> {
    fn from(frame: EthernetFrame<'packet>) -> Self {
        Ipv6Packet::new(frame.data)
    }
}

impl<'packet> From<TcpSegment<'packet>> for Ipv6PacketResult<'packet> {
    fn from(segment: TcpSegment<'packet>) -> Self {
        Ipv6Packet::new(segment.data)
    }
}

impl<'packet> From<UdpSegment<'packet>> for Ipv6PacketResult<'packet> {
    fn from(segment: UdpSegment<'packet>) -> Self {
        Ipv6Packet::new(segment.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    #[test]
    fn ipv6_packet() {
        let mut mac_data: Vec<u8> =
            vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let ip_data: Vec<u8> = vec![
            0x60, 0, 0, 0, 0, 4, 17, 64, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde,
            0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13,
            14, 15, 0xa, 0xb, 0xc, 0xd,
        ];

        let src_addr = Ipv6Addr::new([
            0xdead, 0xbeef, 0xdead, 0xbeef, 0xdead, 0xbeef, 0xdead, 0xbeef,
        ]);
        let dest_addr = Ipv6Addr::new([
            0x0001, 0x0203, 0x0405, 0x0607, 0x0809, 0x0A0B, 0x0C0D, 0x0E0F,
        ]);

        let mut frame = EthernetFrame::new(&mut mac_data).unwrap();
        frame.set_payload(&ip_data);

        let packet = Ipv6PacketResult::from(frame).unwrap();
        assert_eq!(packet.traffic_class(), 0);
        assert_eq!(packet.flow_label(), 0);
        assert_eq!(packet.payload_length(), 4);
        assert_eq!(packet.next_header(), IpProtocol::UDP);
        assert_eq!(packet.hop_limit(), 64);
        assert_eq!(packet.payload().len(), 4);
        assert_eq!(packet.payload()[2], 0xc);
        assert_eq!(packet.src_addr(), src_addr);
        assert_eq!(packet.dest_addr(), dest_addr);
    }

    #[test]
    fn set_src_addr() {
        let mut data: Vec<u8> = vec![
            0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0, 0x60, 0, 0, 0, 0, 4, 17,
            64, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
            0xbe, 0xef, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0xa, 0xb, 0xc, 0xd,
        ];

        let src_addr = Ipv6Addr::new([
            0xdead, 0xbeef, 0xdead, 0xbeef, 0xdead, 0xbeef, 0xdead, 0xbeef,
        ]);

        let new_src_addr = Ipv6Addr::new([
            0x0001, 0x0203, 0x0405, 0x0607, 0x0809, 0x0A0B, 0x0C0D, 0x0E0F,
        ]);

        let mut packet = Ipv6Packet::new(&mut data).unwrap();

        assert_eq!(packet.src_addr(), src_addr);
        packet.set_src_addr(new_src_addr.clone());
        assert_eq!(packet.src_addr(), new_src_addr);
    }

    #[test]
    fn set_dest_addr() {
        let mut data: Vec<u8> = vec![
            0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0, 0x60, 0, 0, 0, 0, 4, 17,
            64, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
            0xbe, 0xef, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0xa, 0xb, 0xc, 0xd,
        ];

        let dest_addr = Ipv6Addr::new([
            0x0001, 0x0203, 0x0405, 0x0607, 0x0809, 0x0A0B, 0x0C0D, 0x0E0F,
        ]);

        let new_dest_addr = Ipv6Addr::new([
            0xdead, 0xbeef, 0xdead, 0xbeef, 0xdead, 0xbeef, 0xdead, 0xbeef,
        ]);

        let mut packet = Ipv6Packet::new(&mut data).unwrap();

        assert_eq!(packet.dest_addr(), dest_addr);
        packet.set_dest_addr(new_dest_addr.clone());
        assert_eq!(packet.dest_addr(), new_dest_addr);
    }

    #[test]
    fn set_payload() {
        let mut data: Vec<u8> = vec![
            0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0, 0x60, 0, 0, 0, 0, 4, 17,
            64, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
            0xbe, 0xef, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0xa, 0xb, 0xc, 0xd,
        ];

        let mut packet = Ipv6Packet::new(&mut data).unwrap();

        assert_eq!(packet.data[packet.payload_offset], 0xa);

        let new_payload: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        packet.set_payload(&new_payload);

        assert_eq!(packet.payload()[3], 4);
        assert_eq!(packet.payload().len(), 10);
    }
}
