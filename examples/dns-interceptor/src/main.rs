use crate::packets::SimplePacket;
use crate::packets::{Interface, IpAndPort};
use crossbeam::crossbeam_channel;
use route_rs_runtime::pipeline::Runner;

mod packets;
mod pipeline;
mod processors;

fn main() {
    let (input_sender, input_receiver) = crossbeam_channel::unbounded();
    let (output_sender, output_receiver) = crossbeam_channel::unbounded();

    let input_packets = vec![
        (
            Interface::LAN,
            SimplePacket {
                source: IpAndPort::new([10, 0, 0, 2], 9779),
                destination: IpAndPort::new([1, 2, 3, 4], 80),
                payload: String::from("HTTP GET /index.html"),
            },
        ),
        (
            Interface::LAN,
            SimplePacket {
                source: IpAndPort::new([10, 0, 0, 2], 9779),
                destination: IpAndPort::new([1, 2, 3, 4], 53),
                payload: String::from("gateway.route-rs.local"),
            },
        ),
    ];

    let output_packets = vec![
        (
            Interface::WAN,
            SimplePacket {
                source: IpAndPort::new([10, 0, 0, 2], 9779),
                destination: IpAndPort::new([1, 2, 3, 4], 80),
                payload: String::from("HTTP GET /index.html"),
            },
        ),
        (
            Interface::LAN,
            SimplePacket {
                source: IpAndPort::new([1, 2, 3, 4], 53),
                destination: IpAndPort::new([10, 0, 0, 2], 9779),
                payload: String::from("10.0.0.1"),
            },
        ),
    ];

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
