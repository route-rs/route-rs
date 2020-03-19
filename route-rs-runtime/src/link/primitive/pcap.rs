use crate::link::{Link, LinkBuilder, PacketStream};
use futures::ready;
use futures::task::{Context, Poll};
use futures::Stream;
use pcap::{Capture, Offline};
use std::iter::Peekable;
use std::pin::Pin;
use std::time::{Duration, Instant};

pub enum PcapReplayMode {
    Dump,
    Realtime,
}

struct PcapPacketWrap {
    timestamp: Duration,
    data: Vec<u8>,
}
// Wrapper to pcap::Capture to create iterator
// because pcap::Packet is basically just references to file data
struct PcapCaptureWrap {
    capture: Capture<Offline>,
}
impl Iterator for PcapCaptureWrap {
    type Item = PcapPacketWrap;
    fn next(&mut self) -> Option<Self::Item> {
        if let Ok(p) = self.capture.next() {
            let ts = p.header.ts;
            //We kinda ignore caplen and len. If they differ then the packet is fragmented.
            //Why are time_t and suseconds_t defined as signed integers?
            let timestamp =
                Duration::from_micros(ts.tv_sec as u64) + Duration::from_secs(ts.tv_sec as u64);
            let data: Vec<u8> = p.data.into();
            Some(PcapPacketWrap { timestamp, data })
        } else {
            None
        }
    }
}

pub struct FromPcap {
    src_file: Option<String>,
    replay_mode: PcapReplayMode,
}
impl Default for FromPcap {
    fn default() -> Self {
        FromPcap {
            src_file: None,
            replay_mode: PcapReplayMode::Dump,
        }
    }
}
impl FromPcap {
    pub fn new() -> Self {
        FromPcap::default()
    }

    pub fn replay_mode(mut self, replay_mode: PcapReplayMode) -> Self {
        self.replay_mode = replay_mode;
        self
    }

    pub fn src_file(mut self, src_file: String) -> Self {
        self.src_file = Some(src_file);
        self
    }
}
impl LinkBuilder<(), Vec<u8>> for FromPcap {
    fn ingressor(self, _in_stream: PacketStream<()>) -> Self {
        panic!("FromPcap takes NO input stream")
    }

    fn ingressors(self, _ingress_streams: Vec<PacketStream<()>>) -> Self {
        panic!("FromPcap takes NO input stream(s)")
    }

    fn build_link(self) -> Link<Vec<u8>> {
        if let Some(src_file) = self.src_file {
            let pcap_proc = FromPcapEggressor::new(src_file, self.replay_mode);
            (vec![], vec![Box::new(pcap_proc)])
        } else {
            panic!("Cannot build FromPcap! Missing source file")
        }
    }
}

struct FromPcapRealtimeState {
    start: Instant,
    cap_start: Duration,
}
enum PcapReplayState {
    RunDump,
    RunRealtime(FromPcapRealtimeState),
}

struct FromPcapEggressor {
    #[allow(dead_code)]
    src_file: String,
    capture: Peekable<PcapCaptureWrap>,
    replay_state: PcapReplayState,
}

impl FromPcapEggressor {
    fn new(src_file: String, replay_mode: PcapReplayMode) -> Self {
        let capture = Capture::from_file(src_file.clone()).unwrap();
        let mut cap_iter = PcapCaptureWrap { capture }.peekable();
        let replay_state = match replay_mode {
            PcapReplayMode::Dump => PcapReplayState::RunDump,
            PcapReplayMode::Realtime => {
                let start = Instant::now();
                let cap_start = if let Some(p) = cap_iter.peek() {
                    p.timestamp
                } else {
                    Duration::from_secs(0)
                };
                PcapReplayState::RunRealtime(FromPcapRealtimeState { start, cap_start })
            }
        };
        FromPcapEggressor {
            src_file,
            capture: cap_iter,
            replay_state,
        }
    }
}

