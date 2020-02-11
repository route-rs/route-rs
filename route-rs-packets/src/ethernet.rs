use crate::*;
use std::borrow::Cow;
use std::convert::{TryFrom, TryInto};

#[derive(Clone, Debug)]
pub struct EthernetFrame {
    pub data: PacketData,
    pub layer2_offset: usize,
    pub payload_offset: usize,
}

impl Packet for EthernetFrame {}

impl EthernetFrame {
    pub fn from_buffer(
        frame: PacketData,
        layer2_offset: usize,
    ) -> Result<EthernetFrame, &'static str> {
        // Ethernet II frames must be at least the header, which is 14bytes
        // 0                    6                    12                      14
        // |---6 byte Dest_MAC--|---6 byte Src_MAC---|--2 Byte EtherType---|
        // We could support other formats for the frames, but IP sits atop Ethernet II

        if frame.len() < 14 {
            return Err("Frame is less than the minimum of 14 bytes");
        }

        Ok(EthernetFrame {
            data: frame,
            layer2_offset,
            payload_offset: 14 + layer2_offset, // To support 802.1Q VLAN Tagging, this number may be different.
        })
    }

    /// Returns an empty EthernetFrame where all values all populated to zero. This function allocates a
    /// new array to hold the header.
    pub fn empty() -> EthernetFrame {
        let mut data = vec![];
        data.resize(14, 0);
        EthernetFrame::from_buffer(data, 0).unwrap()
    }

    pub fn dest_mac(&self) -> MacAddr {
        let bytes = <[u8; 6]>::try_from(&self.data[0..6]).unwrap();
        MacAddr::new(bytes)
    }

    pub fn src_mac(&self) -> MacAddr {
        let bytes = <[u8; 6]>::try_from(&self.data[6..12]).unwrap();
        MacAddr::new(bytes)
    }

    pub fn set_dest_mac(&mut self, mac: MacAddr) {
        self.data[..6].copy_from_slice(&mac.bytes[..6]);
    }

    pub fn set_src_mac(&mut self, mac: MacAddr) {
        self.data[6..12].copy_from_slice(&mac.bytes[..6]);
    }

    pub fn ether_type(&self) -> u16 {
        u16::from_be_bytes(self.data[12..=13].try_into().unwrap())
    }

    pub fn set_ether_type(&mut self, ether_type: u16) {
        self.data[12..=13].copy_from_slice(&ether_type.to_be_bytes());
    }

    // This gives you a cow of a slice of the payload.
    pub fn payload(&self) -> Cow<[u8]> {
        Cow::from(&self.data[self.payload_offset..])
    }

    pub fn set_payload(&mut self, payload: &[u8]) {
        let payload_len = payload.len() as u16;
        self.data.truncate(self.payload_offset);
        self.data.reserve_exact(payload_len as usize);
        self.data.extend(payload);
    }

    pub fn encap_ipv4(ipv4: Ipv4Packet) -> EthernetFrame {
        let mut frame = EthernetFrame::empty();
        frame.set_payload(&ipv4.data[ipv4.layer3_offset..]);
        frame.set_ether_type(0x0800);
        frame
    }

    pub fn encap_ipv6(ipv6: Ipv6Packet) -> EthernetFrame {
        let mut frame = EthernetFrame::empty();
        frame.set_payload(&ipv6.data[ipv6.layer3_offset..]);
        frame.set_ether_type(0x86DD);
        frame
    }
}

/// EthernetFrames are considered the same if they have the same data from the layer 2
/// header and onward. This function does not consider the data before the start of the
/// Ethernet header
impl PartialEq for EthernetFrame {
    fn eq(&self, other: &Self) -> bool {
        self.data[self.layer2_offset..] == other.data[other.layer2_offset..]
    }
}

impl Eq for EthernetFrame {}

impl TryFrom<TcpSegment> for EthernetFrame {
    type Error = &'static str;

    fn try_from(segment: TcpSegment) -> Result<Self, Self::Error> {
        if let Some(layer2_offset) = segment.layer2_offset {
            EthernetFrame::from_buffer(segment.data, layer2_offset)
        } else {
            Err("TCP Segment does not contain an Ethernet Frame")
        }
    }
}

impl TryFrom<UdpSegment> for EthernetFrame {
    type Error = &'static str;

    fn try_from(segment: UdpSegment) -> Result<Self, Self::Error> {
        if let Some(layer2_offset) = segment.layer2_offset {
            EthernetFrame::from_buffer(segment.data, layer2_offset)
        } else {
            Err("UDP Segment does not contain an Ethernet Frame")
        }
    }
}

