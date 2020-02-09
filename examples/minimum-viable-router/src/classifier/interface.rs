use crate::processor::{Interface, InterfaceTaggedPacket};
use route_rs_runtime::classifier::Classifier;
use std::marker::PhantomData;

pub struct ClassifyByDestinationInterface<DataPacket: Send + Clone> {
    phantom: PhantomData<DataPacket>,
}

impl<DataPacket: Send + Clone> ClassifyByDestinationInterface<DataPacket> {
    pub fn new() -> Self {
        ClassifyByDestinationInterface {
            phantom: PhantomData,
        }
    }
}

impl<DataPacket: Send + Clone> Classifier for ClassifyByDestinationInterface<DataPacket> {
    type Packet = InterfaceTaggedPacket<DataPacket>;
    type Class = Option<Interface>;

    fn classify(&self, packet: &Self::Packet) -> Self::Class {
        packet.dst_interface.clone()
    }
}
