use crate::types::{Interface, InterfaceAnnotated};
use route_rs_packets::EthernetFrame;
use route_rs_runtime::classifier::Classifier;

#[derive(Default)]
pub(crate) struct ByOutboundInterface;

impl Classifier for ByOutboundInterface {
    type Packet = InterfaceAnnotated<EthernetFrame>;
    type Class = Interface;

    fn classify(&self, packet: &Self::Packet) -> Self::Class {
        packet.outbound_interface
    }
}
