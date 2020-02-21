use crate::types::{Interface, InterfaceAnnotated};
use crate::interface::processor::InterfaceAnnotationDecap;
use crate::interface::classifier::ByOutboundInterface;
use route_rs_packets::EthernetFrame;
use route_rs_runtime::link::{
    primitive::{ProcessLink, ClassifyLink},
    Link, LinkBuilder, PacketStream, ProcessLinkBuilder,
};

/// InterfaceDispatch
///
/// This link is used to convert packets that have been annotated with an
/// outbound interface into 3 streams, each one destined for their respective
/// interface (host, lan, wan)
pub(crate) struct InterfaceDispatch {
   in_stream: Option<PacketStream<InterfaceAnnotated<EthernetFrame>>>
}

impl InterfaceDispatch {
    #[allow(dead_code)]
    pub fn new() -> Self {
       InterfaceDispatch { in_stream: None }
    }
}

impl LinkBuilder<InterfaceAnnotated<EthernetFrame>, EthernetFrame> for InterfaceDispatch {
    fn ingressors(self, mut in_streams: Vec<PacketStream<InterfaceAnnotated<EthernetFrame>>>) -> InterfaceDispatch {
        assert!(in_streams.len() == 1, "InterfaceDispatch only takes one input stream");

        if self.in_stream.is_some() {
            panic!("Double call of ingressors of InterfaceDeumx");
        }

        let stream = in_streams.remove(0);
        InterfaceDispatch { in_stream: Some(stream) }
    }

    fn ingressor(self, in_stream: PacketStream<InterfaceAnnotated<EthernetFrame>>) -> InterfaceDispatch {
        if self.in_stream.is_some() {
            panic!("Double call of ingressors of InterfaceDeumx");
        }

        InterfaceDispatch { in_stream: Some(in_stream) }
    }

    fn build_link(self) -> Link<EthernetFrame> {
        let (runnables, mut ce) = ClassifyLink::new()
            .ingressor(self.in_stream.unwrap())
            .classifier(ByOutboundInterface)
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
            .processor(InterfaceAnnotationDecap)
            .build_link();

        let (_,mut lan_e) = ProcessLink::new()
            .ingressor(ce.remove(0))
            .processor(InterfaceAnnotationDecap)
            .build_link();
      
        let (_,mut wan_e) = ProcessLink::new()
            .ingressor(ce.remove(0))
            .processor(InterfaceAnnotationDecap)
            .build_link();

        let mut egressors = host_e;
        egressors.append(&mut lan_e);
        egressors.append(&mut wan_e);

        (runnables, egressors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use route_rs_runtime::utils::test::harness::{initialize_runtime, test_link};
    use route_rs_runtime::utils::test::packet_generators::immediate_stream;

    #[test]
    fn interface_dispatch() {
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
            let link = InterfaceDispatch::new()
                .ingressor(immediate_stream(packets))
                .build_link();

            test_link(link, None).await
        });

        let host = &results[0];
        let lan = &results[1];
        let wan = &results[2];

        assert!(host.len() == 3, "Incorrect number of host packets");
        assert!(lan.len() == 3, "Incorrenct number of lan packts");
        assert!(wan.len() == 3, "Incorrect number of wan packets");
    }
}
