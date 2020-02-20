use route_rs_runtime::processor::Processor;
use route_rs_packets::EthernetFrame;
use crate::types::{InterfaceAnnotated, Interface};

/// InterfaceAnnotationEncap: Processor to apply interface annotation to a packet
///
/// Generally, it is important for the router to maintain some kind of state as to which interface
/// the packet arrived from, and which interface it is destined for.
pub(crate) struct InterfaceAnnotationEncap {
    inbound_interface: Interface,
    outbound_interface: Interface,
}

impl InterfaceAnnotationEncap {
    pub(crate) fn new(in_tag: Interface, out_tag: Interface) -> Self {
        InterfaceAnnotationEncap {
            inbound_interface: in_tag,
            outbound_interface: out_tag,
        }
    }
}

impl Processor for InterfaceAnnotationEncap {
    type Input = EthernetFrame;
    type Output = InterfaceAnnotated<EthernetFrame>;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        Some(InterfaceAnnotated {
            packet: packet,
            inbound_interface: self.inbound_interface,
            outbound_interface: self.outbound_interface,
        })
    }
}

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
