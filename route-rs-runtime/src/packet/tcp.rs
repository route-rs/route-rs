use crate::packet::*;

#[allow(dead_code)]
pub struct TcpSegment<'packet> {
    pub data: PacketData<'packet>,
    segment_header_offset: usize,
    ip_version: u8,
}

impl<'packet> TcpSegment<'packet> {
    fn new(segment: PacketData) -> Result<TcpSegment, &'static str> {
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
        if protocol != 6 {
            return Err("Protocol is incorrect, since it isn't six");
        }

        if segment.len() > segment_header_offset + 20 {
            return Err("Segment is too short to have valid TCP Header");
        }

        Ok(TcpSegment {
            data: segment,
            segment_header_offset,
            ip_version,
        })
    }
}

pub type TcpSegmentResult<'packet> = Result<TcpSegment<'packet>, &'static str>;

impl<'packet> From<EthernetFrame<'packet>> for TcpSegmentResult<'packet> {
    fn from(frame: EthernetFrame<'packet>) -> Self {
        TcpSegment::new(frame.data)
    }
}

impl<'packet> From<Ipv4Packet<'packet>> for TcpSegmentResult<'packet> {
    fn from(packet: Ipv4Packet<'packet>) -> Self {
        TcpSegment::new(packet.data)
    }
}

impl<'packet> From<Ipv6Packet<'packet>> for TcpSegmentResult<'packet> {
    fn from(packet: Ipv6Packet<'packet>) -> Self {
        TcpSegment::new(packet.data)
    }
}