impl Stream for FromPcapEggressor {
    type Item = Vec<u8>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let me = Pin::into_inner(self);
        match &me.replay_state {
            PcapReplayState::RunDump => {
                if let Some(p) = me.capture.next() {
                    let output_packet: Vec<u8> = p.data;
                    Poll::Ready(Some(output_packet))
                } else {
                    Poll::Ready(None)
                }
            }
            PcapReplayState::RunRealtime(state) => {
                let peek_intime = match me.capture.peek() {
                    Some(p) => (state.start.elapsed() > (p.timestamp - state.cap_start)),
                    None => return Poll::Ready(None),
                };
                if peek_intime {
                    match me.capture.next() {
                        Some(p) => Poll::Ready(Some(p.data)),
                        None => unreachable!(), // Peek says packet exists
                    }
                } else {
                    // No packet to send, will reschedule
                    // This is inefficient for Realtime mode
                    cx.waker().clone().wake();
                    Poll::Pending
                }
            }
        }
    }
}

use futures::Future;
use pcap::Linktype;
use pcap::{Packet, PacketHeader};
pub struct ToPcap {
    pcap_file: Option<String>,
    in_stream: Option<PacketStream<Vec<u8>>>,
}
impl Default for ToPcap {
    fn default() -> Self {
        ToPcap {
            pcap_file: None,
            in_stream: None,
        }
    }
}
impl ToPcap {
    pub fn new() -> Self {
        ToPcap::default()
    }
    pub fn pcap_file(self, pcap_file: String) -> Self {
        ToPcap {
            pcap_file: Some(pcap_file),
            in_stream: self.in_stream,
        }
    }
}
impl LinkBuilder<Vec<u8>, ()> for ToPcap {
    fn ingressor(self, in_stream: PacketStream<Vec<u8>>) -> Self {
        if self.in_stream.is_some() {
            panic!("ToPcap can only take 1 input stream")
        }

        ToPcap {
            pcap_file: self.pcap_file,
            in_stream: Some(in_stream),
        }
    }
    fn ingressors(self, mut ingress_streams: Vec<PacketStream<Vec<u8>>>) -> Self {
        assert_eq!(
            ingress_streams.len(),
            1,
            "ToPcap can only take 1 ingress stream"
        );

        if self.in_stream.is_some() {
            panic!("ToPcap can only take 1 input stream")
        }

        ToPcap {
            pcap_file: self.pcap_file,
            in_stream: Some(ingress_streams.remove(0)),
        }
    }
    fn build_link(self) -> Link<()> {
        if let Some(pcap_file) = self.pcap_file {
            if let Some(stream) = self.in_stream {
                let pcap_proc = ToPcapRunner::new(pcap_file, stream);
                (vec![Box::new(pcap_proc)], vec![])
            } else {
                panic!("Cannot build ToPcap! Missing in_stream")
            }
        } else {
            panic!("Cannot build ToPcap! Missing source file")
        }
    }
}

