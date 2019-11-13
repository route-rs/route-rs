use crate::*;
use std::borrow::Cow;
use std::convert::{TryFrom, TryInto};
use std::net::Ipv4Addr;

#[derive(Clone, Debug)]
pub struct Ipv4Packet {
    pub data: PacketData,
    pub layer2_offset: Option<usize>,
    pub layer3_offset: usize,
    pub payload_offset: usize,
}

impl Ipv4Packet {
    fn new(
        data: PacketData,
        layer2_offset: Option<usize>,
        layer3_offset: usize,
    ) -> Result<Ipv4Packet, &'static str> {
        // Header of Ethernet Frame: 14 bytes
        // Header of IPv4 Frame: 20 bytes
        if data.len() < layer3_offset + 20 {
            return Err("Data is too short to be an IPv4 Packet");
        }

        // Check version number
        let version: u8 = (data[layer3_offset] & 0xF0) >> 4;
        if version != 4 {
            return Err("Packet has incorrect version, is not Ipv4Packet");
        }

        // TotalLen is the 3rd and 4th byte of the IP Header
        let total_len = u16::from_be_bytes(
            data[layer3_offset + 2..=layer3_offset + 3]
                .try_into()
                .unwrap(),
        ) as usize;
        if data.len() != total_len + layer3_offset {
            return Err("Packet has invalid total length field");
        }

        // This is the header length in 32bit words
        let ihl = (data[layer3_offset] & 0x0F) as usize;
        let payload_offset = layer3_offset + (ihl * 4);

