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

/// Processor that determines whether an IP packet is IPv6 or Ipv4 via the EtherType field in the
/// EthernetFrame
pub struct ClassifyIP;

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

#[cfg(test)]
mod tests {
    use super::*;
    use route_rs_packets::EthernetFrame;
    use route_rs_runtime::link::primitive::ClassifyLink;
    use route_rs_runtime::link::LinkBuilder;
    use route_rs_runtime::utils::test::harness::{initialize_runtime, run_link};
    use route_rs_runtime::utils::test::packet_generators::immediate_stream;

    #[test]
    fn classify_ip() {
        let data_v4: Vec<u8> = vec![
            0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0x08, 00, 0x45, 0, 0, 20, 0, 0,
            0, 0, 64, 17, 0, 0, 192, 178, 128, 0, 10, 0, 0, 1,
        ];
        let data_v6: Vec<u8> = vec![
            0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0x86, 0xDD, 0x60, 0, 0, 0, 0, 4,
            17, 64, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde,
            0xad, 0xbe, 0xef, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0xa, 0xb, 0xc,
            0xd,
        ];
        let data_unknown: Vec<u8> = vec![
            0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0xff, 0xff,
        ];
        let frame1 = EthernetFrame::from_buffer(data_v4, 0).unwrap();
        let frame2 = EthernetFrame::from_buffer(data_v6, 0).unwrap();
        let frame3 = EthernetFrame::from_buffer(data_unknown, 0).unwrap();

        let mut runtime = initialize_runtime();
        let results = runtime.block_on(async {
            let packets = vec![frame1.clone(), frame2.clone(), frame3.clone()];

            let link = ClassifyLink::new()
                .ingressor(immediate_stream(packets))
                .classifier(ClassifyIP)
                .dispatcher(Box::new(|c| match c {
                    ClassifyIPType::IPv4 => 0,
                    ClassifyIPType::IPv6 => 1,
                    ClassifyIPType::None => 2,
                }))
                .num_egressors(3)
                .build_link();

            run_link(link).await
        });
        assert_eq!(results[0][0], frame1);
        assert_eq!(results[1][0], frame2);
        assert_eq!(results[2][0], frame3);
    }

    #[test]
    fn route_ipv4() {
        let data_v4: Vec<u8> = vec![
            0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0x08, 00, 0x45, 0, 0, 20, 0, 0,
            0, 0, 64, 17, 0, 0, 192, 178, 128, 0, 10, 0, 0, 1,
        ];
        let mut packet_interface0 = Ipv4Packet::from_buffer(data_v4.clone(), Some(0), 14).unwrap();
        let mut packet_interface1 = Ipv4Packet::from_buffer(data_v4.clone(), Some(0), 14).unwrap();
        let mut packet_interface2 = Ipv4Packet::from_buffer(data_v4.clone(), Some(0), 14).unwrap();
        let mut packet_default = Ipv4Packet::from_buffer(data_v4, Some(0), 14).unwrap();

        let mut runtime = initialize_runtime();
        let results = runtime.block_on(async {
            packet_interface0.set_dest_addr(Ipv4Addr::new(0, 0, 0, 0));
            packet_interface1.set_dest_addr(Ipv4Addr::new(10, 0, 0, 14));
            packet_interface2.set_dest_addr(Ipv4Addr::new(192, 168, 10, 5));
            packet_default.set_dest_addr(Ipv4Addr::new(192, 167, 0, 0));

            let packets = vec![
                packet_interface0.clone(),
                packet_interface1.clone(),
                packet_interface2.clone(),
                packet_default.clone(),
            ];

            let ipv4_router = Ipv4SubnetRouter::new(Interface0);
            let link = ClassifyLink::new()
                .ingressor(immediate_stream(packets))
                .num_egressors(3)
                .classifier(ipv4_router)
                .dispatcher(Box::new(|c| match c {
                    Interface0 => 0,
                    Interface1 => 1,
                    Interface2 => 2,
                }))
                .build_link();

            run_link(link).await
        });

        assert_eq!(results[0][0], packet_interface0);
        assert_eq!(results[0][1], packet_default);
        assert_eq!(results[1][0], packet_interface1);
        assert_eq!(results[2][0], packet_interface2);
    }

    #[test]
    fn route_ipv6() {
        let data_v6: Vec<u8> = vec![
            0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0x86, 0xDD, 0x60, 0, 0, 0, 0, 4,
            17, 64, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde,
            0xad, 0xbe, 0xef, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0xa, 0xb, 0xc,
            0xd,
        ];
        let mut packet_interface0 = Ipv6Packet::from_buffer(data_v6.clone(), Some(0), 14).unwrap();
        let mut packet_interface1 = Ipv6Packet::from_buffer(data_v6.clone(), Some(0), 14).unwrap();
        let mut packet_interface2 = Ipv6Packet::from_buffer(data_v6.clone(), Some(0), 14).unwrap();
        let mut packet_default = Ipv6Packet::from_buffer(data_v6, Some(0), 14).unwrap();

        let mut runtime = initialize_runtime();
        let results = runtime.block_on(async {
            packet_interface0.set_dest_addr(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0));
            packet_interface1.set_dest_addr(Ipv6Addr::new(0x2001, 0xdb8, 0xdead, 1, 2, 3, 4, 5));
            packet_interface2.set_dest_addr(Ipv6Addr::new(0x2001, 0xdb8, 0xbeef, 6, 7, 8, 9, 10));
            packet_default.set_dest_addr(Ipv6Addr::new(0x2001, 0xdb8, 0xdeaf, 0xbeef, 0, 1, 2, 3));

            let packets = vec![
                packet_interface0.clone(),
                packet_interface1.clone(),
                packet_interface2.clone(),
                packet_default.clone(),
            ];

            let ipv6_router = Ipv6SubnetRouter::new(Interface0);
            let link = ClassifyLink::new()
                .ingressor(immediate_stream(packets))
                .num_egressors(3)
                .classifier(ipv6_router)
                .dispatcher(Box::new(|c| match c {
                    Interface0 => 0,
                    Interface1 => 1,
                    Interface2 => 2,
                }))
                .build_link();

            run_link(link).await
        });

        assert_eq!(results[0][0], packet_interface0);
        assert_eq!(results[0][1], packet_default);
        assert_eq!(results[1][0], packet_interface1);
        assert_eq!(results[2][0], packet_interface2);
    }
}
