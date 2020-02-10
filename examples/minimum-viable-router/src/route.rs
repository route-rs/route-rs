use crate::classifier;
use crate::classifier::Protocol;
use crate::processor::{DecapInterfaceTags, EncapInterfaceTags, Interface};
use route_rs_packets::EthernetFrame;
use route_rs_runtime::link::primitive::{BlackHoleLink, ClassifyLink, JoinLink, ProcessLink};
use route_rs_runtime::link::{Link, LinkBuilder, PacketStream, ProcessLinkBuilder};
use route_rs_runtime::processor::{Identity, Trace};

pub struct Route {
    in_streams: Option<Vec<PacketStream<EthernetFrame>>>,
}

impl Route {
    pub fn new() -> Self {
        Route { in_streams: None }
    }
}

impl LinkBuilder<EthernetFrame, EthernetFrame> for Route {
    fn ingressors(self, in_streams: Vec<PacketStream<EthernetFrame>>) -> Self {
        assert_eq!(in_streams.len(), 3, "Wrong number of inputs to Route");

        Route {
            in_streams: Some(in_streams),
        }
    }

    fn ingressor(self, in_stream: PacketStream<EthernetFrame>) -> Self {
        match self.in_streams {
            Some(mut streams) => {
                assert!(streams.len() < 3, "May only provide 3 input streams");
                streams.push(in_stream);
                Route {
                    in_streams: Some(streams),
                }
            }
            None => Route {
                in_streams: Some(vec![in_stream]),
            },
        }
    }

