use crate::interface::*;
#[allow(unused_imports)]
use crate::processors::*;
use futures::lazy;
#[allow(unused_imports)]
use route_rs_runtime::link::*;
#[allow(unused_imports)]
use route_rs_runtime::pipeline::{InputChannelLink, OutputChannelLink};
use smoltcp::wire::*;

pub struct Pipeline {}

impl route_rs_runtime::pipeline::Runner for Pipeline {
    type Input = (Interface, Ipv4Packet<Vec<u8>>);
    type Output = (Interface, Ipv4Packet<Vec<u8>>);

    fn run(
        _input_channel: crossbeam::Receiver<Self::Input>,
        _output_channel: crossbeam::Sender<Self::Output>,
    ) {
        tokio::run(lazy(move || Ok(())));
    }
}