        Ok(Ipv4Packet {
            data,
            layer2_offset,
            layer3_offset,
            payload_offset,
        })
    }

    pub fn src_addr(&self) -> Ipv4Addr {
        let data: [u8; 4] = self.data[self.layer3_offset + 12..self.layer3_offset + 16]
            .try_into()
            .unwrap();
        Ipv4Addr::from(data)
    }

    pub fn set_src_addr(&mut self, addr: Ipv4Addr) {
        self.data[self.layer3_offset + 12..self.layer3_offset + 16].copy_from_slice(&addr.octets());
    }

    pub fn dest_addr(&self) -> Ipv4Addr {
        let data: [u8; 4] = self.data[self.layer3_offset + 16..self.layer3_offset + 20]
            .try_into()
            .unwrap();
        Ipv4Addr::from(data)
    }

    pub fn set_dest_addr(&mut self, addr: Ipv4Addr) {
        self.data[self.layer3_offset + 16..self.layer3_offset + 20].copy_from_slice(&addr.octets());
    }

    pub fn ihl(&self) -> u8 {
        self.data[self.layer3_offset] & 0x0F
    }

    pub fn set_ihl(&mut self, header_length: usize) {
        self.data[self.layer3_offset] &= 0xF0; // Clear least sig 4 bits
        self.data[self.layer3_offset] |= 0x0F & ((header_length / 4) as u8);
        self.payload_offset = header_length;
    }

    pub fn payload(&self) -> Cow<[u8]> {
        Cow::from(&self.data[self.payload_offset..])
    }

    pub fn set_payload(&mut self, payload: &[u8]) {
        let payload_len = payload.len();

        self.data.truncate(self.payload_offset);

        let total_len = (payload_len as u16 + u16::from(self.ihl() * 4)).to_be_bytes();
        self.data[self.layer3_offset + 2..=self.layer3_offset + 3].copy_from_slice(&total_len);

        self.data.reserve_exact(payload_len);
        self.data.extend(payload);
    }

    pub fn options(&self) -> Option<Cow<[u8]>> {
        if self.ihl() <= 5 {
            return None;
        }
        Some(Cow::from(
            &self.data[self.layer3_offset + 20..self.payload_offset],
        ))
    }

    /// Sets the options of the Ipv4 packet to the provided array, also
    /// sets the IHL field of the packet, and the internal payload_offset
    /// field.
    /// Note: The user should provide options that are padded to a 32bit length.
    pub fn set_options(&mut self, options: &[u8]) {
        let payload = self.data.split_off(self.payload_offset);
        self.data.truncate(self.layer3_offset + 20);
        self.data.reserve_exact(payload.len() + options.len());
        self.data.extend(options);
        self.data.extend(payload);
        self.set_ihl(options.len() + 20);
    }

    pub fn protocol(&self) -> IpProtocol {
        IpProtocol::from(self.data[self.layer3_offset + 9])
    }

    pub fn total_len(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.layer3_offset + 2..=self.layer3_offset + 3]
                .try_into()
                .unwrap(),
        )
    }

    pub fn ttl(&self) -> u8 {
        self.data[self.layer3_offset + 8]
    }

    pub fn set_ttl(&mut self, ttl: u8) {
        self.data[self.layer3_offset + 8] = ttl;
    }

    pub fn checksum(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.layer3_offset + 10..=self.layer3_offset + 11]
                .try_into()
                .unwrap(),
        )
    }

    pub fn dscp(&self) -> u8 {
        self.data[self.layer3_offset + 1] >> 2
    }

    pub fn ecn(&self) -> u8 {
        self.data[self.layer3_offset + 1] & 0x03
    }

    pub fn indentification(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.layer3_offset + 4..=self.layer3_offset + 5]
                .try_into()
                .unwrap(),
        )
    }

    pub fn fragment_offet(&self) -> u16 {
        u16::from_be_bytes([
            self.data[self.layer3_offset + 6] & 0x1F,
            self.data[self.layer3_offset + 7],
        ])
    }

    /// Returns tuple of (Don't Fragment, More Fragments)
    pub fn flags(&self) -> (bool, bool) {
        let df = (self.data[self.layer3_offset + 6] & 0x40) != 0;
        let mf = (self.data[self.layer3_offset + 6] & 0x20) != 0;
        (df, mf)
    }

    /// Verifies the IP header checksum, returns the value and also sets
    /// the internal bookeeping field. As such we need a mutable reference.
    pub fn validate_checksum(&mut self) -> bool {
        let full_sum = &self.data[self.layer3_offset..self.payload_offset]
            .chunks_exact(2)
            .fold(0, |acc: u32, x| {
                acc + u32::from(u16::from_be_bytes([x[0], x[1]]))
            });
        let (carry, mut sum) = (((full_sum & 0xFFFF_0000) >> 16), (full_sum & 0x0000_FFFF));
        sum += carry;
        0 == (!sum & 0xFFFF)
    }

    /// Calculates what the checksum should be set to given the current header
    pub fn caclulate_checksum(&self) -> u16 {
        let full_sum = &self.data[self.layer3_offset..self.payload_offset]
            .chunks_exact(2)
            .enumerate()
            .filter(|x| x.0 != 5)
            .fold(0, |acc: u32, x| {
                acc + u32::from(u16::from_be_bytes([x.1[0], x.1[1]]))
            });
        let (carry, mut sum) = (((full_sum & 0xFFFF_0000) >> 16), (full_sum & 0x0000_FFFF));
        sum += carry;
        if sum & 0xFFFF_0000 != 0 {
            sum += 1;
        }
        sum = !sum & 0xFFFF;
        sum as u16
    }

    /// Sets checksum field to valid value
    pub fn set_checksum(&mut self) {
        let new_checksum = self.caclulate_checksum();
        self.data[self.layer3_offset + 10] = ((new_checksum & 0xFF00) >> 8) as u8;
        self.data[self.layer3_offset + 11] = (new_checksum & 0x00FF) as u8;
    }
}

/// Ipv4Packets are considered the same if they have the same data from the layer 4
/// header and onward. This function does not consider the data before the start of
/// the IPv4 header.
impl PartialEq for Ipv4Packet {
    fn eq(&self, other: &Self) -> bool {
        self.data[self.layer3_offset..] == other.data[other.layer3_offset..]
    }
}

impl Eq for Ipv4Packet {}

/// Returns Ipv4 payload type, reads the header information to get the type
/// of IpProtocol payload is included. Upon error, returns IpProtocol::Reserved.
pub fn get_ipv4_payload_type(
    data: &[u8],
    layer3_offset: usize,
) -> Result<IpProtocol, &'static str> {
    if data.len() <= layer3_offset + 9 || (data[layer3_offset] & 0xF0) != 0x40 {
        // Either data isn't big enough, or the version field does not indicate this is
        // an Ipv4 packet.
        return Err("Is not an Ipv4 packet");
    }
    Ok(IpProtocol::from(data[layer3_offset + 9]))
}

impl TryFrom<EthernetFrame> for Ipv4Packet {
    type Error = &'static str;

    fn try_from(frame: EthernetFrame) -> Result<Self, Self::Error> {
        Ipv4Packet::new(frame.data, Some(frame.layer2_offset), frame.payload_offset)
    }
}

impl TryFrom<TcpSegment> for Ipv4Packet {
    type Error = &'static str;

