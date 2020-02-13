#![cfg(target_os = "linux")]

use afpacket;
use rand::{self, Rng};
use route_rs_afpacket::{AfPacketInput, AfPacketOutput};
use route_rs_packets as packets;
use route_rs_runtime::{
    link::LinkBuilder,
    utils::test::{
        harness::{initialize_runtime, test_link},
        packet_generators::immediate_stream,
    },
};
use std::{ffi::CString, net, time::Duration};
use tokio::{self, time};

fn create_socket() -> (afpacket::SendHalf, afpacket::RecvHalf) {
    let iface = {
        let iface = CString::new("lo");
        assert!(iface.is_ok());
        iface.unwrap()
    };
    let sock = {
        let sock = afpacket::AsyncBoundSocket::from_interface(&iface);
        assert!(sock.is_ok());
        sock.unwrap()
    };

    sock.split()
}

fn make_packet(rng: &mut impl Rng) -> Vec<u8> {
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
    eth_pkt.set_src_mac(packets::MacAddr::new([0xac, 0x67, 0x3d, 0xfa, 0xbc, 0xdd]));
    eth_pkt.set_dest_mac(packets::MacAddr::new([0xac, 0x67, 0x3d, 0xfa, 0xbc, 0xdf]));

    eth_pkt.data
}

// This test is marked as #[ignore] because it needs to be able to open
// a socket. Opening a socket requires CAP_NET_RAW.
// To run this test on a Linux machine, run the test binary like so:
// > sudo target/<path to test executable> --ignored --test-threads=1
#[test]
#[should_panic]
#[ignore]
fn output_panics_when_built_without_ingressor() {
    let (s, _) = create_socket();

    AfPacketOutput::new().channel(s).build_link();
}

#[test]
#[should_panic]
fn output_panics_when_built_without_channel() {
    let packet_generator = immediate_stream(vec![]);

    AfPacketOutput::new()
        .ingressor(packet_generator)
        .build_link();
}

// This test is marked as #[ignore] because it needs to be able to open
// a socket. Opening a socket requires CAP_NET_RAW.
// To run this test on a Linux machine, run the test binary like so:
// > sudo target/<path to test executable> --ignored --test-threads=1
#[test]
#[should_panic]
#[ignore]
fn output_panics_when_built_with_multiple_ingressors() {
    let (s, _) = create_socket();
    let pkt_gen_1 = immediate_stream(vec![]);
    let pkt_gen_2 = immediate_stream(vec![]);

    AfPacketOutput::new()
        .ingressors(vec![pkt_gen_1, pkt_gen_2])
        .channel(s)
        .build_link();
}

#[test]
#[should_panic]
fn input_panics_when_built_with_ingressors() {
    AfPacketInput::new()
        .ingressors(vec![immediate_stream(vec![])])
        .build_link();
}

#[test]
#[should_panic]
fn input_panics_when_built_without_channel() {
    AfPacketInput::new().build_link();
}

// This test is marked as #[ignore] because it needs to be able to open
// a socket. Opening a socket requires CAP_NET_RAW.
// To run this test on a Linux machine, run the test binary like so:
// > sudo target/<path to test executable> --ignored --test-threads=1
#[test]
#[ignore]
fn input_identity() {
    let timeout = Duration::from_secs(1);
    let mut rt = initialize_runtime();

    // make packet
    let mut rng = rand::thread_rng();
    let pkt_data = make_packet(&mut rng);
    let ext_pkt_data = pkt_data.clone();

    let results = rt.block_on(async {
        let iface_name = CString::new("lo").unwrap();

        // set up router
        let mut router_side = afpacket::AsyncBoundSocket::from_interface(&iface_name).unwrap();
        router_side.set_promiscuous(true).unwrap();
        let (_, rx_rtr) = router_side.split();
        let mut external = afpacket::AsyncBoundSocket::from_interface(&iface_name).unwrap();

        // set up link
        let link = AfPacketInput::new().channel(rx_rtr).build_link();
        let _external_task = tokio::spawn(async move {
            // send packet from afpacket socket -> route-rs-afpacket
            external.send(&ext_pkt_data).await.unwrap();
        });

        test_link(link, Some(timeout)).await
    });

    // compare
    assert_eq!(results[0], vec![pkt_data]);
}

// This test is marked as #[ignore] because it needs to be able to open
// a socket. Opening a socket requires CAP_NET_RAW.
// To run this test on a Linux machine, run the test binary like so:
// > sudo target/<path to test executable> --ignored --test-threads=1
#[test]
#[ignore]
fn output_identity() {
    let timeout = Duration::from_secs(1);
    let mut rt = initialize_runtime();

    let mut rng = rand::thread_rng();
    let iface_name = CString::new("lo").unwrap();

    // make packet
    let pkt_data = make_packet(&mut rng);
    let ext_pkt_data = pkt_data.clone();

    let pkt = rt.block_on(async {
        // set up router
        let router_side = afpacket::AsyncBoundSocket::from_interface(&iface_name).unwrap();
        let (tx_rtr, _) = router_side.split();
        let mut external = afpacket::AsyncBoundSocket::from_interface(&iface_name).unwrap();
        external.set_promiscuous(true).unwrap();

        // set up link
        let link = AfPacketOutput::new()
            .ingressor(immediate_stream(vec![pkt_data]))
            .channel(tx_rtr)
            .build_link();

        test_link(link, None).await;
        // recv packet from route-rs-afpacket -> afpacket sock
        let mut pkt = vec![0; 1500];
        let (sz, _) = time::timeout(timeout, external.recv(&mut pkt))
            .await
            .unwrap()
            .unwrap();
        pkt.resize(sz, 0);

        pkt
    });

    // compare
    assert_eq!(pkt, ext_pkt_data);
}
