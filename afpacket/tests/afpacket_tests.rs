#![cfg(target_os = "linux")]

use afpacket;
use rand::{self, Rng};
use smoltcp::{phy::ChecksumCapabilities, wire};
use std::{ffi::CString, sync::mpsc, thread, time::Duration};

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
    let udp_hdr = wire::UdpRepr {
        src_port: 3001,
        dst_port: 3002,
        payload: &body,
    };
    let src_addr = wire::Ipv4Address::new(10, 0, 0, 1);
    let dst_addr = wire::Ipv4Address::new(10, 0, 0, 2);
    let ipv4_hdr = wire::Ipv4Repr {
        src_addr,
        dst_addr,
        protocol: wire::IpProtocol::Udp,
        payload_len: udp_hdr.buffer_len(),
        hop_limit: 2,
    };
    let eth_hdr = wire::EthernetRepr {
        src_addr: wire::EthernetAddress::BROADCAST,
        dst_addr: wire::EthernetAddress::BROADCAST,
        ethertype: wire::EthernetProtocol::Ipv4,
    };
    let buf_size =
        wire::EthernetFrame::<&[u8]>::buffer_len(ipv4_hdr.buffer_len() + udp_hdr.buffer_len());
    let mut out_buffer = vec![0; buf_size];
    let mut eth_pkt = wire::EthernetFrame::new_checked(&mut out_buffer).unwrap();
    eth_hdr.emit(&mut eth_pkt);
    let mut ipv4_pkt = wire::Ipv4Packet::new_checked(eth_pkt.payload_mut()).unwrap();
    ipv4_hdr.emit(&mut ipv4_pkt, &ChecksumCapabilities::default());
    let mut udp_pkt = wire::UdpPacket::new_unchecked(ipv4_pkt.payload_mut());
    udp_hdr.emit(
        &mut udp_pkt,
        &wire::IpAddress::Ipv4(src_addr),
        &wire::IpAddress::Ipv4(dst_addr),
        &ChecksumCapabilities::default(),
    );

    println!("a: sending packet");
    side_a.send(&out_buffer).unwrap();
    println!("a: sent packet");

    let in_buffer = rx.recv_timeout(timeout).unwrap();
    assert_eq!(in_buffer, out_buffer);

    thread_b.join().unwrap();
}
