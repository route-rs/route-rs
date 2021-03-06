use crate::*;
use std::borrow::Cow;
use std::convert::TryFrom;
use std::convert::TryInto;
use std::net::Ipv6Addr;

#[derive(Clone, Debug)]
pub struct Ipv6Packet {
    pub data: PacketData,
    pub layer2_offset: Option<usize>,
    pub layer3_offset: usize,
    pub payload_offset: usize,
}

impl Ipv6Packet {
    pub fn from_buffer(
        data: PacketData,
        layer2_offset: Option<usize>,
        layer3_offset: usize,
    ) -> Result<Ipv6Packet, &'static str> {
        // Header of Ethernet Frame: 14bytes
        // Haeder of IPv6 Frame: 40bytes minimum
        if data.len() < layer3_offset + 40 {
            return Err("Packet is too short to be an Ipv6Packet");
        }

        // Check version number
        let version = (data[layer3_offset] & 0xF0) >> 4;
        if version != 6 {
            return Err("Packet has incorrect version, is not Ipv6Packet");
        }

        // Note, there is a special unhandled edge case here, if the payload len is 0, there may
        // be a hop-by-hop extension header, that means we may have a jumbo packet. Not going to
        // handle this edge case for now, but it is needed before shipping.
        // This may also not be true if there are extension headers, so we just check that we will not
        // overrun our array trying to access the entire payload.
        let payload_len = u16::from_be_bytes(
            data[layer3_offset + 4..=layer3_offset + 5]
                .try_into()
                .unwrap(),
        ) as usize;
        let packet_len = data.len();
        if payload_len + layer3_offset + 40 > packet_len {
            return Err("Packet has invalid payload len field");
        }

