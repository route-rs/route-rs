use crate::types::{Interface, InterfaceAnnotated};
use route_rs_packets::Packet;
use route_rs_runtime::processor::Processor;
use std::marker::PhantomData;

/// InterfaceAnnotationEncap: Processor to apply interface annotation to a packet
///
/// Generally, it is important for the router to maintain some kind of state as to which interface
/// the packet arrived from, and which interface it is destined for.
pub(crate) struct InterfaceAnnotationEncap<P: Send + Clone + Packet> {
    inbound_interface: Interface,
    outbound_interface: Interface,
    phantom: PhantomData<P>,
}

impl<P: Send + Clone + Packet> InterfaceAnnotationEncap<P> {
    pub(crate) fn new(in_tag: Interface, out_tag: Interface) -> Self {
        InterfaceAnnotationEncap {
            inbound_interface: in_tag,
            outbound_interface: out_tag,
            phantom: PhantomData,
        }
    }
}

impl<P: Send + Clone + Packet> Processor for InterfaceAnnotationEncap<P> {
    type Input = P;
    type Output = InterfaceAnnotated<P>;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        Some(InterfaceAnnotated {
            packet,
            inbound_interface: self.inbound_interface,
            outbound_interface: self.outbound_interface,
        })
    }
}