    fn try_from(segment: TcpSegment) -> Result<Self, Self::Error> {
        if let Some(layer3_offset) = segment.layer3_offset {
            Ipv4Packet::new(segment.data, segment.layer2_offset, layer3_offset)
        } else {
            Err("TCP Segment does not contain an IP Packet")
        }
    }
}

impl TryFrom<UdpSegment> for Ipv4Packet {
    type Error = &'static str;

    fn try_from(segment: UdpSegment) -> Result<Self, Self::Error> {
        if let Some(layer3_offset) = segment.layer3_offset {
            Ipv4Packet::new(segment.data, segment.layer2_offset, layer3_offset)
        } else {
            Err("UDP Segment does not contain an IP Packet")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    #[test]
    fn ipv4_packet() {
        let mac_data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let ip_data: Vec<u8> = vec![
            0x45, 0, 0, 20, 0, 0, 0, 0, 64, 17, 0, 0, 192, 178, 128, 0, 10, 0, 0, 1,
        ];

        let mut frame = EthernetFrame::new(mac_data, 0).unwrap();
        frame.set_payload(&ip_data);

        let packet = Ipv4Packet::try_from(frame).unwrap();

        assert_eq!(packet.src_addr(), Ipv4Addr::new(192, 178, 128, 0));
        assert_eq!(packet.dest_addr(), Ipv4Addr::new(10, 0, 0, 1));
        assert_eq!(packet.ihl(), 5);
        assert_eq!(packet.payload().len(), 0);
        assert_eq!(packet.options(), None);
        assert_eq!(packet.protocol(), IpProtocol::UDP);
        assert_eq!(packet.total_len(), 20);
        assert_eq!(packet.ttl(), 64);
        assert_eq!(packet.checksum(), 0);
        assert_eq!(packet.dscp(), 0);
        assert_eq!(packet.ecn(), 0);
        assert_eq!(packet.indentification(), 0);
        assert_eq!(packet.fragment_offet(), 0);
        assert_eq!(packet.flags(), (false, false));
    }

    #[test]
    fn validate_checksum() {
        let mac_data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let invalid_checksum_data: Vec<u8> = vec![
            0x45, 0x00, 0x00, 0x14, 0x00, 0x00, 0x40, 0x00, 0x40, 0x11, 0xb8, 0x61, 0xc0, 0xa8,
            0x00, 0x01, 0xc0, 0xa8, 0x00, 0xc7,
        ];
        let mut frame = EthernetFrame::new(mac_data, 0).unwrap();
        frame.set_payload(&invalid_checksum_data);
        let mut packet = Ipv4Packet::try_from(frame).unwrap();
        assert!(!packet.validate_checksum());

        let valid_checksum_data: Vec<u8> = vec![
            0x45, 0x00, 0x00, 0x14, 0x00, 0x00, 0x40, 0x00, 0x40, 0x11, 0xb8, 0xc0, 0xc0, 0xa8,
            0x00, 0x01, 0xc0, 0xa8, 0x00, 0xc7,
        ];
        let mut frame = EthernetFrame::try_from(packet).unwrap();
        frame.set_payload(&valid_checksum_data);
        let mut packet = Ipv4Packet::try_from(frame).unwrap();
        assert!(packet.validate_checksum());
    }

    #[test]
    fn set_checksum() {
        let mac_data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let ip_data: Vec<u8> = vec![
            0x45, 0x00, 0x00, 0x14, 0x00, 0x00, 0x40, 0x00, 0x40, 0x11, 0xb8, 0x61, 0xc0, 0xa8,
            0x00, 0x01, 0xc0, 0xa8, 0x00, 0xc7,
        ];
        let mut frame = EthernetFrame::new(mac_data, 0).unwrap();
        frame.set_payload(&ip_data);
        let mut packet = Ipv4Packet::try_from(frame).unwrap();
        assert!(!packet.validate_checksum());
        packet.set_checksum();
        assert!(packet.validate_checksum());
    }

    #[test]
    fn set_ihl() {
        let data: Vec<u8> = vec![
            0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0, 0x45, 0, 0, 20, 0, 0, 0, 0,
            64, 17, 0, 0, 192, 178, 128, 0, 10, 0, 0, 1,
        ];

        let mut packet = Ipv4Packet::new(data, Some(0), 14).unwrap();
        assert_eq!(packet.ihl(), 5);
        packet.set_ihl(24);
        assert_eq!(packet.ihl(), 6);
    }
}
