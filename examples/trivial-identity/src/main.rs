use crate::packets::IntegerPacket;
use crate::subscriber::TrivialIdentitySubscriber;
use crossbeam::crossbeam_channel;
use route_rs_runtime::pipeline::Runner;

extern crate tracing;
use tracing::{info, span, Level};

mod packets;
mod pipeline;
mod subscriber;

fn main() {
    let subscriber = TrivialIdentitySubscriber::new();
    tracing::subscriber::set_global_default(subscriber).expect("setting tracing default failed");

    let (input_sender, input_receiver) = crossbeam_channel::unbounded();
    let (output_sender, output_receiver) = crossbeam_channel::unbounded();

    for n in 0..10 {
        let in_packet = IntegerPacket { id: n };
        match input_sender.send(in_packet.clone()) {
            Ok(_) => println!("Sent {:?}", in_packet),
            Err(err) => panic!("Input channel error {}", err),
        }
    }

    drop(input_sender);

    span!(Level::TRACE, "Pipeline span", field_value = 0).in_scope(|| {
        info!(
            in_pipeline = true,
            num_pipelines = 1,
            "hi from inside the pipeline span!"
        );
        crate::pipeline::Pipeline::run(input_receiver, output_sender);
    });

    loop {
        match output_receiver.try_recv() {
            Ok(out_packet) => println!("Received {:?}", out_packet),
            Err(crossbeam_channel::TryRecvError::Empty)
            | Err(crossbeam_channel::TryRecvError::Disconnected) => return,
        }
    }
}
