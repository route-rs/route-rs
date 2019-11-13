use route_rs_packets::{EthernetFrame, Ipv4Packet, Ipv6Packet};
use route_rs_runtime::processor::Processor;
use std::convert::TryFrom;

pub struct Ipv6Decap;

impl Processor for Ipv6Decap {
    type Input = EthernetFrame;
    type Output = Ipv6Packet;

    fn process(&mut self, frame: Self::Input) -> Option<Self::Output> {
        match Ipv6Packet::try_from(frame) {
            Ok(packet) => Some(packet),
            Err(_) => None,
        }
    }
}

pub struct Ipv6Encap;

impl Processor for Ipv6Encap {
    type Input = Ipv6Packet;
    type Output = EthernetFrame;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        match EthernetFrame::try_from(packet) {
            Ok(frame) => Some(frame),
            Err(_) => None,
        }
    }
}

pub struct Ipv4Decap;

impl Processor for Ipv4Decap {
    type Input = EthernetFrame;
    type Output = Ipv4Packet;

    fn process(&mut self, frame: Self::Input) -> Option<Self::Output> {
        match Ipv4Packet::try_from(frame) {
            Ok(packet) => Some(packet),
            Err(_) => None,
        }
    }
}

pub struct Ipv4Encap;

impl Processor for Ipv4Encap {
    type Input = Ipv4Packet;
    type Output = EthernetFrame;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        match EthernetFrame::try_from(packet) {
            Ok(frame) => Some(frame),
            Err(_) => None,
        }
    }
}
