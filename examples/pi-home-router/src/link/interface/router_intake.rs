use crate::types::{InterfaceAnnotated, EtherType};
use crate::link::interface::interface_collect::InterfaceCollect;
use crate::classifier::ethertype::ClassifyByEtherType;
use route_rs_runtime::link::{LinkBuilder, PacketStream, Link};
use route_rs_runtime::link::primitive::ClassifyLink;
use route_rs_packets::EthernetFrame;
use afpacket::RecvHalf;

/// RouterIntake is a link that takes no inputs and outputs 3 streams sorted by EtherType.
/// Intake Manifold handles the creation of AFPacket links to generate packets from the 3 input interfaces
/// (Host, LAN, WAN) and then collects and sorts them into 3 distinct output streams based.
///
/// Outputs: 3 streams of same packet type
/// Port 0: ARP
/// Port 1: IPv4
/// Port 2: IPv6
pub(crate) struct RouterIntake {
    host: Option<RecvHalf>,
    lan: Option<RecvHalf>,
    wan: Option<RecvHalf>,
}

impl RouterIntake {
    pub(crate) fn new() -> Self {
        RouterIntake {
            host: None,
            lan: None,
            wan: None,
        }
    }

    pub(crate) fn host(self, host: RecvHalf) -> Self {
        RouterIntake {
            host: Some(RecvHalf),
            lan: self.lan,
            wan: self.wan,
        }
    }

    pub(crate) fn lan(self, lan: &str) -> Self {
        RouterIntake {
            host: self.host,
            lan: Some(RecvHalf),
            wan: self.wan,
        }
    }

    pub(crate) fn wan(self, wan: &str) -> Self {
        RouterIntake {
            host: self.host,
            lan: self.lan,
            wan: Some(RecvHalf),
        }
    }
}

impl LinkBuilder<(), InterfaceAnnotated<EthernetFrame>> for RouterIntake {
    fn ingressors(self, ingressors: Vec<PacketStream<()>>) -> RouterIntake {
        panic!("RouterIntakeManifold takes no inputs");
    }

    fn ingressor(self, ingressor: PacketStream<()>) -> RouterIntake {
        panic!("RouterIntakeManifold takes no inputs");
    }

    fn build_link(self) -> Link<InterfaceAnnotated<EthernetFrame>>  {
        if self.host.is_none() {
            panic!("Host interface was not provided")
        } else if self.lan.is_none() {
            panic!("LAN interface was not provided")
        } else if self.wan.is_none() {
            panic!("WAN Interface was not provided")
        }

        let all_runnables = vec![];
        let interfaces = vec![];

        //---Declare interface links---//
        let (mut host_runnables, mut host_egressors) = AfPacketInputLink::new()
            .channel(self.host)
            .build_link();
        all_runnables.append(&mut host_runnables);
        interfaces.append(&mut host_egressors);

        let (mut lan_runnables, mut lan_egressors) = AfPacketInputLink::new()
            .channel(self.lan)
            .build_link();
        all_runnables.append(&mut lan_runnables);
        interfaces.append(&mut lan_egressors);

        let (mut wan_runnables, mut wan_egressors) = AfPacketInputLink::new()
            .channel(self.wan)
            .build_link();
        all_runnables.append(&mut wan_runnables);
        interfaces.append(&mut wan_egressors);

        //---Collect from Interfaces---//
        let (mut collect_runnables, collect_egressors) = InterfaceCollect::new()
            .ingressors(interfaces)
            .build_link();
        all_runnables.append(&mut collect_runnables);

        //---Sort into streams by EtherType---//
        let (mut sort_runnables, sort_egressors) = ClassifyLink::new()
            .ingressors(collect_egressors)
            .num_egressors(3)
            .classifier(ClassifyByEtherType)
            .dispatcher(|class| match class {
                EtherType::ARP => Some(0),
                EtherType::IPv4 => Some(1),
                EtherType::IPv6 => Some(2),
                EtherType::Unsupported => None,
            })
            .build_link();
        all_runnables.append(&mut sort_runnables);

        (all_runnables, sort_egressors)
    }
}
