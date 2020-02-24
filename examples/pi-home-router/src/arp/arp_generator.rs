use crate::types::InterfaceAnnotated;
use route_rs_packets::{
    ArpFrame, ArpOp, EthernetFrame, Ipv4Packet, MacAddr, ARP_ETHER_TYPE, IPV4_ETHER_TYPE,
};
use route_rs_runtime::processor::Processor;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};

// TODO: This could also be called AddMacOrGenArp
pub(crate) struct ArpGenerator {
    arp_table: Arc<Mutex<HashMap<Ipv4Addr, MacAddr>>>,
}

impl ArpGenerator {
    pub fn new(arp_table: Arc<Mutex<HashMap<Ipv4Addr, MacAddr>>>) -> Self {
        ArpGenerator { arp_table }
    }
}

impl Processor for ArpGenerator {
    type Input = InterfaceAnnotated<EthernetFrame>;
    type Output = InterfaceAnnotated<EthernetFrame>;

    ///
    /// From the ARP RFC: https://tools.ietf.org/html/rfc826
    ///
    /// As a packet is sent down through the network layers, routing determines the protocol address
    /// of the next hop for the packet and on which piece of hardware it expects to find the station
    /// with the immediate target protocol address. The Address Resolution module converts the
    /// <protocol type, target protocol address> pair to a 48.bit Ethernet address. The Address
    /// Resolution module tries to find this pair in a table. If it finds the pair, it gives the
    /// corresponding 48.bit Ethernet address back to the caller which then transmits the packet.
    ///
    /// If the lookup for the (protocol type, protocol address) fails, it probably informs the
    /// caller that it is throwing the packet away (on the assumption the packet will be
    /// retransmitted by a higher network layer), and generates an Ethernet packet with a type field
    /// of ether_type$ADDRESS_RESOLUTION. The Address Resolution module then sets
    ///
    /// Hardware Type: ares_hrd$Ethernet = 1
    /// Protocol Type: type that is being resolved
    /// Hardware Address Length: 6 (the number of bytes in a 48.bit Ethernet address)
    /// Protocol Address Length: length of an address in that protocol
    /// Op: ares_op$REQUEST = 1
    /// Sender Hardware Address: the 48.bit ethernet address of itself
    /// Sender Protocol Address: the protocol address of itself
    /// Target Protocol Address: the protocol address of the machine that is trying to be accessed.
    ///
    /// It does not set Target Hardware Address to anything in particular, because it is this value
    /// that it is trying to determine. It could set the Target Hardware Address to the broadcast
    /// address (FF:FF:FF:FF:FF:FF) for the hardware (all ones in the case of the 10Mbit Ethernet)
    /// if that makes it convenient for some aspect of the implementation.
    ///
    /// It then causes this packet to be broadcast to all stations on the Ethernet cable originally
    /// determined by the routing mechanism.
    ///
    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        let mut frame = packet.packet;
        match frame.ether_type() {
            IPV4_ETHER_TYPE => {
                let ipv4_packet = Ipv4Packet::try_from(frame).unwrap();

                let ipv4_src_addr = ipv4_packet.src_addr();
                let ipv4_dest_addr = ipv4_packet.dest_addr();
                frame = EthernetFrame::encap_ipv4(ipv4_packet);

                match self.arp_table.lock().unwrap().get(&ipv4_dest_addr) {
                    Some(&dest_mac_addr) => {
                        // We found a MacAddr for this (protocol type, target address) tuple!
                        // Let's set it, and pass the packet along
                        frame.set_dest_mac(dest_mac_addr);
                        Some(InterfaceAnnotated::<EthernetFrame> {
                            packet: frame,
                            inbound_interface: packet.inbound_interface,
                            outbound_interface: packet.outbound_interface,
                        })
                    }
                    None => {
                        // Couldn't find a match in the table, let's generate an ARP request
                        let mut arp_request = ArpFrame::default();
                        arp_request.set_hardware_type(1);
                        arp_request.set_protocol_type(IPV4_ETHER_TYPE);
                        arp_request.set_hardware_addr_len(6);
                        arp_request.set_protocol_addr_len(4);
                        arp_request.set_opcode(ArpOp::Request as u16);
                        arp_request.set_sender_hardware_addr(frame.src_mac());
                        arp_request.set_sender_protocol_addr(IpAddr::V4(ipv4_src_addr));
                        arp_request.set_target_hardware_addr(MacAddr::new([0, 0, 0, 0, 0, 0]));
                        arp_request.set_target_protocol_addr(IpAddr::V4(ipv4_dest_addr));
                        frame = arp_request.frame();
                        frame.set_ether_type(ARP_ETHER_TYPE);
                        Some(InterfaceAnnotated::<EthernetFrame> {
                            packet: frame,
                            inbound_interface: packet.inbound_interface,
                            outbound_interface: packet.outbound_interface,
                        })
                    }
                }
            }
            // Ignore non-IPv4 packets
            // TODO: add error logging?
            _ => Some(InterfaceAnnotated::<EthernetFrame> {
                packet: frame,
                inbound_interface: packet.inbound_interface,
                outbound_interface: packet.outbound_interface,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Interface;
    use route_rs_packets::ARP_ETHER_TYPE;
    use std::net::Ipv4Addr;

    fn outgoing_ipv4(dest_ipv4: Ipv4Addr) -> EthernetFrame {
        let mut packet = Ipv4Packet::empty();
        packet.set_dest_addr(dest_ipv4);

        let mut frame = EthernetFrame::encap_ipv4(packet);
        frame.set_ether_type(IPV4_ETHER_TYPE);
        frame
    }

    #[test]
    fn found_translation_adds_mac() {
        // Initialize IP to MAC translation table with one entry
        let arp_table: Arc<Mutex<HashMap<Ipv4Addr, MacAddr>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let dest_ipv4 = Ipv4Addr::new(1, 2, 3, 4);
        let dest_mac = MacAddr::new([1, 2, 3, 4, 5, 6]);
        arp_table.lock().unwrap().insert(dest_ipv4, dest_mac);

        let output = ArpGenerator::new(arp_table)
            .process(InterfaceAnnotated::<EthernetFrame> {
                packet: outgoing_ipv4(dest_ipv4),
                inbound_interface: Interface::Wan,
                outbound_interface: Interface::Lan,
            })
            .unwrap();

        assert_eq!(output.packet.ether_type(), IPV4_ETHER_TYPE);
        assert_eq!(output.packet.dest_mac(), dest_mac);
        assert_eq!(
            Ipv4Packet::try_from(output.packet).unwrap().dest_addr(),
            dest_ipv4
        );
        assert_eq!(output.inbound_interface, Interface::Wan);
        assert_eq!(output.outbound_interface, Interface::Lan);
    }

    #[test]
    fn missing_translation_generates_arp_request() {
        // Initialize empty IP to MAC translation table
        let arp_table: Arc<Mutex<HashMap<Ipv4Addr, MacAddr>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let dest_ipv4 = Ipv4Addr::new(1, 2, 3, 4);

        let output = ArpGenerator::new(arp_table)
            .process(InterfaceAnnotated::<EthernetFrame> {
                packet: outgoing_ipv4(dest_ipv4),
                inbound_interface: Interface::Wan,
                outbound_interface: Interface::Lan,
            })
            .unwrap();

        assert_eq!(output.packet.ether_type(), ARP_ETHER_TYPE);
        assert_eq!(output.packet.dest_mac(), MacAddr::new([0, 0, 0, 0, 0, 0]));
        assert_eq!(
            Ipv4Packet::try_from(output.packet.clone()).unwrap_err(),
            "Packet has incorrect version, is not Ipv4Packet"
        );
        let arp_request = ArpFrame::try_from(output.packet.clone()).unwrap();
        assert_eq!(arp_request.protocol_type(), IPV4_ETHER_TYPE);
        assert_eq!(arp_request.target_ipv4_addr().unwrap(), dest_ipv4);
        assert_eq!(output.inbound_interface, Interface::Wan);
        assert_eq!(output.outbound_interface, Interface::Lan);
    }
}
