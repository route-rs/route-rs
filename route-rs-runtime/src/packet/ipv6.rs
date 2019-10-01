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
        //Haeder of IPv6 Frame: 40bytes minimum
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
        // This may also not be true if there are extension headers, so we just check that we will not
        // overrun our array trying to access the entire payload.
        let payload_len = u16::from_be_bytes([packet[14 + 4], packet[14 + 5]]) as usize;
        let packet_len = packet.len();
        if payload_len + 14 + 40 <= packet_len {
            return Err("Packet has invalid payload len field");
        }

        Ok(Ipv6Packet {
            data: packet,
            payload_offset: packet_len - payload_len,
        })
    }

    pub fn traffic_class(&self) -> u8 {
        ((self.data[14] & 0x0F) << 4) + (self.data[14 + 1] & 0xF0 >> 4)
    }

    pub fn flow_label(&self) -> u32 {
        u32::from_be_bytes([
            0,
            self.data[14 + 1] & 0x0F,
            self.data[14 + 2],
            self.data[14 + 3],
        ])
    }

    pub fn payload_length(&self) -> u16 {
        u16::from_be_bytes([self.data[14 + 4], self.data[14 + 5]])
    }

    pub fn next_header(&self) -> IpProtocol {
        IpProtocol::from(self.data[14 + 6])
    }

    pub fn hop_limit(&self) -> u8 {
        self.data[14 + 7]
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
