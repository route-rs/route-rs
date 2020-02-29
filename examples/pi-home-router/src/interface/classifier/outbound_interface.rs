use crate::types::{Interface, InterfaceAnnotated};
use route_rs_packets::EthernetFrame;
use route_rs_runtime::classifier::Classifier;

#[derive(Default)]
pub(crate) struct ByOutboundInterface;

impl Classifier for ByOutboundInterface {
    type Packet = InterfaceAnnotated<EthernetFrame>;
    type Class = Interface;

    fn classify(&self, packet: &Self::Packet) -> Self::Class {
        match packet.outbound_interface {
            Interface::Host => Interface::Host,
            Interface::Lan => Interface::Lan,
            Interface::Wan => Interface::Wan,
            Interface::Unmarked => Interface::Unmarked,
        }
    }
}
