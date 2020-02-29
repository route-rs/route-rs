use crate::types::{EtherType, InterfaceAnnotated};
use route_rs_packets::EthernetFrame;
use route_rs_runtime::classifier::Classifier;

pub(crate) struct ByEtherType {}

impl Classifier for ByEtherType {
    type Packet = InterfaceAnnotated<EthernetFrame>;
    type Class = EtherType;

    fn classify(&self, packet: &Self::Packet) -> Self::Class {
        let ether_type = packet.packet.ether_type();
        match ether_type {
            0x0800 => EtherType::IPv4,
            0x0806 => EtherType::ARP,
            0x86DD => EtherType::IPv6,
            _ => EtherType::Unsupported,
        }
    }
}
