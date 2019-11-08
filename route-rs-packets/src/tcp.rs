use crate::*;
use std::borrow::Cow;
use std::convert::{TryFrom, TryInto};

#[derive(Clone)]
pub struct TcpSegment {
    pub data: PacketData,
    pub packet_offset: usize,
    pub segment_offset: usize,
    pub payload_offset: usize,
    ip_version: u8,
    validated_checksum: bool,
}

impl TcpSegment {
    fn new(
        segment: PacketData,
        packet_offset: usize,
        segment_offset: usize,
    ) -> Result<TcpSegment, &'static str> {
        // First let's check that the Frame and IP Header is present
        if segment.len() < packet_offset + 20 {
            return Err("Segment to short to contain valid IP Header");
        }

        let protocol;
        // This requires reaching into the packet. Not ideal
        let ip_version = (segment[packet_offset] & 0xF0) >> 4;
        match ip_version {
            4 => {
                protocol = get_ipv4_payload_type(&segment, packet_offset)
                    .expect("Malformed IPv4 Header in TcpSegment");
            }
            6 => {
                protocol = get_ipv6_payload_type(&segment, packet_offset)
                    .expect("Malformed IPv6 Header in TcpSegment");
            }
            _ => {
                return Err("IP Header has invalid version number");
            }
        }

        if protocol != IpProtocol::TCP {
            return Err("Protocol is incorrect, since it isn't six");
        }

        if segment.len() < segment_offset + 20 {
            return Err("Segment is too short to have valid TCP Header");
        }

        let payload_offset =
            segment_offset + (((segment[segment_offset + 12] & 0xF0) >> 4) as usize * 4);

        Ok(TcpSegment {
            data: segment,
            packet_offset,
            segment_offset,
            payload_offset,
            ip_version,
            validated_checksum: false,
        })
    }

    pub fn src_port(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.segment_offset..=self.segment_offset + 1]
                .try_into()
                .unwrap(),
        )
    }

    pub fn set_src_port(&mut self, port: u16) {
        self.data[self.segment_offset..=self.segment_offset + 1]
            .copy_from_slice(&port.to_be_bytes());
    }

    pub fn dest_port(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.segment_offset + 2..=self.segment_offset + 3]
                .try_into()
                .unwrap(),
        )
    }

    pub fn set_dest_port(&mut self, port: u16) {
        self.data[self.segment_offset + 2..=self.segment_offset + 3]
            .copy_from_slice(&port.to_be_bytes());
    }

    pub fn sequence_number(&self) -> u32 {
        u32::from_be_bytes(
            self.data[self.segment_offset + 4..=self.segment_offset + 7]
                .try_into()
                .unwrap(),
        )
    }

    pub fn acknowledgment_number(&self) -> u32 {
        u32::from_be_bytes(
            self.data[self.segment_offset + 8..=self.segment_offset + 11]
                .try_into()
                .unwrap(),
        )
    }

    pub fn data_offset(&self) -> u8 {
        (self.data[self.segment_offset + 12] & 0xF0) >> 4
    }

    /// Data offset is the value wanted in BYTES
    pub fn set_data_offset(&mut self, data_offset: usize) {
        self.data[self.segment_offset + 12] &= 0xF0;
        self.data[self.segment_offset + 12] |= (((data_offset / 4) << 4) & 0xF0) as u8;
        self.payload_offset = data_offset;
    }

    /// Returns the 9 control bits as a u16, the 9 least significant bits
    /// represent the bits in question
    pub fn control_bits(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.segment_offset + 12..=self.segment_offset + 13]
                .try_into()
                .unwrap(),
        ) & 0x01FF
    }

    pub fn window_size(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.segment_offset + 14..=self.segment_offset + 15]
                .try_into()
                .unwrap(),
        )
    }

    pub fn checksum(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.segment_offset + 16..=self.segment_offset + 17]
                .try_into()
                .unwrap(),
        )
    }

    pub fn urgent_pointer(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.segment_offset + 18..=self.segment_offset + 19]
                .try_into()
                .unwrap(),
        )
    }

    pub fn options(&self) -> Option<Cow<[u8]>> {
        if self.data_offset() <= 5 {
            return None;
        }
        Some(Cow::from(
            &self.data[self.segment_offset + 20..self.payload_offset],
        ))
    }

    /// Sets the options of the tcp segment to the provided array, also
    /// sets the data_offset field of the packet, and the internal payload_offset
    /// field.
    /// To be valid, options must be padded by the user to 32bits
    pub fn set_options(&mut self, options: &[u8]) {
        let payload = self.data.split_off(self.payload_offset);
        self.data.truncate(self.segment_offset + 20);
        self.data.reserve_exact(payload.len() + options.len());
        self.data.extend(options);
        self.data.extend(payload);
        self.set_data_offset(options.len() + 20);
        self.validated_checksum = false;
    }

    pub fn payload(&self) -> Cow<[u8]> {
        Cow::from(&self.data[self.payload_offset..])
    }

    /// Sets TCP payload, if you change the length of the payload, you should
    /// go and reset the length field in the relevant Ipv4 or Ipv6 field, lest you
    /// create an invalid packet.
    pub fn set_payload(&mut self, payload: &[u8]) {
        let payload_len = payload.len();
        self.data.truncate(self.payload_offset);
        self.data.reserve_exact(payload_len);
        self.data.extend(payload);
        self.validated_checksum = false;
    }

    //TODO: Create functions to calculate and set checksum.
}

impl TryFrom<Ipv4Packet> for TcpSegment {
    type Error = &'static str;

    fn try_from(packet: Ipv4Packet) -> Result<Self, Self::Error> {
        TcpSegment::new(packet.data, packet.packet_offset, packet.payload_offset)
    }
}

impl TryFrom<Ipv6Packet> for TcpSegment {
    type Error = &'static str;

    fn try_from(packet: Ipv6Packet) -> Result<Self, Self::Error> {
        TcpSegment::new(packet.data, packet.packet_offset, packet.payload_offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    #[test]
    fn tcp_segment() {
        let mac_data: Vec<u8> =
            vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let ipv4_data: Vec<u8> = vec![
            0x45, 0, 0, 20, 0, 0, 0, 0, 64, 6, 0, 0, 192, 178, 128, 0, 10, 0, 0, 1,
        ];
        let tcp_data: Vec<u8> = vec![
            0, 99, 0, 88, 0, 0, 0, 2, 0, 0, 0, 8, 0x50, 0xFF, 0, 16, 0xDE, 0xAD, 0xBE, 0xEF, 0, 1,
            2, 3, 4, 5, 6, 7, 8, 9, 10,
        ];

        let mut frame = EthernetFrame::new(mac_data).unwrap();
        frame.set_payload(&ipv4_data);
        let mut packet = Ipv4Packet::try_from(frame).unwrap();
        packet.set_payload(&tcp_data);
        let segment = TcpSegment::try_from(packet).unwrap();

        assert_eq!(segment.src_port(), 99);
        assert_eq!(segment.dest_port(), 88);
        assert_eq!(segment.sequence_number(), 2);
        assert_eq!(segment.acknowledgment_number(), 8);
        assert_eq!(segment.data_offset(), 5);
        assert_eq!(segment.control_bits(), 0x00FF);
        assert_eq!(segment.window_size(), 16);
        assert_eq!(segment.checksum(), 0xDEAD);
        assert_eq!(segment.urgent_pointer(), 0xBEEF);
        assert_eq!(segment.options(), None);
        assert_eq!(segment.payload().len(), 11);
        assert_eq!(segment.payload()[0], 0);
    }
}
