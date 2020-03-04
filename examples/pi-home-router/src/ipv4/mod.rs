use crate::interface::classifier::ByInboundInterface;
use crate::interface::processor::{InterfaceAnnotationDecap, InterfaceAnnotationEncap};
use crate::types::{Interface, InterfaceAnnotated};
use cidr::Cidr;
use cidr::Ipv4Cidr;
use route_rs_packets::Ipv4Packet;
use route_rs_runtime::link::primitive::{ClassifyLink, JoinLink, ProcessLink};
use route_rs_runtime::link::{Link, LinkBuilder, PacketStream, ProcessLinkBuilder};
use route_rs_runtime::processor::Identity;
use route_rs_runtime::unpack_link;
use std::net::Ipv4Addr;

mod classify_dest_subnet;
use classify_dest_subnet::ByDestSubnet;

pub(crate) struct HandleIpv4 {
    in_stream: Option<PacketStream<InterfaceAnnotated<Ipv4Packet>>>,
    wan_ip: Option<Ipv4Addr>,
}

#[allow(dead_code)]
impl HandleIpv4 {
    pub(crate) fn new() -> Self {
        HandleIpv4 {
            in_stream: None,
            wan_ip: None,
        }
    }

    pub(crate) fn wan_ip(self, wan_ip: Ipv4Addr) -> Self {
        match self.wan_ip {
            None => HandleIpv4 {
                in_stream: self.in_stream,
                wan_ip: Some(wan_ip),
            },
            Some(_) => panic!("HandleIPv4 takes only one wan_ip"),
        }
    }
}

impl LinkBuilder<InterfaceAnnotated<Ipv4Packet>, InterfaceAnnotated<Ipv4Packet>> for HandleIpv4 {
    fn ingressors(self, mut in_streams: Vec<PacketStream<InterfaceAnnotated<Ipv4Packet>>>) -> Self {
        if self.in_stream.is_some() {
            panic!("HandleIPv4 takes only one ingressor");
        }
        if in_streams.len() == 1 {
            HandleIpv4 {
                in_stream: Some(in_streams.remove(0)),
                wan_ip: self.wan_ip,
            }
        } else {
            panic!("HandleIPv4 takes exactly one ingressor");
        }
    }

    fn ingressor(self, in_stream: PacketStream<InterfaceAnnotated<Ipv4Packet>>) -> Self {
        match self.in_stream {
            None => HandleIpv4 {
                in_stream: Some(in_stream),
                wan_ip: self.wan_ip,
            },
            Some(_) => panic!("HandleIPv4 takes only one ingressor"),
        }
    }

    fn build_link(self) -> Link<InterfaceAnnotated<Ipv4Packet>> {
        assert!(
            self.in_stream.is_some(),
            "HandleIPv4 must have an ingressor defined"
        );
        assert!(
            self.wan_ip.is_some(),
            "HandleIPv4 must have a wan_ip defined"
        );

        let mut all_runnables = vec![];

        // Classify packets based on the inbound interface
        let classify_by_inbound_interface = ClassifyLink::new()
            .ingressor(self.in_stream.unwrap())
            .classifier(ByInboundInterface::new())
            .num_egressors(3)
            .dispatcher(Box::new(|interface| match interface {
                Interface::Host => Some(0),
                Interface::Lan => Some(1),
                Interface::Wan => Some(2),
                Interface::Unmarked => None,
            }))
            .build_link();
        unpack_link!(classify_by_inbound_interface => all_runnables, [from_host, from_lan, from_wan]);

        // Discard all packets from the WAN that aren't destined for my WAN IP. We only have one WAN
        // address, so anything else that comes from the WAN is an error by upstream. We don't care
        // about IPv4 broadcast packets because we aren't an IPv4 client in the WAN.
        let classify_by_dest_subnet_wan = ClassifyLink::new()
            .ingressor(from_wan)
            .classifier(ByDestSubnet::new(
                maplit::hashmap! {
                    Ipv4Cidr::new(self.wan_ip.unwrap(), 32).unwrap() => FromWanDestSubnet::MyWanIp,
                },
                FromWanDestSubnet::Other,
            ))
            .num_egressors(1)
            .dispatcher(Box::new(|subnet| match subnet {
                FromWanDestSubnet::MyWanIp => Some(0),
                FromWanDestSubnet::Other => None,
            }))
            .build_link();
        unpack_link!(classify_by_dest_subnet_wan => all_runnables, [from_wan_to_me_annotated]);

        // Since the NAT decapsulator doesn't care where the packets come from, and we're going to
        // rewrite the destination interface afterward anyways, we can just strip the annotation
        // entirely at this point. We'll rebuild it on the other side.
        let wan_to_me_annot_decap = ProcessLink::new()
            .ingressor(from_wan_to_me_annotated)
            .processor(InterfaceAnnotationDecap::new())
            .build_link();
        unpack_link!(wan_to_me_annot_decap => all_runnables, [from_wan_to_me]);

        // Send traffic from the WAN to the NAT decapsulator.
        //
        // TODO: Implement NAT decapsulator
        let nat_decap = ProcessLink::new()
            .ingressor(from_wan_to_me)
            .processor(Identity::new())
            .build_link();
        unpack_link!(nat_decap => all_runnables, [from_nat_decap]);

        // Since all NAT decapsulated traffic is from the WAN bound for the LAN, we can just
        // regenerate the interface annotations here.
        let nat_decap_to_lan = ProcessLink::new()
            .ingressor(from_nat_decap)
            .processor(InterfaceAnnotationEncap::new(
                Interface::Wan,
                Interface::Lan,
            ))
            .build_link();
        unpack_link!(nat_decap_to_lan => all_runnables, [from_nat_decap_annotated]);

        // Join everything so we have one outbound packet stream
        let final_join = JoinLink::new()
            .ingressors(vec![from_host, from_lan, from_nat_decap_annotated])
            .build_link();
        unpack_link!(final_join => all_runnables, final_join_egressors);

        (all_runnables, final_join_egressors)
    }
}

