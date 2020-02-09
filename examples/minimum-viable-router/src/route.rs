use crate::classifier;
use crate::processor::{DecapInterfaceTags, EncapInterfaceTags, Interface};
use route_rs_packets::EthernetFrame;
use route_rs_runtime::link::primitive::{BlackHoleLink, ClassifyLink, JoinLink, ProcessLink};
use route_rs_runtime::link::{Link, LinkBuilder, PacketStream, ProcessLinkBuilder};

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

        // Dispatch interfaces BEGIN
        let (mut classify_dst_interfaces_runnables, mut classify_dst_interfaces_egressors) =
            ClassifyLink::new()
                .ingressors(vec![join_interfaces_egressor_0])
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
