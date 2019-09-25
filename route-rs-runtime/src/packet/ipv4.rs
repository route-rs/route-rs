use crate::packet::*;
use std::borrow::Cow;
use std::convert::TryFrom;

pub struct Ipv4Packet<'packet> {
    pub data: PacketData<'packet>,
    header_length: usize,
    valid_checksum: bool,
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
            return Err("Packet has invalid total length field");
        }

        //This is the header length in 32bit words
        let internet_header_len = (packet[14] & 0x0F) as usize;
        let header_length = internet_header_len * 4;

        Ok(Ipv4Packet {
            data: packet,
            header_length,
            valid_checksum: true,
        })
    }

    //MAGIC ALERT, src addr offset (12) and Ipv4 header offset
    pub fn src_addr(&self) -> Ipv4Addr {
        let bytes = <[u8; 4]>::try_from(&self.data[(14 + 12)..(14 + 16)]).unwrap();
        Ipv4Addr::new(bytes)
    }

    pub fn set_src_addr(&mut self, addr: Ipv4Addr) {
        self.data[14 + 12..14 + 4].copy_from_slice(&addr.bytes[..4]);
        self.valid_checksum = false;
    }

    //MAGIC ALERT, dest addr offset (16) and Ipv4 header offset
    pub fn dest_addr(&self) -> Ipv4Addr {
        let bytes = <[u8; 4]>::try_from(&self.data[(14 + 16)..(14 + 20)]).unwrap();
        Ipv4Addr::new(bytes)
    }

    pub fn set_dest_addr(&mut self, addr: Ipv4Addr) {
        self.data[14 + 16..14 + 20].copy_from_slice(&addr.bytes[..4]);
        self.valid_checksum = false;
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
        self.valid_checksum = false;
    }

    pub fn options(&self) -> Option<Cow<[u8]>> {
        if self.header_length <= 20 {
            return None;
        }
        Some(Cow::from(&self.data[14 + 20..14 + self.header_length]))
    }

    //set_options

    pub fn protocol(&self) -> IpProtocol {
        let num = self.data[14 + 9];
        IpProtocol::from(num)
    }

    pub fn total_len(&self) -> u16 {
        u16::from_be_bytes([self.data[14 + 2], self.data[14 + 3]])
    }

    pub fn ttl(&self) -> u8 {
        self.data[14 + 8]
    }

    pub fn set_ttl(&mut self, ttl: u8) {
        self.data[14 + 8] = ttl;
        self.valid_checksum = false;
    }

    pub fn header_checksum(&self) -> u16 {
        u16::from_be_bytes([self.data[14 + 10], self.data[14 + 11]])
    }

    pub fn dcsp(&self) -> u8 {
        self.data[14 + 1] >> 2
    }

    pub fn ecn(&self) -> u8 {
        self.data[14 + 1] & 0x03
    }

    pub fn indentification(&self) -> u16 {
        u16::from_be_bytes([self.data[14 + 4], self.data[14 + 5]])
    }

    pub fn fragment_offet(&self) -> u16 {
        u16::from_be_bytes([self.data[14 + 6] & 0x1F, self.data[14 + 7]])
    }

    ///Returns tuple of (Don't Fragment, More Fragments)
    pub fn flags(&self) -> (bool, bool) {
        let df = (self.data[14 + 6] & 0x40) != 0;
        let mf = (self.data[14 + 6] & 0x20) != 0;
        (df, mf)
    }

    /// Verifies the IP header checksum, returns the value and also sets
    /// the internal bookeeping field. As such we need a mutable reference.
    pub fn validate_checksum(&mut self) -> bool {
        let full_sum = &self.data[14..14 + self.header_length()]
            .chunks_exact(2)
            .fold(0, |acc: u32, x| {
                acc + u32::from(u16::from_be_bytes([x[0], x[1]]))
            });
        let (carry, mut sum) = (((full_sum & 0xFFFF_0000) >> 16), (full_sum & 0x0000_FFFF));
        sum += carry;
        self.valid_checksum = 0 == (!sum & 0xFFFF);
        self.valid_checksum
    }

    /// Calculates what the checksum should be set to given the current header
    pub fn caclulate_checksum(&self) -> u16 {
        let full_sum = &self.data[14..14 + self.header_length()]
            .chunks_exact(2)
            .enumerate()
            .filter(|x| x.0 != 5)
            .fold(0, |acc: u32, x| {
                acc + u32::from(u16::from_be_bytes([x.1[0], x.1[1]]))
            });
        let (carry, mut sum) = (((full_sum & 0xFFFF_0000) >> 16), (full_sum & 0x0000_FFFF));
        sum += carry;
        if sum & 0xFFFF_0000 != 0 {
            sum += 1;
        }
        sum = !sum & 0xFFFF;
        sum as u16
    }

    /// Sets checksum field to valid value
    pub fn set_checksum(&mut self) {
        let new_checksum = self.caclulate_checksum();
        self.data[14 + 10] = ((new_checksum & 0xFF00) >> 8) as u8;
        self.data[14 + 11] = (new_checksum & 0x00FF) as u8;
        self.valid_checksum = true;
    }
}

