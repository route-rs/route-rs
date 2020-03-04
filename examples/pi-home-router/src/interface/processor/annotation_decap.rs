use crate::types::InterfaceAnnotated;
use route_rs_packets::Packet;
use route_rs_runtime::processor::Processor;
use std::marker::PhantomData;

/// Removes Interface annotations from a packet
pub(crate) struct InterfaceAnnotationDecap<P: Send + Clone + Packet> {
    phantom: PhantomData<P>,
}

impl<P: Send + Clone + Packet> InterfaceAnnotationDecap<P> {
    pub(crate) fn new() -> Self {
        InterfaceAnnotationDecap {
            phantom: PhantomData,
        }
    }
}

impl<P: Send + Clone + Packet> Processor for InterfaceAnnotationDecap<P> {
    type Input = InterfaceAnnotated<P>;
    type Output = P;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        Some(packet.packet)
    }
}
