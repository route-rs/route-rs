use crate::arp::{ArpGenerator, ArpHandler};
use route_rs_packets::MacAddr;
use route_rs_runtime::link::primitive::ProcessLink;
use route_rs_runtime::link::{Link, LinkBuilder, PacketStream, ProcessLinkBuilder};
use route_rs_runtime::utils::test::packet_generators::immediate_stream;
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

/// Top level structure, implements LinkBuilder so it can be run by the test harness
#[derive(Default)]
pub(crate) struct Router {
    host: Option<String>,
    lan: Option<String>,
    wan: Option<String>,

    // TODO: investigate/discuss concurrency options
    // https://doc.rust-lang.org/std/sync/struct.RwLock.html
    // https://docs.rs/chashmap/2.2.2/chashmap/
    // https://github.com/jonhoo/rust-evmap
    // https://github.com/jonhoo/flurry
    /// Top-level ARP table reference.
    /// Links that require a reference should clone this Arc.
    arp_table: Arc<Mutex<HashMap<Ipv4Addr, MacAddr>>>,
}

impl Router {
    pub(crate) fn new() -> Self {
        Router {
            host: None,
            lan: None,
            wan: None,
            arp_table: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) fn host(self, host: &str) -> Self {
        Router {
            host: Some(String::from(host)),
            lan: self.lan,
            wan: self.wan,
            arp_table: self.arp_table,
        }
    }

    pub(crate) fn lan(self, lan: &str) -> Self {
        Router {
            host: self.host,
            lan: Some(String::from(lan)),
            wan: self.wan,
            arp_table: self.arp_table,
        }
    }

    pub(crate) fn wan(self, wan: &str) -> Self {
        Router {
            host: self.host,
            lan: self.lan,
            wan: Some(String::from(wan)),
            arp_table: self.arp_table,
        }
    }
}

//This link doesn't take or return anything, since it is the top level of our router.
impl LinkBuilder<(), ()> for Router {
    fn ingressors(self, _ingressors: Vec<PacketStream<()>>) -> Router {
        panic!("Top level router takes no input");
    }

    fn ingressor(self, _ingressor: PacketStream<()>) -> Router {
        panic!("Top level router takes no input");
    }

    fn build_link(self) -> Link<()> {
        // Return an empty link for now. As we build the link, this should be returning
        // all the relevant runnables, and no egressors(Since our router has none)

        // TODO: wire up from protocol classifier
        let _arp_handler = ProcessLink::new()
            .ingressor(immediate_stream(vec![]))
            .processor(ArpHandler::new(self.arp_table.clone()))
            .build_link();

        // TODO: not sure where to put ArpGenerator
        let _arp_generator = ProcessLink::new()
            .ingressor(immediate_stream(vec![]))
            .processor(ArpGenerator::new(self.arp_table))
            .build_link();

        (vec![], vec![])
    }
}
