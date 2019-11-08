use crossbeam::crossbeam_channel;
use route_rs_packets::EthernetFrame;
use route_rs_runtime::link::{
    primitive::{ForkLink, JoinLink},
    Link, LinkBuilder, PacketStream,
};
use route_rs_runtime::pipeline::Runner;
use route_rs_runtime::processor::Processor;

// I'd really like to use something like this.
// use route_rs_runtime::utils::test::harness::run_link;
// use route_rs_runtime::utils::test::packet_generators::{immediate_stream, PacketIntervalGenerator};

mod processors;

fn main() {
    let to_ipv6 = processors::Ipv6Encap;
    let to_ipv4 = processors::Ipv4Encap;

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

#[derive(Default)]
pub struct Router<EthernetFrame> {
    in_streams: Option<Vec<PacketStream<EthernetFrame>>>,
}

impl<EthernetFrame> Router<EthernetFrame> {
    pub fn new() -> Self {
        Router { in_streams: None }
    }
}

impl<EthernetFrame> LinkBuilder<EthernetFrame, EthernetFrame> for Router<EthernetFrame> {
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
            (vec![], vec![])
        }
    }
}
