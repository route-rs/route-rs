use crossbeam::crossbeam_channel;
use route_rs_packets::EthernetFrame;
use route_rs_runtime::link::{
    primitive::{ClassifyLink, JoinLink, ProcessLink},
    Link, LinkBuilder, PacketStream, ProcessLinkBuilder,
};
use route_rs_runtime::pipeline::Runner;
use route_rs_runtime::processor::Processor;

// I'd really like to use something like this.
// use route_rs_runtime::utils::test::harness::run_link;
// use route_rs_runtime::utils::test::packet_generators::{immediate_stream, PacketIntervalGenerator};

mod processors;

fn main() {
    let (input_packet_sender, input_receiver) = crossbeam_channel::unbounded();
    let input_packets: Vec<EthernetFrame> = vec![]; //Put packets here.

    for p in input_packets {
        match input_packet_sender.send(p) {
            Ok(_) => {}
            Err(err) => panic!("Input channel error {}", err),
        }
    }

    // Then we call our runner on our router

    // Then we collect the outputs and see if it works
}

// Note that Router is not Generic! This router only takes in EthernetFrames
#[derive(Default)]
pub struct Router {
    in_streams: Option<Vec<PacketStream<EthernetFrame>>>,
}

impl Router {
    pub fn new() -> Self {
        Router { in_streams: None }
    }
}

// Then we declare it is a link that take in EthernetFrames and outputs EthernetFrames
// LinkBuilder is always generic, so we need to fill out EthernetFrame
impl LinkBuilder<EthernetFrame, EthernetFrame> for Router {
    fn ingressors(self, in_streams: Vec<PacketStream<EthernetFrame>>) -> Self {
        assert!(
            in_streams.len() == 1,
            "Support only one input interface for now"
        );
        Router {
            in_streams: Some(in_streams),
        }
    }

    fn build_link(self) -> Link<EthernetFrame> {
        if self.in_streams.is_none() {
            panic!("Can not build link, missing input stream");
        } else {
            // TODO: Build the router here

            // Skeleton Concept:
            //                /---Ipv4Encap ---SetIpv4Subnet---Ipv4Decap--\                       /-- Interface 0
            //   ClassifyIP <                                               > ClassifyInterface <  -- Interface 1
            //                \---Ipv6Encap ---SetIpv6Subnet---Ipv6Decap--/                       \-- Interface 2

            //return an empty thing for now so it compiles.
            let mut all_runnables = vec![];

            let (mut classify_runables, mut classify_egressors) = ClassifyLink::new()
                .ingressors(self.in_streams.unwrap())
                .num_egressors(2)
                .classifier(processors::ClassifyIP)
                .dispatcher(Box::new(|c| match c {
                    processors::ClassifyIPType::IPv4 => 0,
                    processors::ClassifyIPType::IPv6 => 1,
                    processors::ClassifyIPType::None => 1, //Is there a way to drop packets in a classify?
                }))
                .build_link();
            all_runnables.append(&mut classify_runables);

            let (mut ipv4_encap_runnables, ipv4_encap_egressors) = ProcessLink::new()
                .ingressor(classify_egressors.remove(0))
                .processor(processors::Ipv4Encap)
                .build_link();
            all_runnables.append(&mut ipv4_encap_runnables);

            let (mut ipv4_setsubnet_runnables, ipv4_setsubnet_egressors) = ProcessLink::new()
                .ingressors(ipv4_encap_egressors)
                .processor(processors::SetIpv4Subnet)
                .build_link();
            all_runnables.append(&mut ipv4_setsubnet_runnables);

            let (mut ipv4_decap_runnables, mut ipv4_decap_egressors) = ProcessLink::new()
                .ingressors(ipv4_setsubnet_egressors)
                .processor(processors::Ipv4Decap)
                .build_link();
            all_runnables.append(&mut ipv4_decap_runnables);

            let (mut ipv6_encap_runnables, ipv6_encap_egressors) = ProcessLink::new()
                .ingressor(classify_egressors.remove(1))
                .processor(processors::Ipv6Encap)
                .build_link();
            all_runnables.append(&mut ipv6_encap_runnables);

            let (mut ipv6_setsubnet_runnables, ipv6_setsubnet_egressors) = ProcessLink::new()
                .ingressors(ipv6_encap_egressors)
                .processor(processors::SetIpv6Subnet)
                .build_link();
            all_runnables.append(&mut ipv6_setsubnet_runnables);

            let (mut ipv6_decap_runnables, mut ipv6_decap_egressors) = ProcessLink::new()
                .ingressors(ipv6_setsubnet_egressors)
                .processor(processors::Ipv6Decap)
                .build_link();
            all_runnables.append(&mut ipv6_decap_runnables);

            ipv6_decap_egressors.append(&mut ipv4_decap_egressors);
            let (mut ip_join_runnables, mut ip_join_egressors) = JoinLink::new()
                .ingressors(ipv6_decap_egressors)
                .build_link();
            all_runnables.append(&mut ip_join_runnables);

            // TODO:  Make this go out to different interfaces based on the subnet?

            (all_runnables, vec![])
        }
    }
}
