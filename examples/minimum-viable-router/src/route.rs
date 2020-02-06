use route_rs_packets::EthernetFrame;
use route_rs_runtime::link::{Link, LinkBuilder, PacketStream};

pub struct Route {
    in_streams: Option<Vec<PacketStream<EthernetFrame>>>,
}

impl Route {
    pub fn new() -> Self {
        Route { in_streams: None }
    }
}

impl LinkBuilder<EthernetFrame, EthernetFrame> for Route {
    fn ingressors(self, in_streams: Vec<PacketStream<EthernetFrame>>) -> Self {
        assert_eq!(in_streams.len(), 3, "Wrong number of inputs to Route");

        Route {
            in_streams: Some(in_streams),
        }
    }

    fn ingressor(self, in_stream: PacketStream<EthernetFrame>) -> Self {
        match self.in_streams {
            Some(mut streams) => {
                assert!(streams.len() < 3, "May only provide 3 input streams");
                streams.push(in_stream);
                Route {
                    in_streams: Some(streams),
                }
            }
            None => Route {
                in_streams: Some(vec![in_stream]),
            },
        }
    }

    fn build_link(self) -> Link<EthernetFrame> {
        assert!(
            self.in_streams.is_some() && self.in_streams.unwrap().len() == 3,
            "Must provide 3 input streams"
        );

        let mut all_runnables = vec![];
        let mut all_egressors = vec![];

        (all_runnables, all_egressors)
    }
}
