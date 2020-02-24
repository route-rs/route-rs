use crate::arp::ArpFrame;
use crate::types::InterfaceAnnotated;
use route_rs_packets::{EthernetFrame, MacAddr};
use route_rs_runtime::processor::Processor;
use std::collections::HashMap;

pub(crate) struct ArpGenerator {
    // Mapping between (protocol type, protocol address) -> 48-bit Ethernet address
    translation_table: HashMap<(u16, Box<[u8]>), MacAddr>,
}

impl ArpGenerator {
    pub fn new() -> Self {
        ArpGenerator {
            translation_table: HashMap::new(),
        }
    }
}

impl Processor for ArpGenerator {
    type Input = InterfaceAnnotated<EthernetFrame>;
    type Output = InterfaceAnnotated<EthernetFrame>;

    ///
    /// If the lookup for the (protocol type, protocol address) fails, it probably informs the
    /// caller that it is throwing the packet away (on the assumption the packet will be
    /// retransmitted by a higher network layer), and generates an Ethernet packet with a type field
    /// of ether_type$ADDRESS_RESOLUTION. The Address Resolution module then sets
    ///
    /// Hardware Type: ares_hrd$Ethernet = 1
    /// Protocol Type: type that is being resolved
    /// Hardware Address Length: 6 (the number of bytes in a 48.bit Ethernet address)
    /// Protocol Address Length: length of an address in that protocol
    /// Op: ares_op$REQUEST = 1
    /// Sender Hardware Address: the 48.bit ethernet address of itself
    /// Sender Protocol Address: the protocol address of itself
    /// Target Protocol Address: the protocol address of the machine that is trying to be accessed.
    ///
    /// It does not set Target Hardware Address to anything in particular, because it is this value
    /// that it is trying to determine. It could set the Target Hardware Address to the broadcast
    /// address (FF:FF:FF:FF:FF:FF) for the hardware (all ones in the case of the 10Mbit Ethernet)
    /// if that makes it convenient for some aspect of the implementation.
    ///
    /// It then causes this packet to be broadcast to all stations on the Ethernet cable originally
    /// determined by the routing mechanism.
    ///
    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arp_test_runs() {
        println!("Hello!")
    }
}
