use crate::processor::InterfaceTaggedPacket;
use route_rs_packets::EthernetFrame;
use route_rs_runtime::classifier::Classifier;

#[allow(dead_code)]
pub enum Protocol {
    ARP,
    NDP,
    DHCP,
    IPv4,
    IPv6,
    Unknown,
}

pub struct ClassifyByProtocol {}

impl ClassifyByProtocol {
    pub fn new() -> Self {
        ClassifyByProtocol {}
    }
}

impl Classifier for ClassifyByProtocol {
    type Packet = InterfaceTaggedPacket<EthernetFrame>;
    type Class = Protocol;

    fn classify(&self, _packet: &Self::Packet) -> Self::Class {
        Protocol::Unknown
    }
}
