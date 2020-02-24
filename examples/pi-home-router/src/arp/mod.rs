/// ARP Research Questions & Notes
/// https://tools.ietf.org/html/rfc826
///
/// Does this router's ARP resolution have to respond to broadcasts?
/// If someone on the LAN asks what for the MAC addr of a certain IP, that machine should respond.
/// Even if someone is asking for the address of the RaspPi, should we just forward that packet to the host?
/// Or are we implementing ARP _for_ the host?
///
/// There are 2 ARP processors: An ArpGenerator and an ArpHandler.
/// ArpHandler: If a translation table lookup fails, we throw the packet away and construct a new
/// ARP request packet to broadcast and determine the unknown target MAC address.
/// ArpReceiver: If we receive an ARP request, we might need to take some action (updating our
/// translation table) and sending an ARP response back to the sender.
///
///
mod arp_frame;
pub(crate) use self::arp_frame::*;

mod arp_generator;
pub(crate) use self::arp_generator::ArpGenerator;

mod arp_handler;
pub(crate) use self::arp_handler::ArpHandler;
