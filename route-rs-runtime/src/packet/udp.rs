use crate::packet::*;
use std::borrow::Cow;

#[allow(dead_code)]
pub struct UdpSegment<'packet> {
    pub data: PacketData<'packet>,
    segment_header_offset: usize,
    ip_version: u8,
    validated_checksum: bool,
}

impl<'packet> UdpSegment<'packet> {
    fn new(segment: PacketData) -> Result<UdpSegment, &'static str> {
        // First let's check that the Frame and IP Header is present
        if segment.len() < 14 + 20 {
            return Err("Segment to short to contain valid IP Header");
        }

        let segment_header_offset;
        let protocol;
        let ip_version = segment[14] & 0xF0 >> 4;
        match ip_version {
            4 => {
                segment_header_offset = 14 + 20;
                protocol = IpProtocol::from(segment[14 + 9]);
            }
            6 => {
                segment_header_offset = 14 + 40;
                // There is an unhandled edge case here, this could specify either the
                // protocol such as TCP, or it could specify the next extension header, which
                // we would have to parse to determine the protocol. Will need some helper functions
                // to support extension headers.
                protocol = IpProtocol::from(segment[14 + 6]);
            }
            _ => {
                return Err("IP Header has invalid version number");
            }
        }

        // See the other note about how we are not Ipv6 compatible yet :(
        if protocol != IpProtocol::UDP {
            return Err("Protocol is incorrect, since it isn't UDP");
        }

        let length = u16::from_be_bytes([
            segment[segment_header_offset + 4],
            segment[segment_header_offset + 5],
        ]);
        if segment.len() > segment_header_offset + length as usize {
            return Err("Segment is not correct length as given by it's length field");
        }

        Ok(UdpSegment {
            data: segment,
            segment_header_offset,
            ip_version,
            validated_checksum: false,
        })
    }

    pub fn src_port(&self) -> u16 {
        u16::from_be_bytes([
            self.data[self.segment_header_offset],
            self.data[self.segment_header_offset + 1],
        ])
    }

    pub fn set_src_port(&mut self, port: u16) {
        self.data[self.segment_header_offset..=self.segment_header_offset + 1]
            .copy_from_slice(&port.to_be_bytes());
    }

    pub fn dest_port(&self) -> u16 {
        u16::from_be_bytes([
            self.data[self.segment_header_offset + 2],
            self.data[self.segment_header_offset + 3],
        ])
    }

    pub fn set_dest_port(&mut self, port: u16) {
        self.data[self.segment_header_offset + 2..=self.segment_header_offset + 3]
            .copy_from_slice(&port.to_be_bytes());
    }

    pub fn length(&self) -> u16 {
        u16::from_be_bytes([
            self.data[self.segment_header_offset + 4],
            self.data[self.segment_header_offset + 5],
        ])
    }

    pub fn checksum(&self) -> u16 {
        u16::from_be_bytes([
            self.data[self.segment_header_offset + 6],
            self.data[self.segment_header_offset + 7],
        ])
    }

    pub fn payload(&self) -> Cow<[u8]> {
        Cow::from(&self.data[self.segment_header_offset + 8..])
    }
}

pub type UdpSegmentResult<'packet> = Result<UdpSegment<'packet>, &'static str>;
impl<'packet> From<EthernetFrame<'packet>> for UdpSegmentResult<'packet> {
    fn from(frame: EthernetFrame<'packet>) -> Self {
        UdpSegment::new(frame.data)
    }
}

impl<'packet> From<Ipv4Packet<'packet>> for UdpSegmentResult<'packet> {
    fn from(packet: Ipv4Packet<'packet>) -> Self {
        UdpSegment::new(packet.data)
    }
}

impl<'packet> From<Ipv6Packet<'packet>> for UdpSegmentResult<'packet> {
    fn from(packet: Ipv6Packet<'packet>) -> Self {
        UdpSegment::new(packet.data)
    }
}
