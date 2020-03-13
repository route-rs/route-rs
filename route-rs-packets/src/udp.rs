use crate::*;
use std::borrow::Cow;
use std::convert::{TryFrom, TryInto};

#[derive(Clone, Debug)]
pub struct UdpSegment {
    pub data: PacketData,
    pub layer2_offset: Option<usize>,
    pub layer3_offset: Option<usize>,
    pub layer4_offset: usize,
    pub payload_offset: usize,
}

impl Packet for UdpSegment {}

impl<'packet> UdpSegment {
    pub fn from_buffer(
        data: PacketData,
        layer2_offset: Option<usize>, // Prep to switch to optional
        layer3_offset: Option<usize>, // Prep to switch to optional
        layer4_offset: usize,
    ) -> Result<UdpSegment, &'static str> {
        // Do we need to check that the appropriate frame and IP are present? I don't think that will be required
        // once the layer3 and layer2 are optional.
        if data.len() < layer4_offset + 8 {
            return Err("Segment to short to contain valid IP Header");
        }

        if let Some(layer3_offset) = layer3_offset {
            let protocol;
            let ip_version = (data[layer3_offset] & 0xF0) >> 4;
            match ip_version {
                4 => {
                    protocol = get_ipv4_payload_type(&data, layer3_offset)
                        .expect("Malformed IPv4 Header in UdpSegment");
                }
                6 => {
                    protocol = get_ipv6_payload_type(&data, layer3_offset)
                        .expect("Malformed IPv6 Header in UdpSegment");
                }
                _ => {
                    return Err("IP Header has invalid version number");
                }
            }
            // See the other note about how we are not Ipv6 compatible yet :(
            if protocol != IpProtocol::UDP {
                return Err("Protocol is incorrect, since it isn't UDP");
            }
        }

        let length = u16::from_be_bytes(
            data[layer4_offset + 4..=layer4_offset + 5]
                .try_into()
                .unwrap(),
        );

        if data.len() < layer4_offset + length as usize {
            return Err("Segment is not correct length as given by it's length field");
        }

        Ok(UdpSegment {
            data,
            layer2_offset,
            layer3_offset,
            layer4_offset,
            payload_offset: layer4_offset + 8,
        })
    }

    /// Make an empty UDPSegment, with no layer 3 header nor payload.
    pub fn empty() -> UdpSegment {
        let mut data = vec![];
        data.resize(8, 0);
        data[5] = 8; //Set length field.
        UdpSegment::from_buffer(data, None, None, 0).unwrap()
    }

    pub fn src_port(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.layer4_offset..=self.layer4_offset + 1]
                .try_into()
                .unwrap(),
        )
    }

    pub fn set_src_port(&mut self, port: u16) -> &mut Self {
        self.data[self.layer4_offset..=self.layer4_offset + 1].copy_from_slice(&port.to_be_bytes());
        self
    }

    pub fn dest_port(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.layer4_offset + 2..=self.layer4_offset + 3]
                .try_into()
                .unwrap(),
        )
    }

    pub fn set_dest_port(&mut self, port: u16) -> &mut Self {
        self.data[self.layer4_offset + 2..=self.layer4_offset + 3]
            .copy_from_slice(&port.to_be_bytes());
        self
    }

    pub fn length(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.layer4_offset + 4..=self.layer4_offset + 5]
                .try_into()
                .unwrap(),
        )
    }

    pub fn checksum(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.layer4_offset + 6..=self.layer4_offset + 7]
                .try_into()
                .unwrap(),
        )
    }

    /// Manually set the checksum of UDP packet, this should be improved later to
    /// be calculated automatically.
    pub fn set_checksum(&mut self, checksum: u16) -> &mut Self {
        self.data[self.layer4_offset + 6..=self.layer4_offset + 7]
            .copy_from_slice(&checksum.to_be_bytes());
        self
    }

    pub fn payload(&self) -> Cow<[u8]> {
        Cow::from(&self.data[self.layer4_offset + 8..])
    }

    /// Set payload of UDP packet, does not change checksum.
    /// Don't forget to update the length field of the IP packet that contains this.
    pub fn set_payload(&mut self, payload: &[u8]) -> &mut Self {
        let payload_len = payload.len();
        self.data.truncate(self.payload_offset);
        self.data.reserve_exact(payload_len);
        self.data.extend(payload);
        self
    }
}

/// UdpSegments are considered the same if they have the same data from the layer 4
/// header and onward. This function does not consider the data before the start of
/// the UDP header.
impl PartialEq for UdpSegment {
    fn eq(&self, other: &Self) -> bool {
        self.data[self.layer4_offset..] == other.data[other.layer4_offset..]
    }
}

impl Eq for UdpSegment {}

impl TryFrom<Ipv4Packet> for UdpSegment {
    type Error = &'static str;

    fn try_from(packet: Ipv4Packet) -> Result<Self, Self::Error> {
        UdpSegment::from_buffer(
            packet.data,
            packet.layer2_offset,
            Some(packet.layer3_offset),
            packet.payload_offset,
        )
    }
}

impl TryFrom<Ipv6Packet> for UdpSegment {
    type Error = &'static str;

    fn try_from(packet: Ipv6Packet) -> Result<Self, Self::Error> {
        UdpSegment::from_buffer(
            packet.data,
            packet.layer2_offset,
            Some(packet.layer3_offset),
            packet.payload_offset,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    #[test]
    fn udp_segment() {
        let mac_data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let ipv4_data: Vec<u8> = vec![
            0x45, 0, 0, 20, 0, 0, 0, 0, 64, 17, 0, 0, 192, 178, 128, 0, 10, 0, 0, 1,
        ];
        let udp_data: Vec<u8> = vec![
            0, 99, 0, 88, 0, 19, 0xDE, 0xAD, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
        ];

        let mut frame = EthernetFrame::from_buffer(mac_data, 0).unwrap();
        frame.set_payload(&ipv4_data);
        let mut packet = Ipv4Packet::try_from(frame).unwrap();
        packet.set_payload(&udp_data);
        let segment = UdpSegment::try_from(packet).unwrap();

        assert_eq!(segment.src_port(), 99);
        assert_eq!(segment.dest_port(), 88);
        assert_eq!(segment.length(), 19);
        assert_eq!(segment.checksum(), 0xDEAD);
        assert_eq!(segment.payload().len(), 11);
        assert_eq!(segment.payload()[0], 0);
    }

    #[test]
    fn empty() {
        let empty_segment = UdpSegment::empty();
        assert_eq!(empty_segment.layer2_offset, None);
        assert_eq!(empty_segment.layer3_offset, None);
        assert_eq!(empty_segment.layer4_offset, 0);
        assert_eq!(empty_segment.payload_offset, 8);
    }
}
