use crate::types::{Interface, InterfaceAnnotated};
use route_rs_packets::EthernetFrame;
use route_rs_runtime::link::{
    primitive::{ProcessLink, ClassifyLink},
    Link, LinkBuilder, PacketStream, ProcessLinkBuilder,
};
use route_rs_runtime::processor::Processor;
use route_rs_runtime::classifier::Classifier;

/// InterfaceDemux
///
/// This link is used to convert packets that have been annotated with an
/// outbound interface into 3 streams, each one destined for their respective
/// interface (host, lan, wan)
pub(crate) struct InterfaceDemux {
   in_stream: Option<PacketStream<InterfaceAnnotated<EthernetFrame>>>
}

impl InterfaceDemux {
    pub fn new() -> Self {
       InterfaceDemux { in_stream: None }
    }
}

impl LinkBuilder<InterfaceAnnotated<EthernetFrame>, EthernetFrame> for InterfaceDemux {
    fn ingressors(self, mut in_streams: Vec<PacketStream<InterfaceAnnotated<EthernetFrame>>>) -> InterfaceDemux {
        assert!(in_streams.len() == 1, "InterfaceDemux only takes one input stream");

        if self.in_stream.is_some() {
            panic!("Double call of ingressors of InterfaceDeumx");
        }

        let stream = in_streams.remove(0);
        InterfaceDemux { in_stream: Some(stream) }
    }

    fn ingressor(self, in_stream: PacketStream<InterfaceAnnotated<EthernetFrame>>) -> InterfaceDemux {
        if self.in_stream.is_some() {
            panic!("Double call of ingressors of InterfaceDeumx");
        }

        InterfaceDemux { in_stream: Some(in_stream) }
    }

    fn build_link(self) -> Link<EthernetFrame> {
        let (runnables, mut ce) = ClassifyLink::new()
            .ingressor(self.in_stream.unwrap())
            .classifier(OutboundInterface)
            .num_egressors(3)
            .dispatcher(Box::new( |interface| match interface {
                Interface::Host => Some(0),
                Interface::Lan => Some(1),
                Interface::Wan => Some(2),
                Interface::Unmarked => None,
            }))
            .build_link();

        let (_, host_e) = ProcessLink::new()
            .ingressor(ce.remove(0))
            .processor(InterfaceAnnotationStripper)
            .build_link();

        let (_,mut lan_e) = ProcessLink::new()
            .ingressor(ce.remove(0))
            .processor(InterfaceAnnotationStripper)
            .build_link();
      
        let (_,mut wan_e) = ProcessLink::new()
            .ingressor(ce.remove(0))
            .processor(InterfaceAnnotationStripper)
            .build_link();

        let mut egressors = host_e;
        egressors.append(&mut lan_e);
        egressors.append(&mut wan_e);

        (runnables, egressors)
    }
}

#[derive(Default)]
pub(crate) struct OutboundInterface;

impl Classifier for OutboundInterface {
    type Packet = InterfaceAnnotated<EthernetFrame>;
    type Class = Interface;

    fn classify(&self, packet: &Self::Packet) -> Self::Class {
        match packet.outbound_interface {
            Interface::Host => Interface::Host,
            Interface::Lan => Interface::Lan,
            Interface::Wan => Interface::Wan,
            Interface::Unmarked => Interface::Unmarked,
        }
    }
}

#[derive(Default)]
pub(crate) struct InterfaceAnnotationStripper;

impl Processor for InterfaceAnnotationStripper {
    type Input = InterfaceAnnotated<EthernetFrame>;
    type Output = EthernetFrame;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        Some(packet.packet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use route_rs_runtime::utils::test::harness::{initialize_runtime, run_link};
    use route_rs_runtime::utils::test::packet_generators::immediate_stream;

    #[test]
    fn interface_demux() {
        let for_host = vec![
            InterfaceAnnotated{
                packet : EthernetFrame::empty(),
                inbound_interface: Interface::Unmarked,
                outbound_interface: Interface::Host,
            };
            3
        ];
        let mut for_lan = vec![
            InterfaceAnnotated{
                packet : EthernetFrame::empty(),
                inbound_interface: Interface::Unmarked,
                outbound_interface: Interface::Lan,
            };
            3
        ];
        let mut for_wan = vec![
            InterfaceAnnotated{
                packet : EthernetFrame::empty(),
                inbound_interface: Interface::Unmarked,
                outbound_interface: Interface::Wan,
            };
            3
        ];
        let mut unmarked = vec![
            InterfaceAnnotated{
                packet : EthernetFrame::empty(),
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
            let link = InterfaceDemux::new()
                .ingressor(immediate_stream(packets))
                .build_link();

            run_link(link).await
        });

        let host = &results[0];
        let lan = &results[1];
        let wan = &results[2];

        assert!(host.len() == 3, "Incorrect number of host packets");
        assert!(lan.len() == 3, "Incorrenct number of lan packts");
        assert!(wan.len() == 3, "Incorrect number of wan packets");
    }
}
