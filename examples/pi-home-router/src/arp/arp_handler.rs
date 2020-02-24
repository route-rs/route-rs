use crate::types::InterfaceAnnotated;
use route_rs_packets::{ArpFrame, ArpHardwareType, ArpOp, EthernetFrame, MacAddr, IPV4_ETHER_TYPE};
use route_rs_runtime::processor::Processor;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};

// TODO: We need to respond to ARP requests that are targeted at the router.
// Where is this state stored? How to we access it?
const ROUTER_IPV4_ADDR: Ipv4Addr = Ipv4Addr::LOCALHOST;

pub(crate) struct ArpHandler {
    arp_table: Arc<Mutex<HashMap<Ipv4Addr, MacAddr>>>,
}

impl ArpHandler {
    pub fn new(arp_table: Arc<Mutex<HashMap<Ipv4Addr, MacAddr>>>) -> Self {
        ArpHandler { arp_table }
    }
}

impl Processor for ArpHandler {
    type Input = InterfaceAnnotated<EthernetFrame>;
    type Output = InterfaceAnnotated<EthernetFrame>;

    ///
    /// From the ARP RFC: https://tools.ietf.org/html/rfc826
    /// Handler assumes it receives ARP packets
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
    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        let mut arp_frame = ArpFrame::try_from(packet.packet).unwrap();
        if arp_frame.hardware_type() == ArpHardwareType::Ethernet as u16 {
            let protocol_type = arp_frame.protocol_type();

            if protocol_type == IPV4_ETHER_TYPE {
                let mut updated_translation_table = false;
                let sender_ipv4_address = arp_frame.sender_ipv4_addr().unwrap();
                let sender_mac_address = arp_frame.sender_mac_addr().unwrap();

                if !self
                    .arp_table
                    .lock()
                    .unwrap()
                    .contains_key(&sender_ipv4_address)
                {
                    self.arp_table
                        .lock()
                        .unwrap()
                        .insert(sender_ipv4_address, sender_mac_address);
                    updated_translation_table = true;
                }

                let target_ipv4_address = arp_frame.target_ipv4_addr().unwrap();
                if target_ipv4_address == ROUTER_IPV4_ADDR {
                    if !updated_translation_table {
                        self.arp_table
                            .lock()
                            .unwrap()
                            .insert(sender_ipv4_address, sender_mac_address);
                    }

                    if arp_frame.opcode() == ArpOp::Request as u16 {
                        // Generate response ARP frame
                        arp_frame.set_target_hardware_addr(sender_mac_address);
                        arp_frame.set_target_protocol_addr(IpAddr::V4(sender_ipv4_address));
                        // TODO: how can I find this machine's MacAddr?
                        // Will it be pre-populated in the ARP table?
                        arp_frame.set_sender_hardware_addr(MacAddr::new([0, 0, 0, 0, 0, 0]));
                        arp_frame.set_sender_protocol_addr(IpAddr::V4(ROUTER_IPV4_ADDR));
                        arp_frame.set_opcode(ArpOp::Reply as u16);

                        return Some(InterfaceAnnotated::<EthernetFrame> {
                            packet: arp_frame.frame(),
                            inbound_interface: packet.inbound_interface,
                            outbound_interface: packet.outbound_interface,
                        });
                    }
                }
            }
        }
        Some(InterfaceAnnotated::<EthernetFrame> {
            packet: arp_frame.frame(),
            inbound_interface: packet.inbound_interface,
            outbound_interface: packet.outbound_interface,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Interface;
    use route_rs_packets::ARP_ETHER_TYPE;

    fn stub_annotated_input(frame: EthernetFrame) -> InterfaceAnnotated<EthernetFrame> {
        InterfaceAnnotated::<EthernetFrame> {
            packet: frame,
            inbound_interface: Interface::Wan,
            outbound_interface: Interface::Lan,
        }
    }

    fn stub_arp_frame(sender_ipv4_addr: Ipv4Addr, sender_mac_addr: MacAddr) -> ArpFrame {
        let mut arp_frame = ArpFrame::default();
        arp_frame.set_hardware_type(ArpHardwareType::Ethernet as u16);
        arp_frame.set_protocol_type(IPV4_ETHER_TYPE);
        arp_frame.set_sender_protocol_addr(IpAddr::V4(sender_ipv4_addr));
        arp_frame.set_sender_hardware_addr(sender_mac_addr);
        arp_frame.set_opcode(ArpOp::Request as u16);
        let mut frame = arp_frame.frame();
        frame.set_ether_type(ARP_ETHER_TYPE);
        ArpFrame::try_from(frame).unwrap()
    }

    #[test]
    fn new_ip_mac_pairs_added_to_arp_table() {
        let sender_ipv4_addr = Ipv4Addr::new(1, 2, 3, 4);
        let sender_mac_addr = MacAddr::new([1, 2, 3, 4, 5, 6]);

        // Initialize empty IP to MAC translation table
        let arp_table: Arc<Mutex<HashMap<Ipv4Addr, MacAddr>>> =
            Arc::new(Mutex::new(HashMap::new()));

        ArpHandler::new(arp_table.clone())
            .process(stub_annotated_input(
                stub_arp_frame(sender_ipv4_addr, sender_mac_addr).frame(),
            ))
            .unwrap();

        // Arp table sniffed the new pair
        assert_eq!(
            arp_table.lock().unwrap().get(&sender_ipv4_addr).unwrap(),
            &sender_mac_addr
        );
    }

    #[test]
    fn arp_requests_not_addressed_to_router_gets_no_reply() {
        let sender_ipv4_addr = Ipv4Addr::new(1, 2, 3, 4);
        let sender_mac_addr = MacAddr::new([1, 2, 3, 4, 5, 6]);

        let output = ArpHandler::new(Arc::new(Mutex::new(HashMap::new())))
            .process(stub_annotated_input(
                stub_arp_frame(sender_ipv4_addr, sender_mac_addr).frame(),
            ))
            .unwrap();

        // Not addressed to router, so ARP frame is unchanged
        assert_eq!(output.packet.ether_type(), ARP_ETHER_TYPE);
        let output_arp_frame = ArpFrame::try_from(output.packet).unwrap();
        assert_eq!(output_arp_frame.opcode(), ArpOp::Request as u16);
        assert_eq!(
            output_arp_frame.sender_ipv4_addr().unwrap(),
            sender_ipv4_addr
        );
        assert_eq!(output_arp_frame.sender_mac_addr().unwrap(), sender_mac_addr);
    }

    #[test]
    fn arp_requests_addressed_to_router_gets_reply() {
        let sender_ipv4_addr = Ipv4Addr::new(1, 2, 3, 4);
        let sender_mac_addr = MacAddr::new([1, 2, 3, 4, 5, 6]);

        let mut arp_frame = stub_arp_frame(sender_ipv4_addr, sender_mac_addr);
        arp_frame.set_target_protocol_addr(IpAddr::V4(ROUTER_IPV4_ADDR));

        let output = ArpHandler::new(Arc::new(Mutex::new(HashMap::new())))
            .process(stub_annotated_input(arp_frame.frame()))
            .unwrap();

        // Addressed to router, so ARP frame is a Reply, with updated sender values
        assert_eq!(output.packet.ether_type(), ARP_ETHER_TYPE);
        let output_arp_frame = ArpFrame::try_from(output.packet).unwrap();
        assert_eq!(output_arp_frame.opcode(), ArpOp::Reply as u16);
        assert_eq!(
            output_arp_frame.sender_ipv4_addr().unwrap(),
            ROUTER_IPV4_ADDR
        );
        // assert_eq!(
        //     output_arp_frame.sender_mac_addr().unwrap(),
        //     MacAddr::new([0, 0, 0, 0, 0, 0])
        // );
        assert_eq!(
            output_arp_frame.target_ipv4_addr().unwrap(),
            sender_ipv4_addr
        );
        assert_eq!(output_arp_frame.target_mac_addr().unwrap(), sender_mac_addr);
    }
}
