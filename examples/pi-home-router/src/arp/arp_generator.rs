use crate::arp::ArpOp::Request;
use crate::arp::{ArpFrame, ArpTable, IPV4_PROTOCOL_TYPE};
use crate::types::InterfaceAnnotated;
use route_rs_packets::{EthernetFrame, Ipv4Packet, Ipv6Packet, MacAddr};
use route_rs_runtime::processor::Processor;
use std::convert::TryFrom;
use std::net::IpAddr;

pub(crate) struct ArpGenerator {
    // Mapping between (protocol type, protocol address) -> 48-bit Ethernet address
    arp_table: ArpTable,
}

impl ArpGenerator {
    pub fn new() -> Self {
        ArpGenerator {
            arp_table: ArpTable::new(),
        }
    }
}

impl Processor for ArpGenerator {
    type Input = InterfaceAnnotated<EthernetFrame>;
    type Output = InterfaceAnnotated<EthernetFrame>;

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
        // It would be nice if there was a function to determine which network protocol type
        // (IPv4/IPv6/other) an EthernetFrame wraps over.

        // TODO: Use Cows for less copying
        let frame = packet.packet.clone();
        if let Ok(ipv4_packet) = Ipv4Packet::try_from(frame.clone()) {
            let target_addr_bytes = ipv4_packet.dest_addr().octets();
            match self.arp_table.get(IPV4_PROTOCOL_TYPE, &target_addr_bytes) {
                Some(dest_mac_addr) => {
                    // We found a MacAddr for this (protocol type, target address) tuple!
                    // Let's set it, and pass the packet along
                    let mut packet_copy = frame;
                    packet_copy.set_dest_mac(*dest_mac_addr);
                    Some(InterfaceAnnotated::<EthernetFrame> {
                        packet: packet_copy,
                        inbound_interface: packet.inbound_interface,
                        outbound_interface: packet.outbound_interface,
                    })
                }
                None => {
                    // Couldn't find a match in the table, let's generate an ARP request
                    let mut arp_request = ArpFrame::new(EthernetFrame::empty());
                    arp_request.set_hardware_type(1);
                    arp_request.set_protocol_type(IPV4_PROTOCOL_TYPE);
                    arp_request.set_hardware_addr_len(6);
                    arp_request.set_protocol_addr_len(4);
                    arp_request.set_opcode(Request as u8);
                    arp_request.set_sender_hardware_addr(frame.src_mac());
                    arp_request.set_sender_protocol_addr(IpAddr::V4(ipv4_packet.src_addr()));
                    let broadcast_addr: [u8; 6] = [!0, !0, !0, !0, !0, !0]; // TODO: nah this aint right
                    arp_request.set_target_hardware_addr(MacAddr::new(broadcast_addr));
                    arp_request.set_target_protocol_addr(IpAddr::V4(ipv4_packet.dest_addr()));
                    Some(InterfaceAnnotated::<EthernetFrame> {
                        packet: arp_request.frame(),
                        inbound_interface: packet.inbound_interface,
                        outbound_interface: packet.outbound_interface,
                    })
                }
            }
        } else if let Ok(ipv6_packet) = Ipv6Packet::try_from(frame.clone()) {
            // TODO: repeat for v6
            None
        } else {
            // TODO: is this the right error strategy?
            Some(packet)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arp_test_runs() {
        println!("Hello!")
    }
}