        Ok(Ipv6Packet {
            data,
            layer2_offset,
            layer3_offset,
            payload_offset: packet_len - payload_len,
        })
    }

    pub fn empty() -> Ipv6Packet {
        let mut data = vec![0x60];
        data.resize(40, 0);
        Ipv6Packet::from_buffer(data, None, 0).unwrap()
    }

    pub fn traffic_class(&self) -> u8 {
        ((self.data[self.layer3_offset] & 0x0F) << 4)
            + (self.data[self.layer3_offset + 1] & 0xF0 >> 4)
    }

    pub fn set_traffic_class(&mut self, traffic_class: u8) {
        self.data[self.layer3_offset] &= 0xF0;
        self.data[self.layer3_offset + 1] &= 0x0F;
        self.data[self.layer3_offset] |= traffic_class >> 4;
        self.data[self.layer3_offset + 1] |= traffic_class << 4;
    }

    /// Returns flow label as least significant 20 bits.
    pub fn flow_label(&self) -> u32 {
        u32::from_be_bytes([
            0,
            self.data[self.layer3_offset + 1] & 0x0F,
            self.data[self.layer3_offset + 2],
            self.data[self.layer3_offset + 3],
        ])
    }

    /// Sets flow label based on least significant 20 bits of flow_label parameter
    pub fn set_flow_label(&mut self, flow_label: u32) {
        self.data[self.layer3_offset + 1] &= 0xF0;
        self.data[self.layer3_offset + 1] |= ((flow_label & 0x000F_FFFF) >> 16) as u8;
        self.data[self.layer3_offset + 2..=self.layer3_offset + 3]
            .copy_from_slice(&(flow_label as u16).to_be_bytes())
    }

    pub fn payload_length(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.layer3_offset + 4..=self.layer3_offset + 5]
                .try_into()
                .unwrap(),
        )
    }

    pub fn next_header(&self) -> IpProtocol {
        IpProtocol::from(self.data[self.layer3_offset + 6])
    }

    pub fn set_next_header(&mut self, header: u8) {
        self.data[self.layer3_offset + 6] = header;
    }

    pub fn hop_limit(&self) -> u8 {
        self.data[self.layer3_offset + 7]
    }

    pub fn set_hop_limit(&mut self, hop_limit: u8) {
        self.data[self.layer3_offset + 7] = hop_limit;
    }

    // Is there a bug here if there is no payload? wonder if
    // self.payload_offset would overrun the array, and cause a panic
    pub fn payload(&self) -> Cow<[u8]> {
        Cow::from(&self.data[self.payload_offset..])
    }

    pub fn set_payload(&mut self, payload: &[u8]) {
        let new_payload_len = payload.len();
        self.data.truncate(self.payload_offset);

        let payload_len = (new_payload_len as u16).to_be_bytes();
        self.data[self.layer3_offset + 4..self.layer3_offset + 6].copy_from_slice(&payload_len);

        self.data.reserve_exact(new_payload_len);
        self.data.extend(payload);
        self.payload_offset = self.data.len() - payload.len();
    }

    pub fn src_addr(&self) -> Ipv6Addr {
        let data: [u8; 16] = self.data[self.layer3_offset + 8..self.layer3_offset + 24]
            .try_into()
            .unwrap();
        Ipv6Addr::from(data)
    }

    pub fn dest_addr(&self) -> Ipv6Addr {
        let data: [u8; 16] = self.data[self.layer3_offset + 24..self.layer3_offset + 40]
            .try_into()
            .unwrap();
        Ipv6Addr::from(data)
    }

    pub fn set_src_addr(&mut self, addr: Ipv6Addr) {
        self.data[self.layer3_offset + 8..self.layer3_offset + 24].copy_from_slice(&addr.octets());
    }

    pub fn set_dest_addr(&mut self, addr: Ipv6Addr) {
        self.data[self.layer3_offset + 24..self.layer3_offset + 40].copy_from_slice(&addr.octets());
    }

    // TODO: Test the get and set for extension headers.
    pub fn extension_headers(&self) -> Vec<Cow<[u8]>> {
        let mut headers = Vec::<Cow<[u8]>>::new();
        let mut next_header = self.next_header();
        let mut header_ext_len;
        let mut offset = self.layer3_offset + 40; // First byte of first header
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
                | IpProtocol::Use_for_experimentation_and_testing => {
                    header_ext_len = self.data[offset + 1];
                    if header_ext_len == 0 {
                        // Fragments have the minimum of 8, but it set to zero for some dumb reason
                        // https://en.wikipedia.org/wiki/IPv6_packet#Fragment
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
        self.data.truncate(self.layer3_offset + 40);
        for header in headers.iter() {
            self.data.extend(*header);
        }
        self.data.extend(payload);
        if !headers.is_empty() {
            self.set_next_header(first_header as u8);
        }
    }

    /// Takes a UdpSegment, and returns an Ipv6Packet with the
    /// segment as payload. Does not set checksums
    pub fn encap_udp(udp: UdpSegment) -> Ipv6Packet {
        let mut packet = Ipv6Packet::empty();
        packet.set_payload(&udp.data[udp.layer4_offset..]);
        packet.set_next_header(0x11); //UDP Header
        packet
    }

    /// Takes a TcpSegment, and returns an Ipv6Packet with the
    /// segment as payload. Does not set checksums
    pub fn encap_tcp(tcp: TcpSegment) -> Ipv6Packet {
        let mut packet = Ipv6Packet::empty();
        packet.set_payload(&tcp.data[tcp.layer4_offset..]);
        packet.set_next_header(0x06); //TCP Header
        packet
    }
}

/// Ipv6Packets are considered the same if they have the same data from the layer 4
/// header and onward. This function does not consider the data before the start of
/// the IPv6 header.
impl PartialEq for Ipv6Packet {
    fn eq(&self, other: &Self) -> bool {
        self.data[self.layer3_offset..] == other.data[other.layer3_offset..]
    }
}

impl Eq for Ipv6Packet {}

/// Returns Ipv6 payload type, reads the header information to get the type
/// of IpProtocol payload is included. Upon error, returns IpProtocol::Reserved.
pub fn get_ipv6_payload_type(
    data: &[u8],
    layer3_offset: usize,
) -> Result<IpProtocol, &'static str> {
    if data.len() < layer3_offset + 40 || data[layer3_offset] & 0xF0 != 0x60 {
        // In the case of error, we return the reserved as an error.
        return Err("Is not an Ipv6 Packet");
    }

    let mut header = IpProtocol::from(data[layer3_offset + 6]);
    let mut header_ext_len;
    let mut offset = layer3_offset + 40; // First byte of first header
    loop {
        match header {
            IpProtocol::HOPOPT
            | IpProtocol::IPv6_Opts
            | IpProtocol::IPv6_route
            | IpProtocol::IPv6_frag
            | IpProtocol::AH
            | IpProtocol::ESP
            | IpProtocol::Mobility_Header
            | IpProtocol::HIP
            | IpProtocol::Shim6
            | IpProtocol::Use_for_experimentation_and_testing => {
                if data.len() <= offset + 1 {
                    // Check for length overrun
                    return Ok(IpProtocol::Reserved);
                }
                header_ext_len = data[offset + 1];
                if header_ext_len == 0 {
                    // Fragments have the minimum of 8, but it set to zero for some dumb reason
                    // https://en.wikipedia.org/wiki/IPv6_packet#Fragment
                    header_ext_len = 8;
                }
                header = IpProtocol::from(data[offset]);
                offset += header_ext_len as usize;
            }
            _ => {
                return Ok(header);
            }
        }
    }
}

impl TryFrom<EthernetFrame> for Ipv6Packet {
    type Error = &'static str;

    fn try_from(frame: EthernetFrame) -> Result<Self, Self::Error> {
        Ipv6Packet::from_buffer(frame.data, Some(frame.layer2_offset), frame.payload_offset)
    }
}

impl TryFrom<TcpSegment> for Ipv6Packet {
    type Error = &'static str;

    fn try_from(segment: TcpSegment) -> Result<Self, Self::Error> {
        if let Some(layer3_offset) = segment.layer3_offset {
            Ipv6Packet::from_buffer(segment.data, segment.layer2_offset, layer3_offset)
        } else {
            Err("TCP segment does not contain IP Header")
        }
    }
}

impl TryFrom<UdpSegment> for Ipv6Packet {
    type Error = &'static str;

