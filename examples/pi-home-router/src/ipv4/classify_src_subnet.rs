use crate::types::InterfaceAnnotated;
use cidr::{Cidr, Ipv4Cidr};
use route_rs_packets::Ipv4Packet;
use route_rs_runtime::classifier::Classifier;
use std::collections::HashMap;

/// Classifies an Ipv4Packet by its source subnet.
///
/// This classifier uses a HashMap of CIDRs to subnet names. If subnets in the hashmap are
/// overlapping, we return the smallest matching subnet (i.e. the greatest network_length).
pub(crate) struct BySrcSubnet<SubnetName: Clone> {
    subnets_by_len_desc: Vec<Ipv4Cidr>,
    subnets_to_labels: HashMap<Ipv4Cidr, SubnetName>,
    default_subnet: SubnetName,
}

impl<SubnetName: Clone> BySrcSubnet<SubnetName> {
    pub(crate) fn new(
        subnets_to_labels: HashMap<Ipv4Cidr, SubnetName>,
        default_subnet: SubnetName,
    ) -> Self {
        let mut subnets = subnets_to_labels.keys().cloned().collect::<Vec<Ipv4Cidr>>();
        // Note that left and right are switched here because we are sorting in descending order
        subnets.sort_by(|left, right| right.network_length().cmp(&left.network_length()));
        BySrcSubnet {
            subnets_by_len_desc: subnets,
            subnets_to_labels,
            default_subnet,
        }
    }
}

impl<SubnetName: Clone> Classifier for BySrcSubnet<SubnetName> {
    type Packet = InterfaceAnnotated<Ipv4Packet>;
    type Class = SubnetName;

    fn classify(&self, annotated_packet: &Self::Packet) -> Self::Class {
        for cidr in &self.subnets_by_len_desc {
            if cidr.contains(&annotated_packet.packet.src_addr()) {
                return self.subnets_to_labels.get(&cidr).unwrap().clone();
            }
        }

        self.default_subnet.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Interface;
    use std::net::Ipv4Addr;

    #[test]
    fn match_inside_subnet() {
        let classifier = BySrcSubnet::new(
            maplit::hashmap! {
                Cidr::new(Ipv4Addr::new(192, 0, 2, 0), 24).unwrap() => "TEST-NET-1",
            },
            "DEFAULT-NET",
        );
        let ip_inside_subnet = Ipv4Addr::new(192, 0, 2, 21);
        let mut packet = Ipv4Packet::empty();
        packet.set_src_addr(ip_inside_subnet);

        assert_eq!(
            classifier.classify(&InterfaceAnnotated {
                packet,
                inbound_interface: Interface::Unmarked,
                outbound_interface: Interface::Unmarked,
            }),
            "TEST-NET-1"
        );
    }

    #[test]
    fn default_outside_subnet() {
        let classifier = BySrcSubnet::new(
            maplit::hashmap! {
                Cidr::new(Ipv4Addr::new(192, 0, 2, 0), 24).unwrap() => "TEST-NET-1",
            },
            "DEFAULT-NET",
        );
        let ip_outside_subnet = Ipv4Addr::new(198, 51, 100, 13);
        let mut packet = Ipv4Packet::empty();
        packet.set_src_addr(ip_outside_subnet);

        assert_eq!(
            classifier.classify(&InterfaceAnnotated {
                packet,
                inbound_interface: Interface::Unmarked,
                outbound_interface: Interface::Unmarked,
            }),
            "DEFAULT-NET"
        );
    }

    #[test]
    fn smallest_subnet() {
        let classifier = BySrcSubnet::new(
            maplit::hashmap! {
                Cidr::new(Ipv4Addr::new(192, 0, 2, 0), 24).unwrap() => "TEST-NET-1",
                Cidr::new(Ipv4Addr::new(192, 0, 2, 128), 26).unwrap() => "TEST-NET-1-THIRD-QUARTER",
            },
            "DEFAULT-NET",
        );
        let ip_inside_subnet = Ipv4Addr::new(192, 0, 2, 133);
        let mut packet = Ipv4Packet::empty();
        packet.set_src_addr(ip_inside_subnet);

        assert_eq!(
            classifier.classify(&InterfaceAnnotated {
                packet,
                inbound_interface: Interface::Unmarked,
                outbound_interface: Interface::Unmarked,
            }),
            "TEST-NET-1-THIRD-QUARTER"
        );
    }
}