pub type Ipv4PacketResult<'packet> = Result<Ipv4Packet<'packet>, &'static str>;

impl<'packet> From<EthernetFrame<'packet>> for Ipv4PacketResult<'packet> {
    fn from(frame: EthernetFrame<'packet>) -> Self {
        Ipv4Packet::new(frame.data)
    }
}

impl<'packet> From<TcpSegment<'packet>> for Ipv4PacketResult<'packet> {
    fn from(segment: TcpSegment<'packet>) -> Self {
        Ipv4Packet::new(segment.data)
    }
}

impl<'packet> From<UdpSegment<'packet>> for Ipv4PacketResult<'packet> {
    fn from(segment: UdpSegment<'packet>) -> Self {
        Ipv4Packet::new(segment.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    #[test]
    fn ipv4_packet() {
        //Example frame with 0 payload
        let mut mac_data: Vec<u8> =
            vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let ip_data: Vec<u8> = vec![
            0x45, 0, 0, 20, 0, 0, 0, 0, 64, 17, 0, 0, 192, 178, 128, 0, 10, 0, 0, 1,
        ];

        let mut frame = EthernetFrame::new(&mut mac_data).unwrap();
        frame.set_payload(&ip_data);

        let packet = Ipv4PacketResult::from(frame).unwrap();

        assert_eq!(packet.src_addr(), Ipv4Addr::new([192, 178, 128, 0]));
        assert_eq!(packet.dest_addr(), Ipv4Addr::new([10, 0, 0, 1]));
        assert_eq!(packet.header_length(), 20);
        assert_eq!(packet.payload().len(), 0);
        assert_eq!(packet.options(), None);
        assert_eq!(packet.protocol(), IpProtocol::UDP);
        assert_eq!(packet.total_len(), 20);
        assert_eq!(packet.ttl(), 64);
        assert_eq!(packet.header_checksum(), 0);
        assert_eq!(packet.dcsp(), 0);
        assert_eq!(packet.ecn(), 0);
        assert_eq!(packet.indentification(), 0);
        assert_eq!(packet.fragment_offet(), 0);
        assert_eq!(packet.flags(), (false, false));
    }

    #[test]
    fn validate_checksum() {
        let mut mac_data: Vec<u8> =
            vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let invalid_checksum_data: Vec<u8> = vec![
            0x45, 0x00, 0x00, 0x14, 0x00, 0x00, 0x40, 0x00, 0x40, 0x11, 0xb8, 0x61, 0xc0, 0xa8,
            0x00, 0x01, 0xc0, 0xa8, 0x00, 0xc7,
        ];
        let mut frame = EthernetFrame::new(&mut mac_data).unwrap();
        frame.set_payload(&invalid_checksum_data);
        let mut packet = Ipv4PacketResult::from(frame).unwrap();
        assert!(!packet.validate_checksum());

        let valid_checksum_data: Vec<u8> = vec![
            0x45, 0x00, 0x00, 0x14, 0x00, 0x00, 0x40, 0x00, 0x40, 0x11, 0xb8, 0xc0, 0xc0, 0xa8,
            0x00, 0x01, 0xc0, 0xa8, 0x00, 0xc7,
        ];
        let mut frame = EthernetFrameResult::from(packet).unwrap();
        frame.set_payload(&valid_checksum_data);
        let mut packet = Ipv4PacketResult::from(frame).unwrap();
        assert!(packet.validate_checksum());
    }

    #[test]
    fn set_checksum() {
        let mut mac_data: Vec<u8> =
            vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let ip_data: Vec<u8> = vec![
            0x45, 0x00, 0x00, 0x14, 0x00, 0x00, 0x40, 0x00, 0x40, 0x11, 0xb8, 0x61, 0xc0, 0xa8,
            0x00, 0x01, 0xc0, 0xa8, 0x00, 0xc7,
        ];
        let mut frame = EthernetFrame::new(&mut mac_data).unwrap();
        frame.set_payload(&ip_data);
        let mut packet = Ipv4PacketResult::from(frame).unwrap();
        assert!(!packet.validate_checksum());
        packet.set_checksum();
        assert!(packet.validate_checksum());
    }
}
