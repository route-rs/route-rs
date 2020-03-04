use crate::arp::{ipv4_array, ipv6_array};
use route_rs_packets::MacAddr;
use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};

pub(crate) struct ArpTable {
    /// Should we have separate translation tables for different layer 3 protocols so keys have a fixed size?
    // Mappings between ((protocol type, protocol address) -> 48-bit Ethernet address)
    ipv4_mac_translations: HashMap<Ipv4Addr, MacAddr>,
    ipv6_mac_translations: HashMap<Ipv6Addr, MacAddr>,
}

impl ArpTable {
    pub fn new() -> Self {
        ArpTable {
            ipv4_mac_translations: HashMap::new(),
            ipv6_mac_translations: HashMap::new(),
        }
    }

    pub fn contains_key(&self, protocol_type: u16, protocol_address: &[u8]) -> bool {
        match protocol_type {
            0x0800 => self
                .ipv4_mac_translations
                .contains_key(&Ipv4Addr::from(ipv4_array(protocol_address))),
            0x86DD => self
                .ipv6_mac_translations
                .contains_key(&Ipv6Addr::from(ipv6_array(protocol_address))),
            _ => false,
        }
    }

    pub fn get(&self, protocol_type: u16, protocol_address: &[u8]) -> Option<&MacAddr> {
        match protocol_type {
            0x0800 => self
                .ipv4_mac_translations
                .get(&Ipv4Addr::from(ipv4_array(protocol_address))),
            0x86DD => self
                .ipv6_mac_translations
                .get(&Ipv6Addr::from(ipv6_array(protocol_address))),
            _ => None,
        }
    }

    pub fn insert(
        &mut self,
        protocol_type: u16,
        protocol_address: &[u8],
        mac_address: MacAddr,
    ) -> Option<MacAddr> {
        match protocol_type {
            0x0800 => self
                .ipv4_mac_translations
                .insert(Ipv4Addr::from(ipv4_array(protocol_address)), mac_address),
            0x86DD => self
                .ipv6_mac_translations
                .insert(Ipv6Addr::from(ipv6_array(protocol_address)), mac_address),
            _ => None,
        }
    }
}
