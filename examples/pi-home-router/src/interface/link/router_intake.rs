use crate::interface::classifier::ByEtherType;
use crate::interface::link::InterfaceCollect;
use crate::interface::processor::VecToEthernetFrame;
use crate::types::{EtherType, InterfaceAnnotated};
use route_rs_packets::EthernetFrame;
use route_rs_runtime::link::primitive::{ClassifyLink, ProcessLink};
use route_rs_runtime::link::ProcessLinkBuilder;
use route_rs_runtime::link::{Link, LinkBuilder, PacketStream};

/// RouterIntake is a link that take in 3 streams from the interfaces of the PHR, combines them into
/// a stream of packets. That stream is then divided into 3 output streams, by IP type. RouterIntake
/// takes in raw Vec<u8> packets, and outputs InterfaceAnnotated<EthernetFrame> packets. Any packets
/// that are not valid EthernetFrames, or are not of a supported EtherType (IPv4, IPv6, ARP) are
/// dropped. Input streams should be provided in Host, LAN, WAN order.
///
/// Inputs:
/// Port 0: Host
/// Port 1: LAN
/// Port 2: WAN
///
/// Outputs:
/// Port 0: ARP
/// Port 1: IPv4
/// Port 2: IPv6
pub(crate) struct RouterIntake {
    in_streams: Option<Vec<PacketStream<Vec<u8>>>>,
}

impl RouterIntake {
    #[allow(dead_code)]
    pub(crate) fn new() -> Self {
        RouterIntake { in_streams: None }
    }
}

impl LinkBuilder<Vec<u8>, InterfaceAnnotated<EthernetFrame>> for RouterIntake {
    fn ingressors(self, ingressors: Vec<PacketStream<Vec<u8>>>) -> RouterIntake {
        if self.in_streams.is_some() {
            panic!("RouterIntake: Double call of ingressors");
        }
        if ingressors.len() != 3 {
            panic!("RouterIntake requires 3 input streams, Host LAN WAN, in that order");
        }
        RouterIntake {
            in_streams: Some(ingressors),
        }
    }

    fn ingressor(self, ingressor: PacketStream<Vec<u8>>) -> RouterIntake {
        match self.in_streams {
            Some(mut streams) => {
                if streams.len() >= 3 {
                    panic!("RouterIntake requires 3 input streams, Host LAN WAN, in that order");
                }
                streams.push(ingressor);
                RouterIntake {
                    in_streams: Some(streams),
                }
            }
            None => RouterIntake {
                in_streams: Some(vec![ingressor]),
            },
        }
    }

    fn build_link(self) -> Link<InterfaceAnnotated<EthernetFrame>> {
        match &self.in_streams {
            Some(streams) => {
                if streams.len() != 3 {
                    panic!("RouterIntake requires 3 input streams, Host LAN WAN, in that order");
                }
            }
            None => {
                panic!("RouterIntake requires 3 input streams, Host LAN WAN, in that order");
            }
        };

        let mut streams = self.in_streams.unwrap();

        let mut all_runnables = vec![];
        let mut interfaces = vec![];

        let (mut host_runnables, mut host_egressors) = ProcessLink::new()
            .ingressor(streams.remove(0))
            .processor(VecToEthernetFrame)
            .build_link();
        all_runnables.append(&mut host_runnables);
        interfaces.append(&mut host_egressors);

        let (mut lan_runnables, mut lan_egressors) = ProcessLink::new()
            .ingressor(streams.remove(0))
            .processor(VecToEthernetFrame)
            .build_link();
        all_runnables.append(&mut lan_runnables);
        interfaces.append(&mut lan_egressors);

        let (mut wan_runnables, mut wan_egressors) = ProcessLink::new()
            .ingressor(streams.remove(0))
            .processor(VecToEthernetFrame)
            .build_link();
        all_runnables.append(&mut wan_runnables);
        interfaces.append(&mut wan_egressors);

        //---Collect from Interfaces---//
        let (mut collect_runnables, collect_egressors) =
            InterfaceCollect::new().ingressors(interfaces).build_link();
        all_runnables.append(&mut collect_runnables);

        //---Sort into streams by EtherType---//
        let (mut sort_runnables, sort_egressors) = ClassifyLink::new()
            .ingressors(collect_egressors)
            .num_egressors(3)
            .classifier(ByEtherType {})
            .dispatcher(Box::new(|class| match class {
                EtherType::ARP => Some(0),
                EtherType::IPv4 => Some(1),
                EtherType::IPv6 => Some(2),
                EtherType::Unsupported => None,
            }))
            .build_link();
        all_runnables.append(&mut sort_runnables);

        (all_runnables, sort_egressors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use route_rs_runtime::utils::test::harness::{initialize_runtime, test_link};
    use route_rs_runtime::utils::test::packet_generators::immediate_stream;

    #[test]
    fn router_intake() {
        let empty_frame = EthernetFrame::empty();
        let mut arp_frame = empty_frame.clone();
        arp_frame.set_ether_type(0x0806);
        let mut ipv4_frame = empty_frame.clone();
        ipv4_frame.set_ether_type(0x0800);
        let mut ipv6_frame = empty_frame.clone();
        ipv6_frame.set_ether_type(0x86DD);
        let mut unsupported_frame = empty_frame;
        unsupported_frame.set_ether_type(0x1337);

        let packets = vec![
            arp_frame.data,
            ipv4_frame.data,
            ipv6_frame.clone().data,
            unsupported_frame.data,
            ipv6_frame.data,
        ];

        let host = immediate_stream(packets.clone());
        let lan = immediate_stream(packets.clone());
        let wan = immediate_stream(packets);
        let streams = vec![host, lan, wan];

        let mut runtime = initialize_runtime();
        let results = runtime.block_on(async {
            let link = RouterIntake::new().ingressors(streams).build_link();

            test_link(link, None).await
        });

        assert!(results[0].len() == 3, "Incorrect number of ARP packets");
        assert!(results[1].len() == 3, "Incorrenct number of Ipv4 packts");
        assert!(results[2].len() == 6, "Incorrect number of Ipv6 packets");
    }
}
