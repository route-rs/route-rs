use crate::types::InterfaceAnnotated;
use route_rs_packets::EthernetFrame;
use route_rs_runtime::processor::Processor;

/// Removes Interface annotations from a packet
#[derive(Default)]
pub(crate) struct InterfaceAnnotationDecap;

impl Processor for InterfaceAnnotationDecap {
    type Input = InterfaceAnnotated<EthernetFrame>;
    type Output = EthernetFrame;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        Some(packet.packet)
    }
}
