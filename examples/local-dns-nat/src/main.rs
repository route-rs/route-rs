use crate::interface::*;
use crossbeam::crossbeam_channel;
use route_rs_runtime::pipeline::Runner;
use smoltcp::wire::*;

mod elements;
mod interface;
mod pipeline;

fn main() {
    let (input_sender, input_receiver) = crossbeam_channel::unbounded();
    let (output_sender, output_receiver) = crossbeam_channel::unbounded();

    let input_packets: Vec<(Interface, Ipv4Packet<Vec<u8>>)> = vec![];

    let output_packets: Vec<(Interface, Ipv4Packet<Vec<u8>>)> = vec![];

    for p in input_packets {
        match input_sender.send(p.clone()) {
            Ok(_) => println!("Sent {:?}", p),
            Err(err) => panic!("Input channel error {}", err),
        }
    }

    drop(input_sender);

    crate::pipeline::Pipeline::run(input_receiver, output_sender);

    let mut received_packets = vec![];
    loop {
        match output_receiver.try_recv() {
            Ok(out_packet) => {
                println!("Received {:?}", out_packet);
                received_packets.push(out_packet);
            }
            Err(crossbeam_channel::TryRecvError::Empty)
            | Err(crossbeam_channel::TryRecvError::Disconnected) => {
                // We can't guarantee the order of the packets coming out of the router
                // Technically this test allows us to have duplicate packets which would be dumb,
                // but there's not much reason to expect that here so this is okay for now.
                assert!(received_packets.iter().all(|p| output_packets.contains(p)));
                return;
            }
        }
    }
}
