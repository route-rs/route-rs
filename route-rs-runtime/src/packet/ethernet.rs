use std::borrow::Cow;
use crate::packet::*;

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
        MacAddr::new(
            self.data[0],
            self.data[1],
            self.data[2],
            self.data[3],
            self.data[4],
            self.data[5],
        )
    }

    pub fn src_mac(&self) -> MacAddr {
        MacAddr::new(
            self.data[6],
            self.data[7],
            self.data[8],
            self.data[9],
            self.data[10],
            self.data[11],
        )
    }

    pub fn payload_len(&self) -> u16 {
        u16::from_be_bytes([self.data[12], self.data[13]])
    }

    //This gives you a cow of a slice of the payload.
    pub fn payload(&self) -> Cow<[u8]> {
        let payload_len = u16::from_be_bytes([self.data[12], self.data[13]]) as usize;
        Cow::from(&self.data[14..(14 + (payload_len as usize))])
    }

    pub fn set_payload(&mut self, payload: &'frame [u8]) {
        let payload_len = payload.len() as u16;
        self.data.truncate(12); //Cut off and drop the entire payload
        let payload_len_bytes = payload_len.to_be_bytes();
        self.data.reserve_exact(payload_len as usize + 2); //Reserve room for payload and len field.
        self.data.extend(payload_len_bytes.iter());
        self.data.extend(payload);
    }

    // This is super uggo but I don't care right now, I'm sure this is a cleaner way to do it.
    pub fn set_dest_mac(&mut self, mac: MacAddr) {
        self.data[0] = mac.b0;
        self.data[1] = mac.b1;
        self.data[2] = mac.b2;
        self.data[3] = mac.b3;
        self.data[4] = mac.b4;
        self.data[5] = mac.b5;
    }

    pub fn set_src_mac(&mut self, mac: MacAddr) {
        self.data[6] = mac.b0;
        self.data[7] = mac.b1;
        self.data[8] = mac.b2;
        self.data[9] = mac.b3;
        self.data[10] = mac.b4;
        self.data[11] = mac.b5;
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