use route_rs_runtime::processor::Processor;
use route_rs_packets::EthernetFrame;
use crate::types::InterfaceAnnotated;

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