    fn try_from(segment: UdpSegment) -> Result<Self, Self::Error> {
        if let Some(layer3_offset) = segment.layer3_offset {
            Ipv6Packet::from_buffer(segment.data, segment.layer2_offset, layer3_offset)
        } else {
            Err("UDP segment does not contain IP Header")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    #[test]
    fn ipv6_packet() {
        let mac_data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let ip_data: Vec<u8> = vec![
            0x60, 0, 0, 0, 0, 4, 17, 64, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde,
            0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13,
            14, 15, 0xa, 0xb, 0xc, 0xd,
        ];

        let src_addr = Ipv6Addr::new(
            0xdead, 0xbeef, 0xdead, 0xbeef, 0xdead, 0xbeef, 0xdead, 0xbeef,
        );
        let dest_addr = Ipv6Addr::new(
            0x0001, 0x0203, 0x0405, 0x0607, 0x0809, 0x0A0B, 0x0C0D, 0x0E0F,
        );

        let mut frame = EthernetFrame::from_buffer(mac_data, 0).unwrap();
        frame.set_payload(&ip_data);

        let packet = Ipv6Packet::try_from(frame).unwrap();
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
        let data: Vec<u8> = vec![
            0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0, 0x60, 0, 0, 0, 0, 4, 17,
            64, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
            0xbe, 0xef, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0xa, 0xb, 0xc, 0xd,
        ];

        let src_addr = Ipv6Addr::new(
            0xdead, 0xbeef, 0xdead, 0xbeef, 0xdead, 0xbeef, 0xdead, 0xbeef,
        );

        let new_src_addr = Ipv6Addr::new(
            0x0001, 0x0203, 0x0405, 0x0607, 0x0809, 0x0A0B, 0x0C0D, 0x0E0F,
        );

        let mut packet = Ipv6Packet::from_buffer(data, Some(0), 14).unwrap();

        assert_eq!(packet.src_addr(), src_addr);
        packet.set_src_addr(new_src_addr);
        assert_eq!(packet.src_addr(), new_src_addr);
    }

    #[test]
    fn set_dest_addr() {
        let data: Vec<u8> = vec![
            0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0, 0x60, 0, 0, 0, 0, 4, 17,
            64, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
            0xbe, 0xef, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0xa, 0xb, 0xc, 0xd,
        ];

        let dest_addr = Ipv6Addr::new(
            0x0001, 0x0203, 0x0405, 0x0607, 0x0809, 0x0A0B, 0x0C0D, 0x0E0F,
        );

        let new_dest_addr = Ipv6Addr::new(
            0xdead, 0xbeef, 0xdead, 0xbeef, 0xdead, 0xbeef, 0xdead, 0xbeef,
        );

        let mut packet = Ipv6Packet::from_buffer(data, Some(0), 14).unwrap();

        assert_eq!(packet.dest_addr(), dest_addr);
        packet.set_dest_addr(new_dest_addr);
        assert_eq!(packet.dest_addr(), new_dest_addr);
    }

    #[test]
    fn set_payload() {
        let data: Vec<u8> = vec![
            0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0, 0x60, 0, 0, 0, 0, 4, 17,
            64, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
            0xbe, 0xef, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0xa, 0xb, 0xc, 0xd,
        ];

        let mut packet = Ipv6Packet::from_buffer(data, Some(0), 14).unwrap();

        assert_eq!(packet.data[packet.payload_offset], 0xa);

        let new_payload: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        packet.set_payload(&new_payload);

        assert_eq!(packet.payload()[3], 4);
        assert_eq!(packet.payload().len(), 10);
    }

    #[test]
    fn empty() {
        let empty_packet = Ipv6Packet::empty();
        assert_eq!(empty_packet.layer2_offset, None);
        assert_eq!(empty_packet.layer3_offset, 0);
        assert_eq!(empty_packet.payload_offset, 40);
    }

    #[test]
    fn encap_udp() {
        let udp = UdpSegment::empty();
        let packet = Ipv6Packet::encap_udp(udp);
        assert_eq!(packet.layer3_offset, 0);
        assert_eq!(packet.layer2_offset, None);
        assert_eq!(packet.next_header(), IpProtocol::UDP);
        let new_segment = UdpSegment::try_from(packet).unwrap();
        assert_eq!(new_segment.layer2_offset, None);
        assert_eq!(new_segment.layer3_offset, Some(0));
        assert_eq!(new_segment.layer4_offset, 40);
    }

    #[test]
    fn encap_tcp() {
        let tcp = TcpSegment::empty();
        let packet = Ipv6Packet::encap_tcp(tcp);
        assert_eq!(packet.layer3_offset, 0);
        assert_eq!(packet.layer2_offset, None);
        assert_eq!(packet.next_header(), IpProtocol::TCP);
        let new_segment = TcpSegment::try_from(packet).unwrap();
        assert_eq!(new_segment.layer2_offset, None);
        assert_eq!(new_segment.layer3_offset, Some(0));
        assert_eq!(new_segment.layer4_offset, 40);
    }
}
