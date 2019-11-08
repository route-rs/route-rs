use route_rs_packets::{EthernetFrame, Ipv6Packet, Ipv4Packet};
use route_rs_runtime::processor::Processor;

mod processors;

fn main() {
    let data: Vec<u8> =
        vec![0xde, 0xad, 0xbe, 0xef, 0xff, 0xff, 1, 2, 3, 4, 5, 6, 0, 0,
        0x60, 0, 0, 0, 0, 4, 17, 64, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde,
        0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13,
        14, 15, 0xa, 0xb, 0xc, 0xd,
    ];
    let frame = EthernetFrame::new(data).unwrap();

    let mut processor = processors::Ipv6Encap;

    let packet = processor.process(frame);

    match packet {
        Some(packet) => {
            println!("Got ipv6packet: src_addr = {}", packet.src_addr());
        }
        None => {
            println!("Some error occured");
        }
    }
}
