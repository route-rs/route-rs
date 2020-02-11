use crate::types::{Interface, InterfaceAnnotated};
use route_rs_packets::EthernetFrame;
use route_rs_runtime::link::{
    primitive::{JoinLink, ProcessLink},
    Link, LinkBuilder, PacketStream, ProcessLinkBuilder,
};
use route_rs_runtime::processor::Processor;

/// InterfaceMux: Link that our 3 input interfaces, Host, Lan and Wan, expected in that order.
pub(crate) struct InterfaceMux {
    in_streams: Option<Vec<PacketStream<EthernetFrame>>>,
}

impl InterfaceMux {
    pub fn new() -> Self {
        InterfaceMux { in_streams: None }
    }
}

impl LinkBuilder<EthernetFrame, InterfaceAnnotated<EthernetFrame>> for InterfaceMux {
    fn ingressors(self, ingressors: Vec<PacketStream<EthernetFrame>>) -> InterfaceMux {
        assert!(
            ingressors.len() == 3,
            "Link only supports 3 interfaces [Host: 0, Lan: 1, Wan: 2]"
        );
        if self.in_streams.is_some() {
            panic!("Interface Mux: Double call of ingressors function");
        }

        InterfaceMux {
            in_streams: Some(ingressors),
        }
    }

    fn ingressor(self, ingressor: PacketStream<EthernetFrame>) -> InterfaceMux {
        match self.in_streams {
            Some(mut streams) => {
                assert!(streams.len() < 3, "Trying to add too many streams");
                streams.push(ingressor);
                InterfaceMux {
                    in_streams: Some(streams),
                }
            }
            None => InterfaceMux {
                in_streams: Some(vec![ingressor]),
            },
        }
    }

    fn build_link(self) -> Link<InterfaceAnnotated<EthernetFrame>> {
        let mut tagger_streams = vec![];
        let mut streams = self.in_streams.unwrap();

        let tagger = InterfaceTagger::new(Interface::Host);
        let (_, mut annotated_stream) = ProcessLink::new()
            .processor(tagger)
            .ingressor(streams.remove(0))
            .build_link();
        tagger_streams.append(&mut annotated_stream);

        let tagger = InterfaceTagger::new(Interface::Lan);
        let (_, mut annotated_stream) = ProcessLink::new()
            .processor(tagger)
            .ingressor(streams.remove(1))
            .build_link();
        tagger_streams.append(&mut annotated_stream);

        let tagger = InterfaceTagger::new(Interface::Wan);
        let (_, mut annotated_stream) = ProcessLink::new()
            .processor(tagger)
            .ingressor(streams.remove(0))
            .build_link();
        tagger_streams.append(&mut annotated_stream);

        // Join link has the only tokio runnables here, can just return it
        JoinLink::new().ingressors(tagger_streams).build_link()
    }
}

/// InterfaceTagger: Processor to apply an interface tag to a packet
///
/// Generally, it is important for the router to maintain some kind of state as to which interface
/// the packet arrived from, so that it may be routed appropriately.
///
/// If we require more annotations in the future, we may decide to place a hashmap in the heap for each
/// packet.
pub(crate) struct InterfaceTagger {
    tag: Interface,
}

impl InterfaceTagger {
    pub(crate) fn new(tag: Interface) -> Self {
        InterfaceTagger { tag }
    }
}

impl Processor for InterfaceTagger {
    type Input = EthernetFrame;
    type Output = InterfaceAnnotated<EthernetFrame>;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        Some(InterfaceAnnotated {
            packet: packet,
            inbound_interface: self.tag,
            outbound_interface: Interface::Unmarked,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use route_rs_runtime::utils::test::harness::{initialize_runtime, run_link};
    use route_rs_runtime::utils::test::packet_generators::immediate_stream;

    #[test]
    fn InterfaceMux() {
        let packets = vec![EthernetFrame::empty(); 3];
        let host = immediate_stream(packets.clone());
        let lan = immediate_stream(packets.clone());
        let wan = immediate_stream(packets.clone());
        let streams = vec![host, lan, wan];

        let mut runtime = initialize_runtime();
        let results = runtime.block_on(async {
            let link = InterfaceMux::new()
                .ingressors(streams)
                .build_link();

            run_link(link).await
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
