use crate::packet::*;
use std::borrow::Cow;
use std::convert::TryFrom;

pub struct EthernetFrame<'packet> {
    pub data: PacketData<'packet>,
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

        Ok(EthernetFrame { data: frame })
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
        for i in 0..6 {
            self.data[i] = mac.bytes[i];
        }
    }

    pub fn set_src_mac(&mut self, mac: MacAddr) {
        for i in 0..6 {
            self.data[6 + i] = mac.bytes[i];
        }
    }

    pub fn payload_len(&self) -> u16 {
        u16::from_be_bytes([self.data[12], self.data[13]])
    }

    //This gives you a cow of a slice of the payload.
    pub fn payload(&self) -> Cow<[u8]> {
        Cow::from(&self.data[14..])
    }

    pub fn set_payload(&mut self, payload: &[u8]) {
        let payload_len = payload.len() as u16;
        self.data.truncate(12); //Cut off and drop the entire payload and payload_len
        let payload_len_bytes = payload_len.to_be_bytes();
        self.data.reserve_exact(payload_len as usize + 2); //Reserve room for payload and len field.
        self.data.extend(payload_len_bytes.iter());
        self.data.extend(payload);
    }
}

impl<'packet> From<TcpSegment<'packet>> for Result<EthernetFrame<'packet>, &'static str> {
    fn from(segment: TcpSegment<'packet>) -> Self {
        EthernetFrame::new(segment.data)
    }
}

impl<'packet> From<UdpSegment<'packet>> for Result<EthernetFrame<'packet>, &'static str> {
    fn from(segment: UdpSegment<'packet>) -> Self {
        EthernetFrame::new(segment.data)
    }
}

impl<'packet> From<Ipv4Packet<'packet>> for Result<EthernetFrame<'packet>, &'static str> {
    fn from(packet: Ipv4Packet<'packet>) -> Self {
        EthernetFrame::new(packet.data)
    }
}

impl<'packet> From<Ipv6Packet<'packet>> for Result<EthernetFrame<'packet>, &'static str> {
    fn from(packet: Ipv6Packet<'packet>) -> Self {
        EthernetFrame::new(packet.data)
    }
}
