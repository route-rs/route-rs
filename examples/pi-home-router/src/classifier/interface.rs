use route_rs_runtime::classifier::Classifier;
use route_rs_packets::EthernetFrame;
use crate::types::{InterfaceAnnotated, Interface};

#[derive(Default)]
pub(crate) struct ClassifyByOutboundInterface;

impl Classifier for ClassifyByOutboundInterface {
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
