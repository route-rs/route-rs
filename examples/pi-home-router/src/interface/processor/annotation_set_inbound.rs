use crate::types::{Interface, InterfaceAnnotated};
use route_rs_packets::Packet;
use route_rs_runtime::processor::Processor;
use std::marker::PhantomData;

/// InterfaceAnnotationSetInbound: Update the inbound interface for a packet
///
/// This is sometimes useful if we had to discard the inbound interface information previously and
/// now need to reconstruct it.
pub(crate) struct InterfaceAnnotationSetInbound<P: Send + Clone + Packet> {
    inbound_interface: Interface,
    phantom: PhantomData<P>,
}

impl<P: Send + Clone + Packet> InterfaceAnnotationSetInbound<P> {
    pub(crate) fn new(in_tag: Interface) -> Self {
        InterfaceAnnotationSetInbound {
            inbound_interface: in_tag,
            phantom: PhantomData,
        }
    }
}

impl<P: Send + Clone + Packet> Processor for InterfaceAnnotationSetInbound<P> {
    type Input = InterfaceAnnotated<P>;
    type Output = InterfaceAnnotated<P>;

    fn process(&mut self, annotated_packet: Self::Input) -> Option<Self::Output> {
        Some(InterfaceAnnotated {
            packet: annotated_packet.packet,
            inbound_interface: self.inbound_interface,
            outbound_interface: annotated_packet.inbound_interface,
        })
    }
}
