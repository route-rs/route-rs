use crossbeam::crossbeam_channel;
use futures::lazy;
use route_rs_packets::EthernetFrame;
use route_rs_runtime::link::{
    primitive::{ClassifyLink, InputChannelLink, JoinLink, OutputChannelLink, ProcessLink},
    Link, LinkBuilder, PacketStream, ProcessLinkBuilder,
};

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

    //--Declare whole router, as well as input and output links--//
    let mut all_runnables = vec![];
    let (mut input_runnables, input_egressors) =
        InputChannelLink::new().channel(input_receiver).build_link();
    all_runnables.append(&mut input_runnables);

    // Create our router
    let (mut router_runnables, mut router_egressors) =
        Router::new().ingressors(input_egressors).build_link();
    all_runnables.append(&mut router_runnables);

    // Create output channels to collect packets sent to each interface
    let (output0_sender, _output0) = crossbeam_channel::unbounded();
    let (output1_sender, _output1) = crossbeam_channel::unbounded();
    let (output2_sender, _output2) = crossbeam_channel::unbounded();

    let (mut interface0_runnables, _) = OutputChannelLink::new()
        .ingressor(router_egressors.remove(0))
        .channel(output0_sender)
        .build_link();
    all_runnables.append(&mut interface0_runnables);

    let (mut interface0_runnables, _) = OutputChannelLink::new()
        .ingressor(router_egressors.remove(0))
        .channel(output1_sender)
        .build_link();
    all_runnables.append(&mut interface0_runnables);

    let (mut interface0_runnables, _) = OutputChannelLink::new()
        .ingressor(router_egressors.remove(0))
        .channel(output2_sender)
        .build_link();
    all_runnables.append(&mut interface0_runnables);

    tokio::run(lazy(move || {
        for r in all_runnables {
            tokio::spawn(r);
        }
        Ok(())
    }));

    //TODO: Examine output of each interface for correctness
    println!("It finished!");
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

            // Skeleton Concept:                                        /--encap
            //               /--Ipv4Dencap--Ipv4SubnetRouter(Classifier)<--encap      /--Join--Interface 0
            //>--ClassifyIP--<                                          \--encap .... <--Join--Interface 1
            //               \                                          /--encap      \--Join--Interface 2
            //               \--Ipv6Dencap--Ipv6SubnetRouter(Classifier)<--encap
            //                                                          \--encap

            //return an empty thing for now so it compiles.
            let mut all_runnables = vec![];

            let ipv4_router = processors::Ipv4SubnetRouter::new(processors::Interface::Interface0);
            let ipv6_router = processors::Ipv6SubnetRouter::new(processors::Interface::Interface0);

            let (mut classify_runables, mut classify_egressors) = ClassifyLink::new()
                .ingressors(self.in_streams.unwrap())
                .num_egressors(2)
                .classifier(processors::ClassifyIP)
                .dispatcher(Box::new(|c| match c {
                    processors::ClassifyIPType::IPv4 => 0,
                    processors::ClassifyIPType::IPv6 => 1,
                    processors::ClassifyIPType::None => 1, // we can't drop packets in a classify. Maybe we do need
                })) // the DropLink back?
                .build_link();
            all_runnables.append(&mut classify_runables);

            //------------Ipv4 Subnet router--------------//

            // Reminder that process links don't have any runnables, so we can ignore that half of the tuple
            let (_, ipv4_dencap_egressors) = ProcessLink::new()
                .ingressor(classify_egressors.remove(0))
                .processor(processors::Ipv4Dencap)
                .build_link();

            let (mut ipv4_subnet_router_runnables, mut ipv4_subnet_router_egressors) =
                ClassifyLink::new()
                    .ingressors(ipv4_dencap_egressors)
                    .num_egressors(3)
                    .classifier(ipv4_router)
                    .dispatcher(Box::new(|c| match c {
                        processors::Interface::Interface0 => 0,
                        processors::Interface::Interface1 => 1,
                        processors::Interface::Interface2 => 2,
                    }))
                    .build_link();
            all_runnables.append(&mut ipv4_subnet_router_runnables);

            let (_, mut ipv4_encap_interface0_egressors) = ProcessLink::new()
                .ingressor(ipv4_subnet_router_egressors.remove(0))
                .processor(processors::Ipv4Encap)
                .build_link();

            let (_, mut ipv4_encap_interface1_egressors) = ProcessLink::new()
                .ingressor(ipv4_subnet_router_egressors.remove(0))
                .processor(processors::Ipv4Encap)
                .build_link();

            let (_, mut ipv4_encap_interface2_egressors) = ProcessLink::new()
                .ingressor(ipv4_subnet_router_egressors.remove(0))
                .processor(processors::Ipv4Encap)
                .build_link();

            //----------IPv6 Subnet Router--------------//

            let (_, ipv6_dencap_egressors) = ProcessLink::new()
                .ingressor(classify_egressors.remove(0))
                .processor(processors::Ipv6Dencap)
                .build_link();

            let (mut ipv6_subnet_router_runnables, mut ipv6_subnet_router_egressors) =
                ClassifyLink::new()
                    .ingressors(ipv6_dencap_egressors)
                    .num_egressors(3)
                    .classifier(ipv6_router)
                    .dispatcher(Box::new(|c| match c {
                        processors::Interface::Interface0 => 0,
                        processors::Interface::Interface1 => 1,
                        processors::Interface::Interface2 => 2,
                    }))
                    .build_link();
            all_runnables.append(&mut ipv6_subnet_router_runnables);

            let (_, mut ipv6_encap_interface0_egressors) = ProcessLink::new()
                .ingressor(ipv6_subnet_router_egressors.remove(0))
                .processor(processors::Ipv6Encap)
                .build_link();

            let (_, mut ipv6_encap_interface1_egressors) = ProcessLink::new()
                .ingressor(ipv6_subnet_router_egressors.remove(0))
                .processor(processors::Ipv6Encap)
                .build_link();

            let (_, mut ipv6_encap_interface2_egressors) = ProcessLink::new()
                .ingressor(ipv6_subnet_router_egressors.remove(0))
                .processor(processors::Ipv6Encap)
                .build_link();

            //---------Join to interfaces--------------//
            let mut interfaces = vec![];

            let (mut join0_runnables, mut interface0) = JoinLink::new()
                .ingressor(ipv4_encap_interface0_egressors.remove(0))
                .ingressor(ipv6_encap_interface0_egressors.remove(0))
                .build_link();
            all_runnables.append(&mut join0_runnables);
            interfaces.append(&mut interface0);

            let (mut join1_runnables, mut interface1) = JoinLink::new()
                .ingressor(ipv4_encap_interface1_egressors.remove(0))
                .ingressor(ipv6_encap_interface1_egressors.remove(0))
                .build_link();
            all_runnables.append(&mut join1_runnables);
            interfaces.append(&mut interface1);

            let (mut join2_runnables, mut interface2) = JoinLink::new()
                .ingressor(ipv4_encap_interface2_egressors.remove(0))
                .ingressor(ipv6_encap_interface2_egressors.remove(0))
                .build_link();
            all_runnables.append(&mut join2_runnables);
            interfaces.append(&mut interface2);

            //---------Return built Link!--------------//
            (all_runnables, interfaces)
        }
    }
}
