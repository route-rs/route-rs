use crate::elements::*;
use crate::packets::*;
use futures::lazy;
use route_rs_runtime::link::*;
use route_rs_runtime::pipeline::{InputChannelLink, OutputChannelLink};

pub struct Pipeline {}

impl route_rs_runtime::pipeline::Runner for Pipeline {
    type Input = (Interface, SimplePacket);
    type Output = (Interface, SimplePacket);

    fn run(
        input_channel: crossbeam::Receiver<Self::Input>,
        output_channel: crossbeam::Sender<Self::Output>,
    ) {
        let elem_1 = SetInterfaceByDestination::new(u32::from_be_bytes([10, 0, 0, 0]), 0xFF00_0000);
        let elem_2 = ClassifyDNS::new();
        let elem_3 = LocalDNSInterceptor::new();

        let link_1 = InputChannelLink::new(input_channel);
        let link_2 = SyncLink::new(Box::new(link_1), elem_1);
        let mut link_3 = ClassifyLink::new(
            Box::new(link_2),
            elem_2,
            Box::new(|c| if c == ClassifyDNSOutput::DNS { 0 } else { 1 }),
            10,
            2,
        );
        let link_3_egressor_1 = link_3.egressors.pop().unwrap();
        let link_3_egressor_0 = link_3.egressors.pop().unwrap();
        let link_4 = SyncLink::new(Box::new(link_3_egressor_0), elem_3);
        let mut link_5 = JoinLink::new(vec![Box::new(link_4), Box::new(link_3_egressor_1)], 10);
        let link_6 = OutputChannelLink::new(Box::new(link_5.egressor), output_channel);

        let link_5_ingressor_1 = link_5.ingressors.pop().unwrap();
        let link_5_ingressor_0 = link_5.ingressors.pop().unwrap();

        tokio::run(lazy(move || {
            tokio::spawn(link_3.ingressor);
            tokio::spawn(link_5_ingressor_0);
            tokio::spawn(link_5_ingressor_1);
            tokio::spawn(link_6);
            Ok(())
        }));
    }
}
