use crate::packet::*;

#[allow(dead_code)]
pub struct TcpSegment<'packet> {
    pub data: PacketData<'packet>,
    pub packet_offset: usize,
    pub segment_offset: usize,
    ip_version: u8,
    validated_checksum: bool,
}

impl<'packet> TcpSegment<'packet> {
    fn new(segment: PacketData, packet_offset: usize, segment_offset: usize) -> Result<TcpSegment, &'static str> {
        // First let's check that the Frame and IP Header is present
        if segment.len() < packet_offset + 20 {
            return Err("Segment to short to contain valid IP Header");
        }

        let protocol;
        // This also requires reaching into the packet. Not ideal
        let ip_version = segment[packet_offset] & 0xF0 >> 4;
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
        if protocol != IpProtocol::TCP {
            return Err("Protocol is incorrect, since it isn't six");
        }

        if segment.len() > segment_offset + 20 {
            return Err("Segment is too short to have valid TCP Header");
        }

        Ok(TcpSegment {
            data: segment,
            packet_offset,
            segment_offset,
            ip_version,
            validated_checksum: false,
        })
    }
}

pub type TcpSegmentResult<'packet> = Result<TcpSegment<'packet>, &'static str>;

impl<'packet> From<Ipv4Packet<'packet>> for TcpSegmentResult<'packet> {
    fn from(packet: Ipv4Packet<'packet>) -> Self {
        TcpSegment::new(packet.data, packet.packet_offset, packet.packet_offset)
    }
}

impl<'packet> From<Ipv6Packet<'packet>> for TcpSegmentResult<'packet> {
    fn from(packet: Ipv6Packet<'packet>) -> Self {
        TcpSegment::new(packet.data, packet.packet_offset, packet.payload_offset)
    }
}
