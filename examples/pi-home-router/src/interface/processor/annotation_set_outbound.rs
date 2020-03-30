use crate::types::{Interface, InterfaceAnnotated};
use route_rs_packets::Packet;
use route_rs_runtime::processor::Processor;
use std::marker::PhantomData;

/// InterfaceAnnotationSetOutbound: Update the outbound interface for a packet
///
/// When we make a decision about routing, this processor allows us to set the outbound interface.
pub(crate) struct InterfaceAnnotationSetOutbound<P: Send + Clone + Packet> {
    outbound_interface: Interface,
    phantom: PhantomData<P>,
}

impl<P: Send + Clone + Packet> InterfaceAnnotationSetOutbound<P> {
    pub(crate) fn new(out_tag: Interface) -> Self {
        InterfaceAnnotationSetOutbound {
            outbound_interface: out_tag,
            phantom: PhantomData,
        }
    }
}

impl<P: Send + Clone + Packet> Processor for InterfaceAnnotationSetOutbound<P> {
    type Input = InterfaceAnnotated<P>;
    type Output = InterfaceAnnotated<P>;

    fn process(&mut self, annotated_packet: Self::Input) -> Option<Self::Output> {
        Some(InterfaceAnnotated {
            packet: annotated_packet.packet,
            inbound_interface: annotated_packet.inbound_interface,
            outbound_interface: self.outbound_interface,
        })
    }
}
