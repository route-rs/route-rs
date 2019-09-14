use crate::packet::*;


pub struct Ipv4Packet<'packet> {
    pub data: PacketData<'packet>,
}

impl<'packet> Ipv4Packet<'packet> {
    fn new(packet: PacketData) -> Result<Ipv4Packet, &'static str> {
        //Header of Ethernet Frame: 14 bytes
        //Header of IPv4 Frame: 20 bytes
        if packet.len() < 14 + 20 {
            return Err("Data is too short to be an IPv4 Packet");
        }

        //Check version number
        let version: u8 = (packet[14] & 0xF0) >> 4;
        if version != 4 {
            return Err("Packet has incorrect version, is not Ipv4Packet");
        }

        // TotalLen is the 3rd and 4th byte of the IP Header
        let total_len = u16::from_be_bytes([packet[14 + 2], packet[14 + 3]]) as usize;
        if packet.len() != total_len + 14 {
            return Err("Packet has invalid total len field");
        }

        Ok(Ipv4Packet { data: packet })
    }

    //Version u4 -> should be 0100, for ipv4
    //HeaderLen u4 -> header length in 32bit words, min total 20bytes
    //TypeofService u8 ->  RFC2474, some QoS stuff
    //TotalLen u16 -> total length in bytes, min is 20 bytes, max is 65353, since well 16 bits
    //Identification u16 -> Fragment ID, identifies fragmented IP for reassembly later.
    //[ O, DF, MF, Fragment Offset (13 bits)] -> Also for frag stuff
    //TTL u8 -> Should be dropped it zero
    //Protocol u8 -> 1 ICMP, 6 TCP, 17 UDP, 41 IPv6 tun over ipv4
    //Header Checksum u16 -> Ones compliment of the ones complement sum of all 16 bit words in the header
    //src_ip u32
    //dst_ip 32
    //options (0 - 40 bytes), this will be annoying, hardly ever used "in the wild"
    //Payload

    //Validate_header
    //Validate_length
    //Validate Versoin
    //Get Protocol enum
    //payload_offset
    //from(EthernetFrame)
}

// This is the bread and butter of this whole library, it allows us to 'promote' a packet
// from one type to another, theoretically with good checking. Needs some work right now.
// What I really don't want is to have to copy the bytes around during type changing.
impl<'packet> From<EthernetFrame<'packet>> for Result<Ipv4Packet<'packet>, &'static str> {
    fn from(frame: EthernetFrame<'packet>) -> Self {
        Ipv4Packet::new(frame.data)
    }
}

impl<'packet> From<TcpSegment<'packet>> for Result<Ipv4Packet<'packet>, &'static str> {
    fn from(segment: TcpSegment<'packet>) -> Self {
        Ipv4Packet::new(segment.data)
    }
}

impl<'packet> From<UdpSegment<'packet>> for Result<Ipv4Packet<'packet>, &'static str> {
    fn from(segment: UdpSegment<'packet>) -> Self {
        Ipv4Packet::new(segment.data)
    }
}