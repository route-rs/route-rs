use crate::arp::{
    ipv4_array, ipv6_array, mac_array, ArpFrame, ArpHardwareType, ArpOp, ArpTable,
    IPV4_PROTOCOL_TYPE, IPV6_PROTOCOL_TYPE,
};
use crate::types::InterfaceAnnotated;
use route_rs_packets::{EthernetFrame, MacAddr};
use route_rs_runtime::processor::Processor;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

// TODO: We need to respond to ARP requests that are targeted at the router.
// Where is this state stored? How to we access it?
const ROUTER_IPV4_ADDR: Ipv4Addr = Ipv4Addr::LOCALHOST;
const ROUTER_IPV6_ADDR: Ipv6Addr = Ipv6Addr::LOCALHOST;

pub(crate) struct ArpHandler {
    arp_table: ArpTable,
}

impl ArpHandler {
    pub fn new() -> Self {
        ArpHandler {
            arp_table: ArpTable::new(),
        }
    }
}

impl Processor for ArpHandler {
    type Input = InterfaceAnnotated<EthernetFrame>;
    type Output = InterfaceAnnotated<EthernetFrame>;

    ///
    /// ?Do I have the hardware type in ar$hrd?
    /// Yes: (almost definitely)
    ///     [optionally check the hardware length ar$hln]
    ///     ?Do I speak the protocol in ar$pro?
    ///     Yes:
    ///         [optionally check the protocol length ar$pln]
    ///         Merge_flag := false
    ///         If the pair <protocol type, sender protocol address> is already in my translation
    ///             table, update the sender hardware address field of the entry with the new
    ///             information in the packet and set Merge_flag to true.
    ///         ?Am I the target protocol address?
    ///         Yes:
    ///             If Merge_flag is false, add the triplet <protocol type, sender protocol address,
    ///                 sender hardware address> to the translation table.
    ///             ?Is the opcode ares_op$REQUEST?  (NOW look at the opcode!!)
    ///             Yes:
    ///                 Swap hardware and protocol fields, putting the local hardware and protocol
    ///                     addresses in the sender fields.
    ///                 Set the ar$op field to ares_op$REPLY
    ///                 Send the packet to the (new) target hardware address on the same hardware on
    ///                     which the request was received.
    ///
    // TODO: I'd like a more elegant way to deal with different network layer protocols.
    // What's the appropriate level of abstraction?
    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        let arp_frame = ArpFrame::new(packet.packet.clone());

        if arp_frame.hardware_type() == ArpHardwareType::Ethernet as u16 {
            let mut updated_translation_table = false;

            let protocol_type = arp_frame.protocol_type();
            let sender_protocol_address = arp_frame.sender_protocol_addr();

            // TODO maintain mapping of popular EtherTypes to their numbers in route-rs-packets
            // https://www.iana.org/assignments/ieee-802-numbers/ieee-802-numbers.xhtml
            let has_sender_protocol_addr = self
                .arp_table
                .contains_key(protocol_type, sender_protocol_address);

            if has_sender_protocol_addr {
                let sender_mac_address = MacAddr::new(mac_array(arp_frame.sender_hardware_addr()));
                self.arp_table
                    .insert(protocol_type, sender_protocol_address, sender_mac_address);
                updated_translation_table = true;
            }

            let target_protocol_address = arp_frame.target_protocol_addr();
            let target_ipv4_address = Ipv4Addr::from(ipv4_array(target_protocol_address));
            let target_ipv6_address = Ipv6Addr::from(ipv6_array(target_protocol_address));

            if (protocol_type == IPV4_PROTOCOL_TYPE && target_ipv4_address == ROUTER_IPV4_ADDR)
                || (protocol_type == IPV6_PROTOCOL_TYPE && target_ipv6_address == ROUTER_IPV6_ADDR)
            {
                let sender_mac_address = MacAddr::new(mac_array(arp_frame.sender_hardware_addr()));
                if !updated_translation_table {
                    self.arp_table.insert(
                        protocol_type,
                        sender_protocol_address,
                        sender_mac_address,
                    );
                }

                if arp_frame.opcode() == ArpOp::Request as u8 {
                    let mut response_arp_frame = arp_frame.clone();
                    response_arp_frame.set_target_hardware_addr(sender_mac_address);

                    if protocol_type == 0x0800 {
                        response_arp_frame.set_target_protocol_addr(IpAddr::from(ipv4_array(
                            sender_protocol_address,
                        )));
                    } else if protocol_type == 0x86DD {
                        response_arp_frame.set_target_protocol_addr(IpAddr::from(ipv6_array(
                            sender_protocol_address,
                        )));
                    } else {
                        panic!("unsupported network protocol")
                    }

                    response_arp_frame.set_opcode(ArpOp::Reply as u8);

                    return Some(InterfaceAnnotated::<EthernetFrame> {
                        packet: response_arp_frame.frame(),
                        inbound_interface: packet.inbound_interface,
                        outbound_interface: packet.outbound_interface,
                    });
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    fn arp_test_runs() {
        println!("Hello!")
    }
}
