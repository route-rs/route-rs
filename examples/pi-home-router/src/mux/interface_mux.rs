use route_rs_packets::EthernetFrame;
use route_rs_runtime::link::{
    primitive::{JoinLink, ProcessLink},
    Link, LinkBuilder, PacketStream, ProcessLinkBuilder,
};
use route_rs_runtime::processor::Processor;

/// InterfaceMux: Link that joins many interfaces, and applys a tag to each packet based on
/// what interface it originiated from.
pub struct InterfaceMux {
    in_streams: Option<Vec<PacketStream<EthernetFrame>>>,
}

impl InterfaceMux {
    pub fn new() -> Self {
        InterfaceMux{ in_streams: None }
    }
}

impl LinkBuilder<EthernetFrame, (EthernetFrame, usize)> for InterfaceMux {
    fn ingressors(self, ingressors: Vec<PacketStream<EthernetFrame>>) -> InterfaceMux {
        assert!(ingressors.len() >= 1, "Requires at least one ingress stream");
        if self.in_streams.is_some() {
            panic!("Interface Mux: Double call of ingressors function");
        }

        InterfaceMux{ in_streams: Some(ingressors) }
    }

    fn ingressor(self, ingressor: PacketStream<EthernetFrame>) -> InterfaceMux {
        match self.in_streams {
            Some(mut streams) => {
                streams.push(ingressor);
                InterfaceMux {
                    in_streams: Some(streams)
                }
            },
            None => {
                InterfaceMux {
                    in_streams: Some(vec![ingressor])
                }
            }
        }
    }

    fn build_link(self) -> Link<(EthernetFrame, usize)> {
        let mut tagger_streams = vec![];

        for (i, stream) in self.in_streams.unwrap().into_iter().enumerate() {
            let tagger = InterfaceTagger::new(i);
            let (_,mut tag_streams) = ProcessLink::new()
                .processor(tagger)
                .ingressor(stream)
                .build_link();
            tagger_streams.append(&mut tag_streams);
        }

        // Join link has the only tokio runnables here, can just return it
        JoinLink::new()
            .ingressors(tagger_streams)
            .build_link()
    }
}

/// InterfaceTagger: Processor to apply an interface tag to a packet
///
/// Generally, it is important for the router to maintain some kind of state as to which interface
/// the packet arrived from, so that it may be routed appropriately.
///
/// If we require more annotations in the future, we may decide to place a hashmap in the heap for each
/// packet.
pub struct InterfaceTagger {
    tag: usize,
}

impl InterfaceTagger {
    pub fn new(tag: usize) -> Self {
        InterfaceTagger{ tag }
    }
}

impl Processor for InterfaceTagger {
    type Input = EthernetFrame;
    type Output = (EthernetFrame, usize);

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        Some((packet, self.tag))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn InterfaceMux() {
        unimplemnted!();
    }
}
