use crate::packet::*;
use std::borrow::Cow;
use std::convert::TryFrom;

#[allow(dead_code)]
pub struct UdpSegment<'packet> {
    pub data: PacketData<'packet>,
    pub packet_offset: usize,
    pub segment_offset: usize,
    validated_checksum: bool,
}

impl<'packet> UdpSegment<'packet> {
    fn new(
        segment: PacketData,
        packet_offset: usize,
        segment_offset: usize,
    ) -> Result<UdpSegment, &'static str> {
        // First let's check that the Frame and IP Header is present
        if segment.len() < packet_offset + 20 {
            return Err("Segment to short to contain valid IP Header");
        }

        let protocol;
        let ip_version = (segment[packet_offset] & 0xF0) >> 4;
        match ip_version {
            4 => {
                protocol = get_ipv4_payload_type(segment, packet_offset);
            }
            6 => {
                protocol = get_ipv6_payload_type(segment, packet_offset);
            }
            _ => {
                return Err("IP Header has invalid version number");
            }
        }

        // See the other note about how we are not Ipv6 compatible yet :(
        if protocol != IpProtocol::UDP {
            return Err("Protocol is incorrect, since it isn't UDP");
        }

        let length = u16::from_be_bytes([segment[segment_offset + 4], segment[segment_offset + 5]]);

        if segment.len() < segment_offset + length as usize {
            return Err("Segment is not correct length as given by it's length field");
        }

        Ok(UdpSegment {
            data: segment,
            packet_offset,
            segment_offset,
            validated_checksum: false,
        })
    }

    pub fn src_port(&self) -> u16 {
        u16::from_be_bytes([
            self.data[self.segment_offset],
            self.data[self.segment_offset + 1],
        ])
    }

    pub fn set_src_port(&mut self, port: u16) {
        self.data[self.segment_offset..=self.segment_offset + 1]
            .copy_from_slice(&port.to_be_bytes());
    }

    pub fn dest_port(&self) -> u16 {
        u16::from_be_bytes([
            self.data[self.segment_offset + 2],
            self.data[self.segment_offset + 3],
        ])
    }

    pub fn set_dest_port(&mut self, port: u16) {
        self.data[self.segment_offset + 2..=self.segment_offset + 3]
            .copy_from_slice(&port.to_be_bytes());
    }

    pub fn length(&self) -> u16 {
        u16::from_be_bytes([
            self.data[self.segment_offset + 4],
            self.data[self.segment_offset + 5],
        ])
    }

    pub fn checksum(&self) -> u16 {
        u16::from_be_bytes([
            self.data[self.segment_offset + 6],
            self.data[self.segment_offset + 7],
        ])
    }

    pub fn payload(&self) -> Cow<[u8]> {
        Cow::from(&self.data[self.segment_offset + 8..])
    }
}

impl<'packet> TryFrom<Ipv4Packet<'packet>> for UdpSegment<'packet> {
    type Error = &'static str;

    fn try_from(packet: Ipv4Packet<'packet>) -> Result<Self, Self::Error> {
        UdpSegment::new(packet.data, packet.packet_offset, packet.payload_offset)
    }
}

impl<'packet> TryFrom<Ipv6Packet<'packet>> for UdpSegment<'packet> {
    type Error = &'static str;

    fn try_from(packet: Ipv6Packet<'packet>) -> Result<Self, Self::Error> {
        UdpSegment::new(packet.data, packet.packet_offset, packet.payload_offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    #[test]
    fn udp_segment() {
        let mut mac_data: Vec<u8> =
            vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let ipv4_data: Vec<u8> = vec![
            0x45, 0, 0, 20, 0, 0, 0, 0, 64, 17, 0, 0, 192, 178, 128, 0, 10, 0, 0, 1,
        ];
        let udp_data: Vec<u8> = vec![
            0, 99, 0, 88, 0, 19, 0xDE, 0xAD, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
        ];

        let mut frame = EthernetFrame::new(&mut mac_data).unwrap();
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
}
