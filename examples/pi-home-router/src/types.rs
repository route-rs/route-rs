/// File to contain various types used in the router
use route_rs_packets::Packet;

/// InterfaceAnnotated:
///
/// A type to wrap a packet, and annotate which inbound and outbound interfaces
/// the packet originated from, and it marked to be router to.
#[derive(Clone, Debug)]
pub(crate) struct InterfaceAnnotated<P: Packet> {
    pub(crate) packet: P,
    pub(crate) inbound_interface: Interface,
    pub(crate) outbound_interface: Interface,
}

/// Interface:
///
/// An enum to label the inbound and outbound interfaces with, None is used to
/// denote an unknown or yet-to-be determined interface.
#[derive(Copy, Debug, Clone)]
pub enum Interface {
    Host,
    Wan,
    Lan,
    None,
}
