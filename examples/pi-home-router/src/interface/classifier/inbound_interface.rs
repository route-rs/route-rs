use crate::types::{Interface, InterfaceAnnotated};
use route_rs_packets::Packet;
use route_rs_runtime::classifier::Classifier;
use std::marker::PhantomData;

pub(crate) struct ByInboundInterface<P: Send + Clone + Packet> {
    phantom: PhantomData<P>,
}

impl<P: Send + Clone + Packet> ByInboundInterface<P> {
    pub(crate) fn new() -> Self {
        ByInboundInterface {
            phantom: PhantomData,
        }
    }
}

impl<P: Send + Clone + Packet> Classifier for ByInboundInterface<P> {
    type Packet = InterfaceAnnotated<P>;
    type Class = Interface;

    fn classify(&self, packet: &Self::Packet) -> Self::Class {
        packet.inbound_interface
    }
}
