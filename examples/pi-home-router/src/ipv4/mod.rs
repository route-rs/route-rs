use crate::types::InterfaceAnnotated;
use route_rs_packets::Ipv4Packet;
use route_rs_runtime::link::{Link, LinkBuilder, PacketStream};

pub(crate) struct HandleIpv4 {
    in_stream: Option<PacketStream<InterfaceAnnotated<Ipv4Packet>>>,
}

#[allow(dead_code)]
impl HandleIpv4 {
    pub(crate) fn new() -> Self {
        HandleIpv4 { in_stream: None }
    }
}

impl LinkBuilder<InterfaceAnnotated<Ipv4Packet>, InterfaceAnnotated<Ipv4Packet>> for HandleIpv4 {
    fn ingressors(self, mut in_streams: Vec<PacketStream<InterfaceAnnotated<Ipv4Packet>>>) -> Self {
        if self.in_stream.is_some() {
            panic!("HandleIPv4 takes only one ingressor");
        }
        if in_streams.len() == 1 {
            HandleIpv4 {
                in_stream: Some(in_streams.remove(0)),
            }
        } else {
            panic!("HandleIPv4 takes exactly one ingressor");
        }
    }

    fn ingressor(self, in_stream: PacketStream<InterfaceAnnotated<Ipv4Packet>>) -> Self {
        match self.in_stream {
            None => HandleIpv4 {
                in_stream: Some(in_stream),
            },
            Some(_) => panic!("HandleIPv4 takes only one ingressor"),
        }
    }

    fn build_link(self) -> Link<InterfaceAnnotated<Ipv4Packet>> {
        assert!(
            self.in_stream.is_some(),
            "HandleIPv4 must have an ingressor defined"
        );

        (vec![], vec![self.in_stream.unwrap()])
    }
}
