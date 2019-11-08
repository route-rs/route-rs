use route_rs_packets::{EthernetFrame, Ipv6Packet, Ipv4Packet};
use route_rs_runtime::processor::Processor;
use route_rs_runtime::classifier::Classifier;
use std::convert::TryFrom;

pub struct Ipv6Encap;

impl Processor for Ipv6Encap {
    type Input = EthernetFrame;
    type Output = Ipv6Packet;

    fn process(&mut self, frame: Self::Input) -> Option<Self::Output> {
        match Ipv6Packet::try_from(frame) {
            Ok(packet) => Some(packet),
            Err(_) => None,
        }
    }
}

pub struct Ipv6Decap;

impl Processor for Ipv6Decap {
    type Input = Ipv6Packet;
    type Output = EthernetFrame;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        match EthernetFrame::try_from(packet) {
            Ok(frame) => Some(frame),
            Err(_) => None,
        }
    }
}

pub struct Ipv4Encap;

impl Processor for Ipv4Encap {
    type Input = EthernetFrame;
    type Output = Ipv4Packet;
    
    fn process(&mut self, frame: Self::Input) -> Option<Self::Output> {
        match Ipv4Packet::try_from(frame) {
            Ok(packet) => Some(packet),
            Err(_) => None,
        }
    }
}

pub struct Ipv4Decap;

impl Processor for Ipv4Decap {
    type Input = Ipv4Packet;
    type Output = EthernetFrame;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        match EthernetFrame::try_from(packet) {
            Ok(frame) => Some(frame),
            Err(_) => None,
        }
    }
}

pub enum ClassifyIPType {
    IPv6,
    IPv4,
    None,
}

/// Processor that determines whether an IP packet is IPv6 or Ipv4.
pub struct ClassifyIP;

// In this case, Processors take Ownership, Classifiers take references, in a way that makes sense.
impl Classifier for ClassifyIP {
    type Packet = EthernetFrame;
    type Class = ClassifyIPType;

    fn classify(&self, frame: &Self::Packet) -> Self::Class {
        // We have to check the etherType
        // https://en.wikipedia.org/wiki/EtherType
        let ether_type = frame.ether_type();
        if ether_type == 0x0800 {
            ClassifyIPType::IPv4
        } else if ether_type == 0x86DD {
            ClassifyIPType::IPv6
        } else {
            ClassifyIPType::None
        }
    }
}



