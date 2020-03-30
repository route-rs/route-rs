use crate::interface::link::InterfaceDispatch;
use crate::interface::processor::EthernetFrameToVec;
use crate::types::InterfaceAnnotated;
use route_rs_packets::EthernetFrame;
use route_rs_runtime::link::primitive::{JoinLink, ProcessLink};
use route_rs_runtime::link::ProcessLinkBuilder;
use route_rs_runtime::link::{Link, LinkBuilder, PacketStream};
use route_rs_runtime::unpack_link;

/// RouterExhaust is a link that takes any number of input streams of
/// InterfaceAnnotated<EthernetFrame>s, and splits them into 3 outbound raw
/// packet streams of Vec<u8>. The outbound streams should flow straight into
/// the outbound interface link. The streams are in Host, LAN, WAN order.
///
/// Outbound:
/// Port 0: Host
/// Port 1: LAN
/// Port 2: WAN
pub(crate) struct RouterExhaust {
    in_streams: Option<Vec<PacketStream<InterfaceAnnotated<EthernetFrame>>>>,
}

impl RouterExhaust {
    #[allow(dead_code)]
    pub(crate) fn new() -> Self {
        RouterExhaust { in_streams: None }
    }
}

impl LinkBuilder<InterfaceAnnotated<EthernetFrame>, Vec<u8>> for RouterExhaust {
    fn ingressors(
        mut self,
        ingressors: Vec<PacketStream<InterfaceAnnotated<EthernetFrame>>>,
    ) -> Self {
        assert!(!ingressors.is_empty(), "Ingressor vector is empty");
        assert!(
            self.in_streams.is_none(),
            "RouterExhaust already has input_streams"
        );
        self.in_streams = Some(ingressors);
        self
    }

    fn ingressor(mut self, ingressor: PacketStream<InterfaceAnnotated<EthernetFrame>>) -> Self {
        if self.in_streams.is_none() {
            self.in_streams = Some(vec![ingressor]);
        } else {
            let mut streams = self.in_streams.unwrap();
            streams.push(ingressor);
            self.in_streams = Some(streams);
        }
        self
    }

    fn build_link(self) -> Link<Vec<u8>> {
        if self.in_streams.is_none() {
            panic!("Input Streams were not provided")
        }

        let mut all_runnables = vec![];

        //---Join Inputs links---//
        let join_inputs = JoinLink::new()
            .ingressors(self.in_streams.unwrap())
            .build_link();
        unpack_link!(join_inputs => all_runnables, join_egressors);

        //---Sort to Interface---//
        let sort_to_interface = InterfaceDispatch::new()
            .ingressors(join_egressors)
            .build_link();
        unpack_link!(sort_to_interface => all_runnables, [host_d, lan_d, wan_d]);

        //---Create Raw streams---//
        let host_to_bytes = ProcessLink::new()
            .ingressor(host_d)
            .processor(EthernetFrameToVec)
            .build_link();
        unpack_link!(host_to_bytes => all_runnables, [host_egressor]);

        let lan_to_bytes = ProcessLink::new()
            .ingressor(lan_d)
            .processor(EthernetFrameToVec)
            .build_link();
        unpack_link!(lan_to_bytes => all_runnables, [lan_egressor]);

        let wan_to_bytes = ProcessLink::new()
            .ingressor(wan_d)
            .processor(EthernetFrameToVec)
            .build_link();
        unpack_link!(wan_to_bytes => all_runnables, [wan_egressor]);

        (
            all_runnables,
            vec![host_egressor, lan_egressor, wan_egressor],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Interface;
    use route_rs_runtime::utils::test::harness::{initialize_runtime, test_link};
    use route_rs_runtime::utils::test::packet_generators::immediate_stream;

    #[test]
    fn router_exhaust() {
        let for_host = vec![
            InterfaceAnnotated {
                packet: EthernetFrame::empty(),
                inbound_interface: Interface::Unmarked,
                outbound_interface: Interface::Host,
            };
            3
        ];
        let mut for_lan = vec![
            InterfaceAnnotated {
                packet: EthernetFrame::empty(),
                inbound_interface: Interface::Unmarked,
                outbound_interface: Interface::Lan,
            };
            3
        ];
        let mut for_wan = vec![
            InterfaceAnnotated {
                packet: EthernetFrame::empty(),
                inbound_interface: Interface::Unmarked,
                outbound_interface: Interface::Wan,
            };
            3
        ];
        let mut unmarked = vec![
            InterfaceAnnotated {
                packet: EthernetFrame::empty(),
                inbound_interface: Interface::Unmarked,
                outbound_interface: Interface::Unmarked,
            };
            3
        ];
        let mut packets = for_host;
        packets.append(&mut for_lan);
        packets.append(&mut for_wan);
        packets.append(&mut unmarked);

        let mut runtime = initialize_runtime();
        let results = runtime.block_on(async {
            let link = RouterExhaust::new()
                .ingressor(immediate_stream(packets.clone()))
                .ingressor(immediate_stream(packets.clone()))
                .ingressor(immediate_stream(packets))
                .build_link();

            test_link(link, None).await
        });

        let host = &results[0];
        let lan = &results[1];
        let wan = &results[2];

        assert!(host.len() == 9, "Incorrect number of host packets");
        assert!(lan.len() == 9, "Incorrenct number of lan packts");
        assert!(wan.len() == 9, "Incorrect number of wan packets");
    }
}
