use crate::classifiers::Interface::*;
use route_rs_packets::{EthernetFrame, Ipv4Packet, Ipv6Packet};
use route_rs_runtime::classifier::Classifier;
use std::net::{Ipv4Addr, Ipv6Addr};
use treebitmap::IpLookupTable;

#[derive(Copy, Clone)]
pub enum Interface {
    Interface0,
    Interface1,
    Interface2,
}

pub struct Ipv4SubnetRouter {
    pub default_if: Interface,
    pub lookup_table: IpLookupTable<Ipv4Addr, Interface>,
}

impl Ipv4SubnetRouter {
    pub fn new(default_if: Interface) -> Self {
        let mut lookup_table = IpLookupTable::new();
        lookup_table.insert(Ipv4Addr::new(0, 0, 0, 0), 0, Interface0);
        lookup_table.insert(Ipv4Addr::new(10, 0, 0, 0), 8, Interface1);
        lookup_table.insert(Ipv4Addr::new(192, 168, 0, 0), 16, Interface2);
        lookup_table.insert(Ipv4Addr::new(10, 10, 10, 0), 24, Interface2);
        Ipv4SubnetRouter {
            default_if,
            lookup_table,
        }
    }
}

impl Classifier for Ipv4SubnetRouter {
    type Packet = Ipv4Packet;
    type Class = Interface;

    fn classify(&self, packet: &Self::Packet) -> Self::Class {
        if let Some(entry) = self.lookup_table.longest_match(packet.dest_addr()) {
            *entry.2
        } else {
            self.default_if
        }
    }
}

pub struct Ipv6SubnetRouter {
    pub default_if: Interface,
    pub lookup_table: IpLookupTable<Ipv6Addr, Interface>,
}

impl Ipv6SubnetRouter {
    pub fn new(default_if: Interface) -> Self {
        let mut lookup_table = IpLookupTable::new();
        lookup_table.insert(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0), 0, Interface0);
        lookup_table.insert(
            Ipv6Addr::new(0x2001, 0xdb8, 0xdead, 0, 0, 0, 0, 0),
            48,
            Interface1,
        );
        lookup_table.insert(
            Ipv6Addr::new(0x2001, 0xdb8, 0xbeef, 0, 0, 0, 0, 0),
            48,
            Interface2,
        );
        lookup_table.insert(
            Ipv6Addr::new(0x2001, 0xdb8, 0xdead, 0xbeef, 0, 0, 0, 0),
            64,
            Interface2,
        );
        Ipv6SubnetRouter {
            default_if,
            lookup_table,
        }
    }
}

impl Classifier for Ipv6SubnetRouter {
    type Packet = Ipv6Packet;
    type Class = Interface;

    fn classify(&self, packet: &Self::Packet) -> Self::Class {
        if let Some(entry) = self.lookup_table.longest_match(packet.dest_addr()) {
            *entry.2
        } else {
            self.default_if
        }
    }
}

pub enum ClassifyIPType {
    IPv6,
    IPv4,
    None,
}

/// Processor that determines whether an IP packet is IPv6 or Ipv4.
pub struct ClassifyIP;

// In this case, Processors take Ownership, Classifiers take references, in a way that makes sense.
impl Classifier for ClassifyIP {
    type Packet = EthernetFrame;
    type Class = ClassifyIPType;

    fn classify(&self, frame: &Self::Packet) -> Self::Class {
        // We have to check the etherType
        // https://en.wikipedia.org/wiki/EtherType
        let ether_type = frame.ether_type();
        if ether_type == 0x0800 {
            ClassifyIPType::IPv4
        } else if ether_type == 0x86DD {
            ClassifyIPType::IPv6
        } else {
            ClassifyIPType::None
        }
    }
}
