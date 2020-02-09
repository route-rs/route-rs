use route_rs_runtime::processor::Processor;
use std::marker::PhantomData;

#[derive(Clone)]
pub enum Interface {
    /// The Wide Area Network above the router
    WAN,
    /// The Local Area Network below the router
    LAN,
    /// The Linux host system in which the router is running
    Host,
}

#[derive(Clone)]
pub struct InterfaceTaggedPacket<Packet: Send + Clone> {
    /// The interface tag for where the packet came from. None if we don't know yet.
    src_interface: Option<Interface>,
    /// The interface tag for where the packet is going. None if we don't know yet.
    dst_interface: Option<Interface>,
    /// The actual data packet.
    packet: Packet,
}

/// When receiving a packet from an interface, we want to tag it with that interface in order to
/// keep track of where it came from for later routing decisions. In practical use we probably won't
/// want to set up the destination interface at the beginning, but maybe we will so we can leave
/// that in here too since we have to provision the field anyway.
pub struct EncapInterfaceTags<Packet: Send + Clone> {
    src_interface: Option<Interface>,
    dst_interface: Option<Interface>,
    phantom_packet: PhantomData<Packet>,
}

impl<Packet: Send + Clone> EncapInterfaceTags<Packet> {
    pub fn new(
        src_interface: Option<Interface>,
        dst_interface: Option<Interface>,
    ) -> EncapInterfaceTags<Packet> {
        EncapInterfaceTags {
            src_interface,
            dst_interface,
            phantom_packet: PhantomData,
        }
    }
}

impl<Packet: Send + Clone> Processor for EncapInterfaceTags<Packet> {
    type Input = Packet;
    type Output = InterfaceTaggedPacket<Packet>;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        Some(InterfaceTaggedPacket {
            src_interface: self.src_interface.clone(),
            dst_interface: self.dst_interface.clone(),
            packet,
        })
    }
}

/// When sending a packet back out, we need to unwrap all of the interface tags after we're done
/// making routing decisions, so that we can hand just the packet to the actual interface.
pub struct DecapInterfaceTags<Packet: Send + Clone> {
    phantom_packet: PhantomData<Packet>,
}

impl<Packet: Send + Clone> DecapInterfaceTags<Packet> {
    pub fn new() -> DecapInterfaceTags<Packet> {
        DecapInterfaceTags {
            phantom_packet: PhantomData,
        }
    }
}

impl<Packet: Send + Clone> Processor for DecapInterfaceTags<Packet> {
    type Input = InterfaceTaggedPacket<Packet>;
    type Output = Packet;

    fn process(&mut self, tagged_packet: Self::Input) -> Option<Self::Output> {
        Some(tagged_packet.packet)
    }
}