#[derive(Clone)]
enum FromWanDestSubnet {
    MyWanIp,
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;
    use route_rs_runtime::utils::test::harness::{initialize_runtime, test_link};
    use route_rs_runtime::utils::test::packet_generators::immediate_stream;

    const MY_WAN_IP: Ipv4Addr = Ipv4Addr::new(192, 0, 2, 1);
    const OTHER_WAN_IP: Ipv4Addr = Ipv4Addr::new(192, 0, 2, 101);

    fn test_handleipv4_link(
        packets: Vec<InterfaceAnnotated<Ipv4Packet>>,
        wan_ip: Ipv4Addr,
    ) -> Vec<InterfaceAnnotated<Ipv4Packet>> {
        let mut runtime = initialize_runtime();
        let mut results = runtime.block_on(async {
            let link = HandleIpv4::new()
                .ingressor(immediate_stream(packets))
                .wan_ip(wan_ip)
                .build_link();
            test_link(link, None).await
        });
        results.remove(0)
    }

    #[test]
    fn drop_inbound_interface_unmarked() {
        let from_host = InterfaceAnnotated {
            packet: Ipv4Packet::empty(),
            inbound_interface: Interface::Host,
            outbound_interface: Interface::Unmarked,
        };
        let from_unmarked = InterfaceAnnotated {
            packet: Ipv4Packet::empty(),
            inbound_interface: Interface::Unmarked,
            outbound_interface: Interface::Unmarked,
        };

        let output_packets = test_handleipv4_link(vec![from_host, from_unmarked], MY_WAN_IP);

        assert_eq!(output_packets.len(), 1);
        assert!(output_packets
            .iter()
            .all(|pkt| { pkt.inbound_interface != Interface::Unmarked }))
    }

    #[test]
    fn drop_wan_not_for_me() {
        let mut from_wan_to_me_packet = Ipv4Packet::empty();
        from_wan_to_me_packet.set_dest_addr(MY_WAN_IP);
        let from_wan_to_me = InterfaceAnnotated {
            packet: from_wan_to_me_packet,
            inbound_interface: Interface::Wan,
            outbound_interface: Interface::Unmarked,
        };
        let mut from_wan_to_other_packet = Ipv4Packet::empty();
        from_wan_to_other_packet.set_dest_addr(OTHER_WAN_IP);
        let from_wan_to_other = InterfaceAnnotated {
            packet: from_wan_to_other_packet,
            inbound_interface: Interface::Wan,
            outbound_interface: Interface::Unmarked,
        };

        let output_packets =
            test_handleipv4_link(vec![from_wan_to_me, from_wan_to_other], MY_WAN_IP);

        assert_eq!(output_packets.len(), 1);
        assert!(output_packets
            .iter()
            .all(|pkt| { pkt.packet.dest_addr() == MY_WAN_IP }))
    }

    #[test]
    fn from_wan_to_me_goes_to_lan() {
        let mut from_wan_to_me_packet = Ipv4Packet::empty();
        from_wan_to_me_packet.set_dest_addr(MY_WAN_IP);
        let from_wan_to_me = InterfaceAnnotated {
            packet: from_wan_to_me_packet,
            inbound_interface: Interface::Wan,
            outbound_interface: Interface::Unmarked,
        };

        let output_packets = test_handleipv4_link(vec![from_wan_to_me], MY_WAN_IP);

        assert_eq!(output_packets.len(), 1);
        assert_eq!(
            output_packets.first().unwrap().outbound_interface,
            Interface::Lan
        );
    }
}
