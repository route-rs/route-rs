use crate::{EthernetFrame, MacAddr, ARP_ETHER_TYPE};
use std::convert::{TryFrom, TryInto};
use std::net::IpAddr;

pub enum ArpOp {
    Request = 1,
    Reply = 2,
}

pub enum ArpHardwareType {
    Ethernet = 1,
}

const HARDWARE_TYPE_RANGE: (usize, usize) = (0, 2);
const PROTOCOL_TYPE_RANGE: (usize, usize) = (2, 4);
const HARDWARE_ADDR_LEN_RANGE: (usize, usize) = (4, 5);
const PROTOCOL_ADDR_LEN_RANGE: (usize, usize) = (5, 6);
const OPCODE_RANGE: (usize, usize) = (6, 8);

///
/// EthernetFrame wrapper with getters/setters for the packet structure described in RFC 826
/// https://tools.ietf.org/html/rfc826
///
#[derive(Clone)]
pub struct ArpFrame {
    frame: EthernetFrame,
}

impl ArpFrame {
    ///
    /// Constructs a new, empty packet with a payload big enough for all ARP fields,
    /// given some hardware/protocol address lengths.
    ///
    pub fn new(hardware_addr_len: u8, protocol_addr_len: u8) -> Self {
        let payload_len = 8 + (2 * hardware_addr_len as usize) + (2 * protocol_addr_len as usize);
        let payload: Vec<u8> = vec![0; payload_len];

        let mut frame = EthernetFrame::empty();
        frame.set_payload(payload.as_slice());

        let mut arp_frame = ArpFrame { frame };
        arp_frame.set_hardware_addr_len(hardware_addr_len);
        arp_frame.set_protocol_addr_len(protocol_addr_len);
        arp_frame
    }

    pub fn hardware_type(&self) -> u16 {
        let (start, end) = HARDWARE_TYPE_RANGE;
        u16::from_be_bytes(self.arp_data(start, end).try_into().unwrap())
    }

    pub fn protocol_type(&self) -> u16 {
        let (start, end) = PROTOCOL_TYPE_RANGE;
        u16::from_be_bytes(self.arp_data(start, end).try_into().unwrap())
    }

    pub fn hardware_addr_len(&self) -> u8 {
        let (start, end) = HARDWARE_ADDR_LEN_RANGE;
        u8::from_be_bytes(self.arp_data(start, end).try_into().unwrap())
    }

    pub fn protocol_addr_len(&self) -> u8 {
        let (start, end) = PROTOCOL_ADDR_LEN_RANGE;
        u8::from_be_bytes(self.arp_data(start, end).try_into().unwrap())
    }

    pub fn opcode(&self) -> u16 {
        let (start, end) = OPCODE_RANGE;
        u16::from_be_bytes(self.arp_data(start, end).try_into().unwrap())
    }

    pub fn sender_hardware_addr(&self) -> &[u8] {
        let (start, end) = self.sender_hardware_addr_range();
        self.arp_data(start, end)
    }

    pub fn sender_protocol_addr(&self) -> &[u8] {
        let (start, end) = self.sender_protocol_addr_range();
        self.arp_data(start, end)
    }

    pub fn target_hardware_addr(&self) -> &[u8] {
        let (start, end) = self.target_hardware_addr_range();
        self.arp_data(start, end)
    }

    pub fn target_protocol_addr(&self) -> &[u8] {
        let (start, end) = self.target_protocol_addr_range();
        self.arp_data(start, end)
    }

    pub fn set_hardware_type(&mut self, htype: u16) {
        let (start, end) = HARDWARE_TYPE_RANGE;
        self.set_arp_data(&htype.to_be_bytes(), start, end);
    }

    pub fn set_protocol_type(&mut self, ptype: u16) {
        let (start, end) = PROTOCOL_TYPE_RANGE;
        self.set_arp_data(&ptype.to_be_bytes(), start, end);
    }

    pub fn set_hardware_addr_len(&mut self, len: u8) {
        let (start, end) = HARDWARE_ADDR_LEN_RANGE;
        self.set_arp_data(&len.to_be_bytes(), start, end);
    }

    pub fn set_protocol_addr_len(&mut self, len: u8) {
        let (start, end) = PROTOCOL_ADDR_LEN_RANGE;
        self.set_arp_data(&len.to_be_bytes(), start, end);
    }

    pub fn set_opcode(&mut self, code: u16) {
        let (start, end) = OPCODE_RANGE;
        self.set_arp_data(&code.to_be_bytes(), start, end);
    }

    pub fn set_sender_hardware_addr(&mut self, addr: MacAddr) {
        // NOTE: should we set len based on frame data, or param?
        let (start, end) = self.sender_hardware_addr_range();
        self.set_arp_data(&addr.bytes, start, end);
    }

    pub fn set_sender_protocol_addr(&mut self, ip_addr: IpAddr) {
        // NOTE: should we set len based on frame data, or param?
        let (start, _) = self.sender_protocol_addr_range();
        self.set_ip_addr(ip_addr, start);
    }

    pub fn set_target_hardware_addr(&mut self, addr: MacAddr) {
        // NOTE: should we set len based on frame data, or param?
        let (start, end) = self.target_hardware_addr_range();
        self.set_arp_data(&addr.bytes, start, end);
    }

    pub fn set_target_protocol_addr(&mut self, ip_addr: IpAddr) {
        // NOTE: should we set len based on frame data, or param?
        let (start, _) = self.target_protocol_addr_range();
        self.set_ip_addr(ip_addr, start);
    }

