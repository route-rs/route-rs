use crate::types::InterfaceAnnotated;
use crate::link::interface::interface_dispatch::InterfaceDispatch;
use route_rs_runtime::link::{LinkBuilder, PacketStream, Link};
use route_rs_runtime::link::primitive::JoinLink;
use route_rs_packets::EthernetFrame;
use afpacket::SendHalf;

/// RouterExhaust is a link that takes any number of input streams of
/// InterfaceAnnotated<EthernetFrame>
pub(crate) struct RouterExhaust {
    in_streams: Option<Vec<PacketStream<InterfaceAnnotated<EthernetFrame>>>>,
    host: Option<SendHalf>,
    lan: Option<SendHalf>,
    wan: Option<SendHalf>,
}

impl RouterExhaust {
    pub(crate) fn new() -> Self {
        RouterExhaust {
            in_streams: None,
            host: None,
            lan: None,
            wan: None,
        }
    }

    pub(crate) fn host(self, host: SendHalf) -> Self {
        RouterExhaust {
            in_streams: None,
            host: Some(SendHalf),
            lan: self.lan,
            wan: self.wan,
        }
    }

    pub(crate) fn lan(self, lan: &str) -> Self {
        RouterExhaust {
            in_streams: None,
            host: self.host,
            lan: Some(SendHalf),
            wan: self.wan,
        }
    }

    pub(crate) fn wan(self, wan: &str) -> Self {
        RouterExhaust {
            in_streams: None,
            host: self.host,
            lan: self.lan,
            wan: Some(SendHalf),
        }
    }
}

impl LinkBuilder<InterfaceAnnotated<EthernetFrame>, ()>  for RouterExhaust {
    fn ingressors(self, ingressors: Vec<PacketStream<InterfaceAnnotated<EthernetFrame>>>) -> RouterExhaust {
        assert!(!ingressors.is_empty(), "Ingressor vector is empty");
        assert!(self.in_streams.is_none(), "RouterExhaust already has input_streams");
        RouterExhaust {
            in_streams: Some(ingressors),
            host: self.host,
            lan: self.lan,
            wan: self.wan,
        }
    }


    fn ingressor(self, ingressor: PacketStream<InterfaceAnnotated<EthernetFrame>>) -> RouterExhaust {
        if self.in_streams.is_none() {
            return RouterExhaust {
                in_streams: Some(vec![ingressor]),
                host: self.host,
                lan: self.lan,
                wan: self.wan,
            }
        } else {
            self.in_streams.unwrap().push(ingressor);
            return RouterExhaust {
                in_streams: self.in_streams,
                host: self.host,
                lan: self.lan,
                wan: self.wan,
            }
        }
    }

    fn build_link(self) -> Link<InterfaceAnnotated<EthernetFrame>>  {
        if self.host.is_none() {
            panic!("Host interface was not provided")
        } else if self.lan.is_none() {
            panic!("LAN interface was not provided")
        } else if self.wan.is_none() {
            panic!("WAN Interface was not provided")
        } else if self.in_streams.is_none() {
            panic!("Input Streams were not provided")
        }

        let all_runnables = vec![];

        //---Join Inputs links---//
        let (mut join_runnables, join_egressors) = JoinLink::new()
            .ingressors(self.in_streams.unwrap())
            .build_link();
        all_runnables.append(&mut join_runnables);

        //---Sort to Interface---//
        let (mut dispatch_runnables, mut dispatch_egressors) = InterfaceDispatch::new()
            .ingressors(join_egressors)
            .build_link();
        all_runnables.append(&mut dispatch_runnables);

        //---Provide to Interfaces---//
        // Host, Lan, Wan
        let (mut host_runnables, _) = AfPacketOutputLink::new()
            .ingressor(dispatch_egressors.remove(0))
            .channel(self.host)
            .build_link;
        all_runnables.append(&mut host_runnables);

        let (mut lan_runnables, _) = AfPacketOutputLink::new()
            .ingressor(dispatch_egressors.remove(0))
            .channel(self.lan)
            .build_link;
        all_runnables.append(&mut lan_runnables);

        let (mut wan_runnables, _) = AfPacketOutputLink::new()
            .ingressor(dispatch_egressors.remove(0))
            .channel(self.wan)
            .build_link;
        all_runnables.append(&mut wan_runnables);

        (all_runnables, ())
    }
}