impl TryFrom<Ipv4Packet> for EthernetFrame {
    type Error = &'static str;

    fn try_from(packet: Ipv4Packet) -> Result<Self, Self::Error> {
        if let Some(layer2_offset) = packet.layer2_offset {
            EthernetFrame::from_buffer(packet.data, layer2_offset)
        } else {
            Err("IPv4 Packet does not contain an Ethernet Frame")
        }
    }
}

impl TryFrom<Ipv6Packet> for EthernetFrame {
    type Error = &'static str;

    fn try_from(packet: Ipv6Packet) -> Result<Self, Self::Error> {
        if let Some(layer2_offset) = packet.layer2_offset {
            EthernetFrame::from_buffer(packet.data, layer2_offset)
        } else {
            Err("Ipv6 Packet does not contain an Ethernet Frame")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    #[test]
    fn ethernet_frame() {
        let data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let frame = EthernetFrame::from_buffer(data, 0).unwrap();
        assert_eq!(
            frame.dest_mac(),
            MacAddr::new([0xde, 0xad, 0xbe, 0xef, 0xff, 0xff])
        );
        assert_eq!(frame.src_mac(), MacAddr::new([1, 2, 3, 4, 5, 6]));
        assert_eq!(frame.ether_type(), 0);
        assert_eq!(frame.payload().len(), 0);
    }

    #[test]
    fn set_payload() {
        let data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let mut frame = EthernetFrame::from_buffer(data, 0).unwrap();
        assert_eq!(frame.ether_type(), 0);
        assert_eq!(frame.payload().len(), 0);

        let new_payload: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
        frame.set_payload(&new_payload);
        assert_eq!(frame.payload(), new_payload);
        assert_eq!(frame.payload()[2], 3);
    }

    #[test]
    #[should_panic(expected = "Frame is less than the minimum of 14 bytes")]
    fn invalid_data_length() {
        let data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6];
        let _frame = EthernetFrame::from_buffer(data, 0).unwrap();
    }

    #[test]
    fn set_dest_mac() {
        let data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let mut frame = EthernetFrame::from_buffer(data, 0).unwrap();
        let new_dest = MacAddr::new([0x98, 0x88, 0x18, 0x12, 0xb4, 0xdf]);
        frame.set_dest_mac(new_dest);
        assert_eq!(frame.dest_mac(), new_dest);
    }

    #[test]
    fn set_src_mac() {
        let data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let mut frame = EthernetFrame::from_buffer(data, 0).unwrap();
        let new_src = MacAddr::new([0x98, 0x88, 0x18, 0x12, 0xb4, 0xdf]);
        frame.set_src_mac(new_src);
        assert_eq!(frame.src_mac(), new_src);
    }

    #[test]
    fn ether_type() {
        let data: Vec<u8> = vec![
            0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0xff, 0xff,
        ];
        let frame = EthernetFrame::from_buffer(data, 0).unwrap();
        assert_eq!(frame.ether_type(), 0xffff);
    }

    #[test]
    fn empty() {
        let empty_frame = EthernetFrame::empty();
        assert_eq!(empty_frame.layer2_offset, 0);
        assert_eq!(empty_frame.payload_offset, 14);
    }

    #[test]
    fn encap_ipv4() {
        let frame = EthernetFrame::encap_ipv4(Ipv4Packet::empty());
        assert_eq!(frame.layer2_offset, 0);
        assert_eq!(frame.payload_offset, 14);
        assert_eq!(frame.ether_type(), 0x0800);
    }

    #[test]
    fn encap_ipv6() {
        let frame = EthernetFrame::encap_ipv6(Ipv6Packet::empty());
        assert_eq!(frame.layer2_offset, 0);
        assert_eq!(frame.payload_offset, 14);
        assert_eq!(frame.ether_type(), 0x86DD);
    }

    #[test]
    fn full_encap_decap() {
        let frame = EthernetFrame::encap_ipv4(Ipv4Packet::encap_udp(UdpSegment::empty()));
        let _segment = UdpSegment::try_from(Ipv4Packet::try_from(frame).unwrap()).unwrap();

        let tcp_segment = TcpSegment::try_from(
            Ipv6Packet::try_from(EthernetFrame::encap_ipv6(Ipv6Packet::encap_tcp(
                TcpSegment::empty(),
            )))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(tcp_segment.layer2_offset, Some(0));
        assert_eq!(tcp_segment.layer3_offset, Some(14));
        assert_eq!(tcp_segment.layer4_offset, 54);
        assert_eq!(tcp_segment.payload_offset, 74);
    }
}