pub struct ToPcapRunner {
    #[allow(dead_code)]
    pcap_file: String,
    start: Instant,
    stream: PacketStream<Vec<u8>>,
    packet_dump: pcap::Savefile,
}
impl Unpin for ToPcapRunner {}
impl ToPcapRunner {
    pub fn new(pcap_file: String, stream: PacketStream<Vec<u8>>) -> Self {
        let start = Instant::now();
        let cap = Capture::dead(Linktype(1)).unwrap();
        let packet_dump = cap.savefile(pcap_file.clone()).unwrap();
        ToPcapRunner {
            pcap_file,
            start,
            stream,
            packet_dump,
        }
    }
}
impl Future for ToPcapRunner {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        let collector = Pin::into_inner(self);
        loop {
            match ready!(Pin::new(&mut collector.stream).poll_next(cx)) {
                Some(packet) => {
                    let ts = collector.start.elapsed();
                    let tv_sec = ts.as_secs();
                    let tv_usec = (ts - Duration::from_secs(tv_sec)).as_micros();
                    let packet_hdr = PacketHeader {
                        ts: libc::timeval {
                            tv_sec: tv_sec as i64,
                            #[cfg(target_os = "linux")]
                            tv_usec: tv_usec as i64,
                            #[cfg(target_os = "macos")]
                            tv_usec: tv_usec as i32,
                        },
                        caplen: packet.len() as u32,
                        len: packet.len() as u32,
                    };
                    let pcap_packet = Packet {
                        header: &packet_hdr,
                        data: &packet,
                    };
                    collector.packet_dump.write(&pcap_packet);
                }
                None => return Poll::Ready(()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test::harness::{initialize_runtime, test_link};
    use std::path::PathBuf;

    #[test]
    fn pcap_read_example() {
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("src/utils/test/fixture/test_icmp_8888.pcapng");
        let file = d.to_string_lossy().into_owned();
        println!("Using {}", file);

        let mut runtime = initialize_runtime();
        let results = runtime.block_on(async {
            let link = FromPcap::new()
                .src_file(file)
                .replay_mode(PcapReplayMode::Dump)
                .build_link();
            test_link(link, None).await
        });
        // The test_icmp_8888.pcapng file contains 12 icmp requests and 12 icmp replies.
        // Every second, an icmp request is sent and an icmp response follows.
        // Following checks that the packets are 24 interleaved icmp requests and replies.
        assert_eq!(results[0].len(), 24);
        let byte_ethertype_ipv4 = vec![0x08, 0x00]; //At offset 12 size 2
        let byte_ipv4_proto = vec![0x01]; // At offset 23 size 1
        let byte_icmp_type_code_request = vec![0x08, 0x00]; // At offset 34 size 2
        let byte_icmp_type_code_response = vec![0x00, 0x00]; // At offset 34 size 2
        for i in 0..24 {
            if i % 2 == 0 {
                //Expect icmp request
                assert_eq!(results[0][i][12..14], *byte_ethertype_ipv4);
                assert_eq!(results[0][i][23..24], *byte_ipv4_proto);
                assert_eq!(results[0][i][34..36], *byte_icmp_type_code_request);
            } else {
                //Expect icmp response
                assert_eq!(results[0][i][12..14], *byte_ethertype_ipv4);
                assert_eq!(results[0][i][23..24], *byte_ipv4_proto);
                assert_eq!(results[0][i][34..36], *byte_icmp_type_code_response);
            }
        }
    }

    use tempfile::tempdir;
    #[test]
    fn pcap_read_write_example() {
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("src/utils/test/fixture/test_icmp_8888.pcapng");
        let rfile = d.to_string_lossy().into_owned();

        // tempdir valid until out of scope. File descriptors to content
        // inside created tempdir should be closed before the tempdir can
        // properly close. Asserts will also prevent tempdir from being
        // deleted. This is a feature.
        let dir = tempdir().unwrap();
        let d = dir.path().join("test_icmp_8888_copy.pcapng");
        let wfile: String = d.to_string_lossy().into_owned();
        println!("Using read file {}", rfile);
        println!("Using write file {}", wfile);
        {
            // FromPcap to ToPcap
            let mut runtime = initialize_runtime();
            runtime.block_on(async {
                let (mut run0, mut egr0) = FromPcap::new()
                    .src_file(rfile.clone())
                    .replay_mode(PcapReplayMode::Dump)
                    .build_link();
                let (run1, egr1) = ToPcap::new()
                    .pcap_file(wfile.clone())
                    .ingressor(egr0.pop().unwrap())
                    .build_link();
                run0.extend(run1);
                let link = (run0, egr1);
                test_link(link, None).await
            });

            // Confirm both files contain same packets with FromPcap
            let mut runtime = initialize_runtime();
            let results_read = runtime.block_on(async {
                let link = FromPcap::new()
                    .src_file(rfile)
                    .replay_mode(PcapReplayMode::Dump)
                    .build_link();
                test_link(link, None).await
            });
            let results_write = runtime.block_on(async {
                let link = FromPcap::new()
                    .src_file(wfile)
                    .replay_mode(PcapReplayMode::Dump)
                    .build_link();
                test_link(link, None).await
            });
            assert_eq!(results_read, results_write);
        }
    }
}
