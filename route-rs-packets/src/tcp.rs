use crate::*;
use std::borrow::Cow;
use std::convert::{TryFrom, TryInto};

#[derive(Clone)]
pub struct TcpSegment {
    pub data: PacketData,
    pub layer2_offset: Option<usize>,
    pub layer3_offset: Option<usize>,
    pub layer4_offset: usize,
    pub payload_offset: usize,
}

impl TcpSegment {
    fn new(
        data: PacketData,
        layer2_offset: Option<usize>,
        layer3_offset: Option<usize>,
        layer4_offset: usize,
    ) -> Result<TcpSegment, &'static str> {
        if data.len() < layer4_offset + 20 {
            return Err("Segment to short to contain valid TCP Header");
        }

        if let Some(layer3_offset) = layer3_offset {
            let protocol;
            let ip_version = (data[layer3_offset] & 0xF0) >> 4;
            match ip_version {
                4 => {
                    protocol = get_ipv4_payload_type(&data, layer3_offset)
                        .expect("Malformed IPv4 Header in TcpSegment");
                }
                6 => {
                    protocol = get_ipv6_payload_type(&data, layer3_offset)
                        .expect("Malformed IPv6 Header in TcpSegment");
                }
                _ => {
                    return Err("IP Header has invalid version number");
                }
            }

            if protocol != IpProtocol::TCP {
                return Err("Protocol is incorrect, since it isn't six");
            }
        }

        let payload_offset =
            layer4_offset + (((data[layer4_offset + 12] & 0xF0) >> 4) as usize * 4);

        Ok(TcpSegment {
            data,
            layer2_offset,
            layer3_offset,
            layer4_offset,
            payload_offset,
        })
    }

    pub fn src_port(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.layer4_offset..=self.layer4_offset + 1]
                .try_into()
                .unwrap(),
        )
    }

    pub fn set_src_port(&mut self, port: u16) {
        self.data[self.layer4_offset..=self.layer4_offset + 1].copy_from_slice(&port.to_be_bytes());
    }

    pub fn dest_port(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.layer4_offset + 2..=self.layer4_offset + 3]
                .try_into()
                .unwrap(),
        )
    }

    pub fn set_dest_port(&mut self, port: u16) {
        self.data[self.layer4_offset + 2..=self.layer4_offset + 3]
            .copy_from_slice(&port.to_be_bytes());
    }

    pub fn sequence_number(&self) -> u32 {
        u32::from_be_bytes(
            self.data[self.layer4_offset + 4..=self.layer4_offset + 7]
                .try_into()
                .unwrap(),
        )
    }

    pub fn acknowledgment_number(&self) -> u32 {
        u32::from_be_bytes(
            self.data[self.layer4_offset + 8..=self.layer4_offset + 11]
                .try_into()
                .unwrap(),
        )
    }

    pub fn data_offset(&self) -> u8 {
        (self.data[self.layer4_offset + 12] & 0xF0) >> 4
    }

    /// Data offset is the value wanted in BYTES
    pub fn set_data_offset(&mut self, data_offset: usize) {
        self.data[self.layer4_offset + 12] &= 0xF0;
        self.data[self.layer4_offset + 12] |= (((data_offset / 4) << 4) & 0xF0) as u8;
        self.payload_offset = data_offset;
    }

    /// Returns the 9 control bits as a u16, the 9 least significant bits
    /// represent the bits in question
    pub fn control_bits(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.layer4_offset + 12..=self.layer4_offset + 13]
                .try_into()
                .unwrap(),
        ) & 0x01FF
    }

    pub fn window_size(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.layer4_offset + 14..=self.layer4_offset + 15]
                .try_into()
                .unwrap(),
        )
    }

    pub fn checksum(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.layer4_offset + 16..=self.layer4_offset + 17]
                .try_into()
                .unwrap(),
        )
    }

    pub fn urgent_pointer(&self) -> u16 {
        u16::from_be_bytes(
            self.data[self.layer4_offset + 18..=self.layer4_offset + 19]
                .try_into()
                .unwrap(),
        )
    }

    pub fn options(&self) -> Option<Cow<[u8]>> {
        if self.data_offset() <= 5 {
            return None;
        }
        Some(Cow::from(
            &self.data[self.layer4_offset + 20..self.payload_offset],
        ))
    }

    /// Sets the options of the tcp segment to the provided array, also
    /// sets the data_offset field of the packet, and the internal payload_offset
    /// field.
    /// To be valid, options must be padded by the user to 32bits
    pub fn set_options(&mut self, options: &[u8]) {
        let payload = self.data.split_off(self.payload_offset);
        self.data.truncate(self.layer4_offset + 20);
        self.data.reserve_exact(payload.len() + options.len());
        self.data.extend(options);
        self.data.extend(payload);
        self.set_data_offset(options.len() + 20);
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
    }

    //TODO: Create functions to calculate and set checksum.
}

impl TryFrom<Ipv4Packet> for TcpSegment {
    type Error = &'static str;

    fn try_from(packet: Ipv4Packet) -> Result<Self, Self::Error> {
        TcpSegment::new(
            packet.data,
            packet.layer2_offset,
            Some(packet.layer3_offset),
            packet.payload_offset,
        )
    }
}

impl TryFrom<Ipv6Packet> for TcpSegment {
    type Error = &'static str;

    fn try_from(packet: Ipv6Packet) -> Result<Self, Self::Error> {
        TcpSegment::new(
            packet.data,
            packet.layer2_offset,
            Some(packet.layer3_offset),
            packet.payload_offset,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    #[test]
    fn tcp_segment() {
        let mac_data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let ipv4_data: Vec<u8> = vec![
            0x45, 0, 0, 20, 0, 0, 0, 0, 64, 6, 0, 0, 192, 178, 128, 0, 10, 0, 0, 1,
        ];
        let tcp_data: Vec<u8> = vec![
            0, 99, 0, 88, 0, 0, 0, 2, 0, 0, 0, 8, 0x50, 0xFF, 0, 16, 0xDE, 0xAD, 0xBE, 0xEF, 0, 1,
            2, 3, 4, 5, 6, 7, 8, 9, 10,
        ];

        let mut frame = EthernetFrame::new(mac_data, 0).unwrap();
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