    // Move ownership of the frame back to the caller
    pub fn frame(self) -> EthernetFrame {
        self.frame
    }

    // Returns the bytes in the ethernet frame between start and end, exclusive
    fn arp_data(&self, start: usize, end: usize) -> &[u8] {
        let frame_offset_start = self.frame.payload_offset + start;
        let frame_offset_end = self.frame.payload_offset + end;

        // TODO: I'd like to use `self.frame.payload()` here, but having ownership difficulties with Cow
        &self.frame.data[frame_offset_start..frame_offset_end]
    }

    fn set_arp_data(&mut self, bytes: &[u8], start: usize, end: usize) {
        let frame_offset_start = self.frame.payload_offset + start;
        let frame_offset_end = self.frame.payload_offset + end;

        // TODO: I'd like to mutate`self.frame.payload()` here, but having ownership difficulties with Cow
        self.frame.data[frame_offset_start..frame_offset_end].copy_from_slice(bytes);
    }

    fn set_ip_addr(&mut self, addr: IpAddr, start: usize) {
        match addr {
            IpAddr::V4(ipv4) => self.set_arp_data(&ipv4.octets(), start, start + 4),
            IpAddr::V6(ipv6) => self.set_arp_data(&ipv6.octets(), start, start + 16),
        };
    }

    fn sender_hardware_addr_range(&self) -> (usize, usize) {
        let hlen = self.hardware_addr_len() as usize;

        let start = 8;
        let end = start + hlen;
        (start, end)
    }
    fn sender_protocol_addr_range(&self) -> (usize, usize) {
        let hlen = self.hardware_addr_len() as usize;
        let plen = self.protocol_addr_len() as usize;

        let start = 8 + hlen;
        let end = start + plen;
        (start, end)
    }
    fn target_hardware_addr_range(&self) -> (usize, usize) {
        let hlen = self.hardware_addr_len() as usize;
        let plen = self.protocol_addr_len() as usize;

        let start = 8 + hlen + plen;
        let end = start + hlen;
        (start, end)
    }
    fn target_protocol_addr_range(&self) -> (usize, usize) {
        let hlen = self.hardware_addr_len() as usize;
        let plen = self.protocol_addr_len() as usize;

        let start = 8 + (2 * hlen) + plen;
        let end = start + plen;
        (start, end)
    }
}

impl TryFrom<EthernetFrame> for ArpFrame {
    type Error = &'static str;

    ///
    /// Decorates the given EthernetFrame with ArpFrame getters/setters.
    /// Validates
    /// - The frame has an ARP ether type
    /// - The frame has a reasonable payload size given the hardware/protocol address lengths
    ///
    fn try_from(frame: EthernetFrame) -> Result<Self, Self::Error> {
        if frame.ether_type() != ARP_ETHER_TYPE {
            return Err("Frame does not have ARP ether type.");
        };

        let arp_frame = ArpFrame { frame };
        let payload_len = arp_frame.frame.payload().len();

        if payload_len < 8 {
            return Err("Frame payload is too small");
        }

        let hlen = arp_frame.hardware_addr_len() as usize;
        let plen = arp_frame.protocol_addr_len() as usize;

        if payload_len != (8 + (2 * hlen) + (2 * plen)) {
            return Err("Frame payload doesn't match address length fields");
        }

        Ok(arp_frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_empty_arp_frame() {
        let arp_frame = ArpFrame::new(6, 4);
        assert_eq!(arp_frame.hardware_type(), 0);
        assert_eq!(arp_frame.protocol_type(), 0);
        assert_eq!(arp_frame.hardware_addr_len(), 6);
        assert_eq!(arp_frame.protocol_addr_len(), 4);
        assert_eq!(arp_frame.opcode(), 0);
        assert_eq!(arp_frame.sender_hardware_addr(), [0, 0, 0, 0, 0, 0]);
        assert_eq!(arp_frame.sender_protocol_addr(), [0, 0, 0, 0]);
        assert_eq!(arp_frame.target_hardware_addr(), [0, 0, 0, 0, 0, 0]);
        assert_eq!(arp_frame.target_protocol_addr(), [0, 0, 0, 0]);
    }

    #[test]
    fn arp_frame_from_ethernet() -> Result<(), String> {
        let arp_payload: Vec<u8> = vec![
            0x00, 0x01, 0x00, 0x01, 0x06, 0x04, 0x00, 0x01, 1, 2, 3, 4, 5, 6, 10, 0, 0, 1, 10, 9,
            8, 7, 6, 5, 0xff, 0xff, 0xff, 0xff,
        ];
        let mut ethernet_frame = EthernetFrame::empty();
        ethernet_frame.set_payload(&arp_payload);
        ethernet_frame.set_ether_type(ARP_ETHER_TYPE);

        let arp_frame = ArpFrame::try_from(ethernet_frame)?;
        assert_eq!(arp_frame.hardware_type(), 1);
        assert_eq!(arp_frame.protocol_type(), 1);
        assert_eq!(arp_frame.hardware_addr_len(), 6);
        assert_eq!(arp_frame.protocol_addr_len(), 4);
        assert_eq!(arp_frame.opcode(), ArpOp::Request as u16);
        assert_eq!(arp_frame.sender_hardware_addr(), [1, 2, 3, 4, 5, 6]);
        assert_eq!(arp_frame.sender_protocol_addr(), [10, 0, 0, 1]);
        assert_eq!(arp_frame.target_hardware_addr(), [10, 9, 8, 7, 6, 5]);
        assert_eq!(arp_frame.target_protocol_addr(), [0xff, 0xff, 0xff, 0xff]);
        Ok(())
    }
}
