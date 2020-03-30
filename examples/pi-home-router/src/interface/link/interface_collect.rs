use crate::interface::processor::InterfaceAnnotationEncap;
use crate::types::{Interface, InterfaceAnnotated};
use route_rs_packets::EthernetFrame;
use route_rs_runtime::link::{
    primitive::{JoinLink, ProcessLink},
    Link, LinkBuilder, PacketStream, ProcessLinkBuilder,
};
use route_rs_runtime::unpack_link;

/// InterfaceCollect: Link that our 3 input interfaces, Host, Lan and Wan, expected in that order.
pub(crate) struct InterfaceCollect {
    in_streams: Option<Vec<PacketStream<EthernetFrame>>>,
}

impl InterfaceCollect {
    #[allow(dead_code)]
    pub fn new() -> Self {
        InterfaceCollect { in_streams: None }
    }
}

impl LinkBuilder<EthernetFrame, InterfaceAnnotated<EthernetFrame>> for InterfaceCollect {
    fn ingressors(mut self, ingressors: Vec<PacketStream<EthernetFrame>>) -> Self {
        assert!(
            ingressors.len() == 3,
            "Link only supports 3 interfaces [Host: 0, Lan: 1, Wan: 2]"
        );
        if self.in_streams.is_some() {
            panic!("Interface Mux: Double call of ingressors function");
        }

        self.in_streams = Some(ingressors);
        self
    }

    fn ingressor(mut self, ingressor: PacketStream<EthernetFrame>) -> Self {
        match self.in_streams {
            Some(mut streams) => {
                assert!(streams.len() < 3, "Trying to add too many streams");
                streams.push(ingressor);
                self.in_streams = Some(streams);
            }
            None => self.in_streams = Some(vec![ingressor]),
        }
        self
    }

    fn build_link(self) -> Link<InterfaceAnnotated<EthernetFrame>> {
        let mut streams = self.in_streams.unwrap();

        let annotate_host = ProcessLink::new()
            .processor(InterfaceAnnotationEncap::new(
                Interface::Host,
                Interface::Unmarked,
            ))
            .ingressor(streams.remove(0))
            .build_link();
        unpack_link!(annotate_host => _, [host]);

        let annotate_lan = ProcessLink::new()
            .processor(InterfaceAnnotationEncap::new(
                Interface::Lan,
                Interface::Unmarked,
            ))
            .ingressor(streams.remove(1))
            .build_link();
        unpack_link!(annotate_lan => _, [lan]);

        let annotate_wan = ProcessLink::new()
            .processor(InterfaceAnnotationEncap::new(
                Interface::Wan,
                Interface::Unmarked,
            ))
            .ingressor(streams.remove(0))
            .build_link();
        unpack_link!(annotate_wan => _, [wan]);

        // Join link has the only tokio runnables here, can just return it
        JoinLink::new()
            .ingressors(vec![host, lan, wan])
            .build_link()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use route_rs_runtime::utils::test::harness::{initialize_runtime, test_link};
    use route_rs_runtime::utils::test::packet_generators::immediate_stream;

    #[test]
    fn interface_join() {
        let packets = vec![EthernetFrame::empty(); 3];
        let host = immediate_stream(packets.clone());
        let lan = immediate_stream(packets.clone());
        let wan = immediate_stream(packets);
        let streams = vec![host, lan, wan];

        let mut runtime = initialize_runtime();
        let results = runtime.block_on(async {
            let link = InterfaceCollect::new().ingressors(streams).build_link();

            test_link(link, None).await
        });

        let output = &results[0];
        let (mut host, mut lan, mut wan, mut unmarked) = (0, 0, 0, 0);
        for packet in output {
            match packet.inbound_interface {
                Interface::Host => host += 1,
                Interface::Lan => lan += 1,
                Interface::Wan => wan += 1,
                Interface::Unmarked => unmarked += 1,
            }
        }

        assert!(host == 3, "Incorrect number of host packets");
        assert!(lan == 3, "Incorrenct number of lan packts");
        assert!(wan == 3, "Incorrect number of wan packets");
        assert!(unmarked == 0, "Incorrect number of unmarked packets");
    }
}
