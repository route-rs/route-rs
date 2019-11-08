use crate::packets::*;
use route_rs_runtime::classifier::Classifier;
use route_rs_runtime::processor::Processor;
use std::collections::HashMap;

pub struct SetInterfaceByDestination {
    lan_subnet_prefix: u32,
    lan_subnet_mask: u32,
}

impl SetInterfaceByDestination {
    pub fn new() -> Self {
        let lan_subnet_prefix = u32::from_be_bytes([10, 0, 0, 1]);
        let lan_subnet_mask = 0xFF00_0000;
        SetInterfaceByDestination {
            lan_subnet_prefix,
            lan_subnet_mask,
        }
    }
}

// NOTE: Should SetInterfaceByDestination be a WAN/LAN Classifier instead of an Processor?
impl Processor for SetInterfaceByDestination {
    type Input = (Interface, SimplePacket);
    type Output = (Interface, SimplePacket);

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        let dest_ip = packet.1.destination.ip;
        if (dest_ip & self.lan_subnet_mask) == self.lan_subnet_prefix {
            Some((Interface::LAN, packet.1))
        } else {
            Some((Interface::WAN, packet.1))
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum ClassifyDNSOutput {
    DNS,
    Other,
}

//TODO: This should be a Unit struct, then you do not have to call New.
pub struct ClassifyDNS {}

impl ClassifyDNS {
    pub fn new() -> Self {
        ClassifyDNS {}
    }
}

impl Classifier for ClassifyDNS {
    type Packet = (Interface, SimplePacket);
    type Class = ClassifyDNSOutput;

    fn classify(&self, packet: &Self::Packet) -> Self::Class {
        match packet.1.destination.port {
            53 => ClassifyDNSOutput::DNS,
            _ => ClassifyDNSOutput::Other,
        }
    }
}

pub struct LocalDNSInterceptor {
    intercept_rules: HashMap<String, String>,
}

impl LocalDNSInterceptor {
    pub fn new() -> Self {
        let intercept_rules: HashMap<String, String> =
            [("gateway.route-rs.local".to_string(), "10.0.0.1".to_string())]
                .iter()
                .cloned()
                .collect();
        LocalDNSInterceptor { intercept_rules }
    }
}

impl Processor for LocalDNSInterceptor {
    type Input = (Interface, SimplePacket);
    type Output = (Interface, SimplePacket);

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        let (in_interface, in_packet) = packet;
        let maybe_lan_address = self.intercept_rules.get(&in_packet.payload.to_string());

        let (out_interface, out_packet) = match (&in_interface, maybe_lan_address) {
            (Interface::WAN, Some(lan_address)) => (
                Interface::LAN,
                SimplePacket {
                    source: in_packet.destination,
                    destination: in_packet.source,
                    payload: lan_address.to_string(),
                },
            ),
            _ => (in_interface, in_packet),
        };
        Some((out_interface, out_packet))
    }
}
