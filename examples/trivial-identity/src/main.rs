use crate::packets::IntegerPacket;
use crossbeam::crossbeam_channel;
use route_rs_runtime::pipeline::Runner;

mod packets;
mod pipeline;

fn main() {
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

    crate::pipeline::Pipeline::run(input_receiver, output_sender);

    loop {
        match output_receiver.try_recv() {
            Ok(out_packet) => println!("Received {:?}", out_packet),
            Err(crossbeam_channel::TryRecvError::Empty)
            | Err(crossbeam_channel::TryRecvError::Disconnected) => return,
        }
    }
}