    fn build_link(self) -> Link<EthernetFrame> {
        let in_stream_wan;
        let in_stream_lan;
        let in_stream_host;
        match self.in_streams {
            Some(mut streams) => {
                assert_eq!(streams.len(), 3, "Must provide 3 input streams to Route");
                in_stream_wan = streams.remove(0);
                in_stream_lan = streams.remove(0);
                in_stream_host = streams.remove(0);
            }
            None => panic!("Must provide some input streams to Route"),
        }

        let mut all_runnables = vec![];
        let mut all_egressors = vec![];

        // Encap from interfaces BEGIN
        let encap_wan_src = EncapInterfaceTags::new(Some(Interface::WAN), None);
        let encap_lan_src = EncapInterfaceTags::new(Some(Interface::LAN), None);
        let encap_host_src = EncapInterfaceTags::new(Some(Interface::Host), None);

        let (mut encap_wan_src_runnables, mut encap_wan_src_egressors) = ProcessLink::new()
            .processor(encap_wan_src)
            .ingressor(in_stream_wan)
            .build_link();
        all_runnables.append(&mut encap_wan_src_runnables);
        let encap_wan_src_egressor_0 = encap_wan_src_egressors.remove(0);

        let (mut encap_lan_src_runnables, mut encap_lan_src_egressors) = ProcessLink::new()
            .processor(encap_lan_src)
            .ingressor(in_stream_lan)
            .build_link();
        all_runnables.append(&mut encap_lan_src_runnables);
        let encap_lan_src_egressor_0 = encap_lan_src_egressors.remove(0);

        let (mut encap_host_src_runnables, mut encap_host_src_egressors) = ProcessLink::new()
            .processor(encap_host_src)
            .ingressor(in_stream_host)
            .build_link();
        all_runnables.append(&mut encap_host_src_runnables);
        let encap_host_src_egressor_0 = encap_host_src_egressors.remove(0);
        // Encap from interfaces END

        // Join interfaces BEGIN
        let (mut join_interfaces_runnables, mut join_interfaces_egressors) = JoinLink::new()
            .ingressors(vec![
                encap_wan_src_egressor_0,
                encap_lan_src_egressor_0,
                encap_host_src_egressor_0,
            ])
            .build_link();
        all_runnables.append(&mut join_interfaces_runnables);
        let join_interfaces_egressor_0 = join_interfaces_egressors.remove(0);
        // Join interfaces END

        // Ingress trace BEGIN
        let ingress_trace = Trace::new();
        let (mut ingress_trace_runnables, mut ingress_trace_egressors) = ProcessLink::new()
            .ingressor(join_interfaces_egressor_0)
            .processor(ingress_trace)
            .build_link();
        all_runnables.append(&mut ingress_trace_runnables);
        let ingress_trace_egressor_0 = ingress_trace_egressors.remove(0);
        // Ingress trace END

        // Classify based on Protocol BEGIN
        let (mut classify_protocol_runnables, mut classify_protocol_egressors) =
            ClassifyLink::new()
                .ingressor(ingress_trace_egressor_0)
                .num_egressors(6)
                .classifier(classifier::ClassifyByProtocol::new())
                .dispatcher(Box::new(|c| match c {
                    Protocol::Unknown => 0,
                    Protocol::ARP => 1,
                    Protocol::NDP => 2,
                    Protocol::DHCP => 3,
                    Protocol::IPv4 => 4,
                    Protocol::IPv6 => 5,
                }))
                .build_link();
        all_runnables.append(&mut classify_protocol_runnables);
        let classify_protocol_egressor_0 = classify_protocol_egressors.remove(0);
        let classify_protocol_egressor_1 = classify_protocol_egressors.remove(0);
        let classify_protocol_egressor_2 = classify_protocol_egressors.remove(0);
        let classify_protocol_egressor_3 = classify_protocol_egressors.remove(0);
        let classify_protocol_egressor_4 = classify_protocol_egressors.remove(0);
        let classify_protocol_egressor_5 = classify_protocol_egressors.remove(0);

        let (mut unknown_proto_drop_runnables, _unknown_proto_drop_egressors) =
            BlackHoleLink::new()
                .ingressor(classify_protocol_egressor_0)
                .build_link();
        all_runnables.append(&mut unknown_proto_drop_runnables);
        // Classify based on Protocol END

        // ARP BEGIN
        let (mut handle_arp_runnables, mut handle_arp_egressors) = ProcessLink::new()
            .ingressor(classify_protocol_egressor_1)
            .processor(Identity::new())
            .build_link();
        all_runnables.append(&mut handle_arp_runnables);
        let handle_arp_egressor_0 = handle_arp_egressors.remove(0);
        // ARP END

        // NDP BEGIN
        let (mut handle_ndp_runnables, mut handle_ndp_egressors) = ProcessLink::new()
            .ingressor(classify_protocol_egressor_2)
            .processor(Identity::new())
            .build_link();
        all_runnables.append(&mut handle_ndp_runnables);
        let handle_ndp_egressor_0 = handle_ndp_egressors.remove(0);
        // NDP END

        // DHCP BEGIN
        let (mut handle_dhcp_runnables, mut handle_dhcp_egressors) = ProcessLink::new()
            .ingressor(classify_protocol_egressor_3)
            .processor(Identity::new())
            .build_link();
        all_runnables.append(&mut handle_dhcp_runnables);
        let handle_dhcp_egressor_0 = handle_dhcp_egressors.remove(0);
        // DHCP END

        // IPv4 BEGIN
        let (mut handle_ipv4_runnables, mut handle_ipv4_egressors) = ProcessLink::new()
            .ingressor(classify_protocol_egressor_4)
            .processor(Identity::new())
            .build_link();
        all_runnables.append(&mut handle_ipv4_runnables);
        let handle_ipv4_egressor_0 = handle_ipv4_egressors.remove(0);
        // IPv4 END

        // IPv6 BEGIN
        let (mut handle_ipv6_runnables, mut handle_ipv6_egressors) = ProcessLink::new()
            .ingressor(classify_protocol_egressor_5)
            .processor(Identity::new())
            .build_link();
        all_runnables.append(&mut handle_ipv6_runnables);
        let handle_ipv6_egressor_0 = handle_ipv6_egressors.remove(0);
        // IPv6 END

        // Join output packets BEGIN
        let (mut join_outputs_runnables, mut join_outputs_egressors) = JoinLink::new()
            .ingressors(vec![
                handle_arp_egressor_0,
                handle_ndp_egressor_0,
                handle_dhcp_egressor_0,
                handle_ipv4_egressor_0,
                handle_ipv6_egressor_0,
            ])
            .build_link();
        all_runnables.append(&mut join_outputs_runnables);
        let join_outputs_egressor_0 = join_outputs_egressors.remove(0);
        // Join output packets END

        // Egress trace BEGIN
        let egress_trace = Trace::new();
        let (mut egress_trace_runnables, mut egress_trace_egressors) = ProcessLink::new()
            .ingressor(join_outputs_egressor_0)
            .processor(egress_trace)
            .build_link();
        all_runnables.append(&mut egress_trace_runnables);
        let egress_trace_egressor_0 = egress_trace_egressors.remove(0);
        // Egress trace END

        // Dispatch interfaces BEGIN
        let (mut classify_dst_interfaces_runnables, mut classify_dst_interfaces_egressors) =
            ClassifyLink::new()
                .ingressors(vec![egress_trace_egressor_0])
                .num_egressors(4)
                .classifier(classifier::ClassifyByDestinationInterface::new())
                .dispatcher(Box::new(|c| match c {
                    None => 0,
                    Some(Interface::WAN) => 1,
                    Some(Interface::LAN) => 2,
                    Some(Interface::Host) => 3,
                }))
                .build_link();
        all_runnables.append(&mut classify_dst_interfaces_runnables);
        let classify_dst_interfaces_egressor_0 = classify_dst_interfaces_egressors.remove(0);
        let classify_dst_interfaces_egressor_1 = classify_dst_interfaces_egressors.remove(0);
        let classify_dst_interfaces_egressor_2 = classify_dst_interfaces_egressors.remove(0);
        let classify_dst_interfaces_egressor_3 = classify_dst_interfaces_egressors.remove(0);

        let (mut no_dst_drop_runnables, _no_dst_drop_egressors) = BlackHoleLink::new()
            .ingressor(classify_dst_interfaces_egressor_0)
            .build_link();
        all_runnables.append(&mut no_dst_drop_runnables);
        // Dispatch interfaces END

        // Decap to interfaces BEGIN
        let decap_wan_dst = DecapInterfaceTags::new();
        let decap_lan_dst = DecapInterfaceTags::new();
        let decap_host_dst = DecapInterfaceTags::new();

        let (mut decap_wan_dst_runnables, mut decap_wan_dst_egressors) = ProcessLink::new()
            .processor(decap_wan_dst)
            .ingressor(classify_dst_interfaces_egressor_1)
            .build_link();
        all_runnables.append(&mut decap_wan_dst_runnables);
        all_egressors.append(&mut decap_wan_dst_egressors);

        let (mut decap_lan_dst_runnables, mut decap_lan_dst_egressors) = ProcessLink::new()
            .processor(decap_lan_dst)
            .ingressor(classify_dst_interfaces_egressor_2)
            .build_link();
        all_runnables.append(&mut decap_lan_dst_runnables);
        all_egressors.append(&mut decap_lan_dst_egressors);

        let (mut decap_host_dst_runnables, mut decap_host_dst_egressors) = ProcessLink::new()
            .processor(decap_host_dst)
            .ingressor(classify_dst_interfaces_egressor_3)
            .build_link();
        all_runnables.append(&mut decap_host_dst_runnables);
        all_egressors.append(&mut decap_host_dst_egressors);
        // Decap to interfaces END

        (all_runnables, all_egressors)
    }
}
