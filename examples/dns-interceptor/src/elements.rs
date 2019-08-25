use crate::packets::*;
use route_rs_runtime::element::{ClassifyElement, Element};
use std::collections::HashMap;

pub struct SetInterfaceByDestination {
    lan_subnet_prefix: u32,
    lan_subnet_mask: u32,
}

impl SetInterfaceByDestination {
    pub fn new(lan_subnet_prefix: u32, lan_subnet_mask: u32) -> Self {
        SetInterfaceByDestination {
            lan_subnet_prefix,
            lan_subnet_mask,
        }
    }
}

impl Element for SetInterfaceByDestination {
    type Input = (Interface, SimplePacket);
    type Output = (Interface, SimplePacket);

    fn process(&mut self, packet: Self::Input) -> Self::Output {
        let dest_ip = packet.1.destination.ip;
        if (dest_ip & self.lan_subnet_mask) == self.lan_subnet_prefix {
            (Interface::LAN, packet.1)
        } else {
            (Interface::WAN, packet.1)
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum ClassifyDNSOutput {
    DNS,
    Other,
}

pub struct ClassifyDNS {}

impl ClassifyDNS {
    pub fn new() -> Self {
        ClassifyDNS {}
    }
}

impl Element for ClassifyDNS {
    type Input = (Interface, SimplePacket);
    type Output = (ClassifyDNSOutput, Self::Input);

    fn process(&mut self, packet: Self::Input) -> Self::Output {
        match packet.1.destination.port {
            53 => (ClassifyDNSOutput::DNS, packet),
            _ => (ClassifyDNSOutput::Other, packet),
        }
    }
}

impl ClassifyElement for ClassifyDNS {
    type Class = ClassifyDNSOutput;
    type ActualOutput = Self::Input;
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

impl Element for LocalDNSInterceptor {
    type Input = (Interface, SimplePacket);
    type Output = (Interface, SimplePacket);

    fn process(&mut self, packet: Self::Input) -> Self::Output {
        match (
            &packet.0,
            self.intercept_rules.get(&packet.1.payload.to_string()),
        ) {
            (Interface::WAN, Some(address)) => (
                Interface::LAN,
                SimplePacket {
                    source: packet.1.destination,
                    destination: packet.1.source,
                    payload: address.to_string(),
                },
            ),
            _ => packet,
        }
    }
}
