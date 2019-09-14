use crate::packet::*;

pub struct UdpSegment<'packet> {
    pub data: PacketData<'packet>,
    segment_header_offset: usize,
    ip_version: u8,
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
                protocol = segment[14 + 9];
            }
            6 => {
                segment_header_offset = 14 + 40;
                // There is an unhandled edge case here, this could specify either the
                // protocol such as TCP, or it could specify the next extension header, which
                // we would have to parse to determine the protocol. Will need some helper functions
                // to support extension headers.
                protocol = segment[14 + 6];
            }
            _ => {
                return Err("IP Header has invalid version number");
            }
        }

        // See the other note about how we are not Ipv6 compatible yet :(
        if protocol != 17 {
            return Err("Protocol is incorrect, since it isn't UDP");
        }

        if segment.len() > segment_header_offset + 8 {
            return Err("Segment is too short to have valid TCP Header");
        }

        Ok(UdpSegment {
            data: segment,
            segment_header_offset,
            ip_version,
        })
    }
}

impl<'packet> From<EthernetFrame<'packet>> for Result<UdpSegment<'packet>, &'static str> {
    fn from(frame: EthernetFrame<'packet>) -> Self {
        UdpSegment::new(frame.data)
    }
}

impl<'packet> From<Ipv4Packet<'packet>> for Result<UdpSegment<'packet>, &'static str> {
    fn from(packet: Ipv4Packet<'packet>) -> Self {
        UdpSegment::new(packet.data)
    }
}

impl<'packet> From<Ipv6Packet<'packet>> for Result<UdpSegment<'packet>, &'static str> {
    fn from(packet: Ipv6Packet<'packet>) -> Self {
        UdpSegment::new(packet.data)
    }
}