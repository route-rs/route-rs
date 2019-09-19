use crate::packet::*;
use std::borrow::Cow;
use std::convert::TryFrom;

pub struct Ipv4Packet<'packet> {
    pub data: PacketData<'packet>,
    header_length: usize,
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

        //This is the header length in 32bit words
        let internet_header_len = (packet[14] & 0x0F) as usize;
        let header_length = (internet_header_len * 4) + 14;

        Ok(Ipv4Packet {
            data: packet,
            header_length,
        })
    }

    //MAGIC ALERT, src addr offset (12) and Ipv4 header offset
    pub fn src_addr(&self) -> Ipv4Addr {
        let bytes = <[u8; 4]>::try_from(&self.data[(14 + 12)..(14 + 15)]).unwrap();
        Ipv4Addr::new(bytes)
    }

    pub fn set_src_addr(&mut self, addr: Ipv4Addr) {
        self.data[14 + 12..14 + 4].clone_from_slice(&addr.bytes[..4]);
    }

    //MAGIC ALERT, dest addr offset (16) and Ipv4 header offset
    pub fn dest_addr(&self) -> Ipv4Addr {
        let bytes = <[u8; 4]>::try_from(&self.data[(14 + 16)..(14 + 19)]).unwrap();
        Ipv4Addr::new(bytes)
    }

    pub fn set_dest_addr(&mut self, addr: Ipv4Addr) {
        self.data[14 + 16..14 + 20].clone_from_slice(&addr.bytes[..4]);
    }

    /// Returns header length in bytes
    pub fn header_length(&self) -> usize {
        self.header_length
    }

    pub fn payload(&self) -> Cow<[u8]> {
        Cow::from(&self.data[14 + self.header_length..])
    }

    pub fn set_payload(&mut self, payload: &[u8]) {
        let payload_len = payload.len() as u16;

        self.data.truncate(self.header_length);

        let total_len = (payload_len + self.header_length as u16).to_be_bytes();
        self.data[14 + 2] = total_len[0];
        self.data[14 + 3] = total_len[1];

        self.data.reserve_exact(payload_len as usize);
        self.data.extend(payload);
    }

    pub fn options(&self) -> Option<Cow<[u8]>> {
        if self.header_length > 20 {
            return None;
        }
        Some(Cow::from(&self.data[14 + 20..14 + self.header_length]))
    }

    pub fn protocol(&self) -> IpProtocol {
        let num = self.data[14 + 9];
        IpProtocol::from(num)
    }

    pub fn total_len(&self) -> u16 {
        u16::from_be_bytes([self.data[14 + 2], self.data[14 + 3]])
    }

    //TypeofService u8 ->  RFC2474, some QoS stuff
    //TotalLen u16 -> total length in bytes, min is 20 bytes, max is 65353, since well 16 bits
    //Identification u16 -> Fragment ID, identifies fragmented IP for reassembly later.
    //[ O, DF, MF, Fragment Offset (13 bits)] -> Also for frag stuff
    //TTL u8 -> Should be dropped it zero
    //Protocol u8 -> 1 ICMP, 6 TCP, 17 UDP, 41 IPv6 tun over ipv4
    //Header Checksum u16 -> Ones compliment of the ones complement sum of all 16 bit words in the header

    //Validate_header
    //Validate_length
    //Validate Versoin
    //Get Protocol enum
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
