use crate::packet::*;

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
        //Haeder of IPv6 Frame: 40bytes
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
        let payload_len = u16::from_be_bytes([packet[14 + 4], packet[14 + 5]]) as usize;
        if payload_len + 14 + 40 != packet.len() {
            return Err("Packet has invalid payload len field");
        }

        //Unhandled edge case where the offset may be different due to extension headers
        Ok(Ipv6Packet {
            data: packet,
            payload_offset: 54,
        })
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
