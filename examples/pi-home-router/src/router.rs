use crate::arp::{ArpGenerator, ArpHandler};
use route_rs_runtime::link::primitive::ProcessLink;
use route_rs_runtime::link::{Link, LinkBuilder, PacketStream, ProcessLinkBuilder};
use route_rs_runtime::utils::test::packet_generators::immediate_stream;

/// Top level structure, implements LinkBuilder so it can be run by the test harness
#[derive(Default)]
pub(crate) struct Router {
    host: Option<String>,
    lan: Option<String>,
    wan: Option<String>,
}

impl Router {
    pub(crate) fn new() -> Self {
        Router {
            host: None,
            lan: None,
            wan: None,
        }
    }

    pub(crate) fn host(self, host: &str) -> Self {
        Router {
            host: Some(String::from(host)),
            lan: self.lan,
            wan: self.wan,
        }
    }

    pub(crate) fn lan(self, lan: &str) -> Self {
        Router {
            host: self.host,
            lan: Some(String::from(lan)),
            wan: self.wan,
        }
    }

    pub(crate) fn wan(self, wan: &str) -> Self {
        Router {
            host: self.host,
            lan: self.lan,
            wan: Some(String::from(wan)),
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
            .processor(ArpHandler::new())
            .build_link();

        // TODO: not sure where to put ArpGenerator
        let _arp_generator = ProcessLink::new()
            .ingressor(immediate_stream(vec![]))
            .processor(ArpGenerator::new())
            .build_link();

        (vec![], vec![])
    }
}
