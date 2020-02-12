#![cfg(target_os = "linux")]
#![cfg(feature = "tokio-support")]

use afpacket;
use rand::{self, Rng};
use route_rs_packets as packets;
use std::{ffi::CString, net, time::Duration};
use tokio::{self, runtime, sync::mpsc, time};

#[test]
#[ignore]
fn layer2_loopback() {
    // If this takes more than a second to occur, something's definitely wrong.
    let timeout = Duration::from_secs(1);

    let mut rt = runtime::Runtime::new().unwrap();

    rt.block_on(async {
        let mut rng = rand::thread_rng();
        let iface_name = CString::new("lo").unwrap();

        let mut side_a = afpacket::AsyncBoundSocket::from_interface(&iface_name).unwrap();

        let (mut tx, mut rx) = mpsc::channel(1);

        let task_b = tokio::spawn(async move {
            let mut side_b = afpacket::AsyncBoundSocket::from_interface(&iface_name).unwrap();
            side_b.set_promiscuous(true).unwrap();

            println!("b: receiving packet");
            let mut in_buffer = vec![0; 1500];
            let (len, _) = side_b.recv(&mut in_buffer).await.unwrap();
            in_buffer.resize(len, 0);
            println!("b: received packet");

            side_b.set_promiscuous(false).unwrap();

            tx.send(in_buffer).await.unwrap();
        });

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
        side_a.send(&eth_pkt.data).await.unwrap();
        println!("a: sent packet");

        let in_buffer = time::timeout(timeout, rx.recv()).await.unwrap().unwrap();
        assert_eq!(in_buffer, eth_pkt.data);

        task_b.await.unwrap();
    });
}

#[test]
#[ignore]
fn bidirectional_loop() {
    // If this takes more than a second to occur, something's definitely wrong.
    let timeout = Duration::from_secs(1);

    let mut rt = runtime::Runtime::new().unwrap();

    rt.block_on(async {
        let mut rng = rand::thread_rng();
        let iface_name = CString::new("lo").unwrap();

        let mut side_a = afpacket::AsyncBoundSocket::from_interface(&iface_name).unwrap();
        side_a.set_promiscuous(true).unwrap();
        let mut side_b = afpacket::AsyncBoundSocket::from_interface(&iface_name).unwrap();
        side_b.set_promiscuous(true).unwrap();

        let (mut tx_a, mut rx_a) = side_a.split();
        let (mut tx_b, mut rx_b) = side_b.split();

        let task_b = tokio::spawn(async move {
            let mut in_buffer = vec![0; 1500];
            let (len, _) = rx_b.recv(&mut in_buffer).await.unwrap();
            in_buffer.resize(len, 0);

            tx_a.send(&in_buffer).await.unwrap();
        });

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

        tx_b.send(&eth_pkt.data).await.unwrap();

        let mut in_buffer = vec![0; 1500];
        let (len, _) = time::timeout(timeout, rx_a.recv(&mut in_buffer))
            .await
            .unwrap()
            .unwrap();
        in_buffer.resize(len, 0);
        assert_eq!(in_buffer, eth_pkt.data);

        task_b.await.unwrap();
    });
}
