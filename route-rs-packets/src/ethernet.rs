use crate::*;
use std::borrow::Cow;
use std::convert::{TryFrom, TryInto};

pub struct EthernetFrame<'packet> {
    pub data: PacketData<'packet>,
    pub payload_offset: usize,
}

impl<'frame> EthernetFrame<'frame> {
    pub fn new(frame: PacketData) -> Result<EthernetFrame, &'static str> {
        // Ethernet II frames must be at least the header, which is 14bytes
        // 0                    6                    12                      14
        // |---6 byte Dest_MAC--|---6 byte Src_MAC---|--2 Byte Payload Len---|
        // We could support other formats for the frames, but IP sits atop Ethernet II

        // If we want to have formated errors, we will need to create an object that implements
        // Error that we can return, rather than making the error a &'static str
        // Will create an issue, so it can be done later.
        if frame.len() < 14 {
            return Err("Frame is less than the minimum of 14 bytes");
        }
        //Can't use helper here, since we don't have the object yet :(.
        let payload_len = u16::from_be_bytes([frame[12], frame[13]]) as usize;
        if payload_len + 14 != frame.len() {
            return Err("Frame has invalivd payload len field");
        }

        Ok(EthernetFrame {
            data: frame,
            payload_offset: 14,
        })
    }

    pub fn dest_mac(&self) -> MacAddr {
        let bytes = <[u8; 6]>::try_from(&self.data[0..6]).unwrap();
        MacAddr::new(bytes)
    }

    pub fn src_mac(&self) -> MacAddr {
        let bytes = <[u8; 6]>::try_from(&self.data[6..12]).unwrap();
        MacAddr::new(bytes)
    }

    pub fn set_dest_mac(&mut self, mac: MacAddr) {
        self.data[..6].copy_from_slice(&mac.bytes[..6]);
    }

    pub fn set_src_mac(&mut self, mac: MacAddr) {
        self.data[6..12].copy_from_slice(&mac.bytes[..6]);
    }

    pub fn payload_len(&self) -> u16 {
        u16::from_be_bytes(self.data[12..=13].try_into().unwrap())
    }

    //This gives you a cow of a slice of the payload.
    pub fn payload(&self) -> Cow<[u8]> {
        Cow::from(&self.data[self.payload_offset..])
    }

    pub fn set_payload(&mut self, payload: &[u8]) {
        let payload_len = payload.len() as u16;
        self.data.truncate(self.payload_offset - 2); //Cut off and drop the entire payload and payload_len
        let payload_len_bytes = payload_len.to_be_bytes();
        self.data.reserve_exact(payload_len as usize + 2); //Reserve room for payload and len field.
        self.data.extend(payload_len_bytes.iter());
        self.data.extend(payload);
    }
}

impl<'packet> TryFrom<TcpSegment<'packet>> for EthernetFrame<'packet> {
    type Error = &'static str;

    fn try_from(segment: TcpSegment<'packet>) -> Result<Self, Self::Error> {
        EthernetFrame::new(segment.data)
    }
}

impl<'packet> TryFrom<UdpSegment<'packet>> for EthernetFrame<'packet> {
    type Error = &'static str;

    fn try_from(segment: UdpSegment<'packet>) -> Result<Self, Self::Error> {
        EthernetFrame::new(segment.data)
    }
}

impl<'packet> TryFrom<Ipv4Packet<'packet>> for EthernetFrame<'packet> {
    type Error = &'static str;

    fn try_from(packet: Ipv4Packet<'packet>) -> Result<Self, Self::Error> {
        EthernetFrame::new(packet.data)
    }
}

impl<'packet> TryFrom<Ipv6Packet<'packet>> for EthernetFrame<'packet> {
    type Error = &'static str;

    fn try_from(packet: Ipv6Packet<'packet>) -> Result<Self, Self::Error> {
        EthernetFrame::new(packet.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    #[test]
    fn ethernet_frame() {
        //Example frame with 0 payload
        let mut data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let frame = EthernetFrame::new(&mut data).unwrap();
        assert_eq!(
            frame.dest_mac(),
            MacAddr::new([0xde, 0xad, 0xbe, 0xef, 0xff, 0xff])
        );
        assert_eq!(frame.src_mac(), MacAddr::new([1, 2, 3, 4, 5, 6]));
        assert_eq!(frame.payload_len(), 0);
        assert_eq!(frame.payload().len(), 0);
    }

    #[test]
    fn set_payload() {
        let mut data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let mut frame = EthernetFrame::new(&mut data).unwrap();
        assert_eq!(frame.payload_len(), 0);
        assert_eq!(frame.payload().len(), 0);

        let new_payload: Vec<u8> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
        frame.set_payload(&new_payload);
        assert_eq!(frame.payload_len(), new_payload.len() as u16);
        assert_eq!(frame.payload(), new_payload);
        assert_eq!(frame.payload()[2], 3);
    }

    #[test]
    #[should_panic(expected = "Frame is less than the minimum of 14 bytes")]
    fn invalid_data_length() {
        let mut data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6];
        let _frame = EthernetFrame::new(&mut data).unwrap();
    }

    #[test]
    #[should_panic(expected = "Frame has invalivd payload len field")]
    fn invalid_payload_length() {
        let mut data: Vec<u8> = vec![
            0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0xff, 1, 2,
        ];
        let _frame = EthernetFrame::new(&mut data).unwrap();
    }

    #[test]
    fn set_dest_mac() {
        let mut data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let mut frame = EthernetFrame::new(&mut data).unwrap();
        let new_dest = MacAddr::new([0x98, 0x88, 0x18, 0x12, 0xb4, 0xdf]);
        frame.set_dest_mac(new_dest);
        assert_eq!(frame.dest_mac(), new_dest);
    }

    #[test]
    fn set_src_mac() {
        let mut data: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0];
        let mut frame = EthernetFrame::new(&mut data).unwrap();
        let new_src = MacAddr::new([0x98, 0x88, 0x18, 0x12, 0xb4, 0xdf]);
        frame.set_src_mac(new_src);
        assert_eq!(frame.src_mac(), new_src);
    }
}
