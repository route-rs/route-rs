#![cfg(target_os = "linux")]

use afpacket;
use rand::{self, Rng};
use route_rs_packets as packets;
use std::{ffi::CString, net, sync::mpsc, thread, time::Duration};

#[test]
#[ignore]
fn layer2_loopback() {
    // If this takes more than a second to occur, something's definitely wrong.
    let timeout = Duration::from_secs(1);

    let mut rng = rand::thread_rng();

    let iface_name = CString::new("lo").unwrap();

    let side_a = afpacket::Socket::new().unwrap();
    let mut side_a = side_a.bind(&iface_name).unwrap();

    let side_b = afpacket::Socket::new().unwrap();

    let (tx, rx) = mpsc::channel();

    let thread_b = thread::spawn(move || {
        let mut side_b = side_b.bind(&iface_name).unwrap();
        side_b.set_promiscuous(true).unwrap();

        println!("b: recving packet");
        let mut in_buffer = vec![0; 1500];
        let (len, _) = side_b.recv(&mut in_buffer).unwrap();
        in_buffer.resize(len, 0);
        println!("b: recved packet");

        side_b.set_promiscuous(false).unwrap();

        tx.send(in_buffer).unwrap();
    });

    // now send a packet from side a to side b
    let body = {
        let mut body = vec![0; 64];
        rng.fill(&mut body[..]);
        body
    };
    let mut udp_pkt = packets::UdpSegment::empty();
    udp_pkt.set_src_port(3001);
    udp_pkt.set_dest_port(3002);
    udp_pkt.set_payload(&body);
    let mut ipv4_pkt = packets::Ipv4Packet::encap_udp(udp_pkt);
    ipv4_pkt.set_src_addr(net::Ipv4Addr::new(10, 0, 0, 1));
    ipv4_pkt.set_dest_addr(net::Ipv4Addr::new(10, 0, 0, 2));
    ipv4_pkt.set_ttl(2);
    let mut eth_pkt = packets::EthernetFrame::encap_ipv4(ipv4_pkt);
    eth_pkt.set_src_mac(packets::MacAddr::new([0xff, 0xff, 0xff, 0xff, 0xff, 0xff]));
    eth_pkt.set_dest_mac(packets::MacAddr::new([0xff, 0xff, 0xff, 0xff, 0xff, 0xff]));

    println!("a: sending packet");
    side_a.send(&eth_pkt.data).unwrap();
    println!("a: sent packet");

    let in_buffer = rx.recv_timeout(timeout).unwrap();
    assert_eq!(in_buffer, eth_pkt.data);

    thread_b.join().unwrap();
}
