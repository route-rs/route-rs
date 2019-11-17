use crate::link::Link;
use futures::{Async, Poll, Stream};
use pcap::{Capture, Offline};
use std::iter::Peekable;
use std::time::{Duration, Instant};

pub enum PcapReplayMode {
    Dump,
    Realtime,
}

pub struct FromPcapProducer {
    src_file: Option<String>,
    replay_mode: PcapReplayMode,
}
impl Default for FromPcapProducer {
    fn default() -> Self {
        FromPcapProducer {
            src_file: None,
            replay_mode: PcapReplayMode::Dump,
        }
    }
}

impl FromPcapProducer {
    pub fn new() -> Self {
        FromPcapProducer::default()
    }
    pub fn replay_mode(self, replay_mode: PcapReplayMode) -> Self {
        FromPcapProducer {
            src_file: self.src_file,
            replay_mode,
        }
    }
    pub fn src_file(self, src_file: String) -> Self {
        FromPcapProducer {
            src_file: Some(src_file),
            replay_mode: self.replay_mode,
        }
    }
    pub fn build_link(self) -> Link<Vec<u8>> {
        if let Some(src_file) = self.src_file {
            let pcap_proc = FromPcapProcessor::new(src_file, self.replay_mode);
            (vec![], vec![Box::new(pcap_proc)])
        } else {
            panic!("Cannot build producer! Missing source file")
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

#[allow(dead_code)] // src_file
struct FromPcapProcessor {
    src_file: String,
    capture: Peekable<PcapCaptureWrap>,
    replay_state: PcapReplayState,
}

impl FromPcapProcessor {
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
        FromPcapProcessor {
            src_file,
            capture: cap_iter,
            replay_state,
        }
    }
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
use tokio::prelude::task;

impl Stream for FromPcapProcessor {
    type Item = Vec<u8>;
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match &self.replay_state {
            PcapReplayState::RunDump => {
                if let Some(p) = self.capture.next() {
                    let output_packet: Vec<u8> = p.data;
                    Ok(Async::Ready(Some(output_packet)))
                } else {
                    Ok(Async::Ready(None))
                }
            }
            PcapReplayState::RunRealtime(state) => {
                let peek_intime = if let Some(p) = self.capture.peek() {
                    (state.start.elapsed() > (p.timestamp - state.cap_start))
                } else {
                    //End of Pcap File
                    return Ok(Async::Ready(None));
                };
                if peek_intime {
                    if let Some(p) = self.capture.next() {
                        let output_packet: Vec<u8> = p.data;
                        println!(
                            "Sending Time {:?} {:?} {:?}",
                            state.start.elapsed(),
                            p.timestamp,
                            p.timestamp - state.cap_start
                        );
                        Ok(Async::Ready(Some(output_packet)))
                    } else {
                        unreachable!() // This is unexpected as logically peek believes a packet exists
                    }
                } else {
                    // No packet to send reschedule
                    // TODO: Should we rely on Delay instead and let that notify?
                    task::current().notify();
                    Ok(Async::NotReady)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test::harness::run_link;
    use std::path::PathBuf;

    #[test]
    fn read_packets_router() {
        // pcap captures 11 pings
        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("src/utils/test/fixture/test_icmp_8888.pcapng");
        let file = d.to_string_lossy().into_owned().to_string();
        println!("Using {}", file);

        let (run0, egr0) = FromPcapProducer::new()
            .src_file(file)
            .replay_mode(PcapReplayMode::Dump)
            .build_link();
        let link = (run0, egr0);
        let results = run_link(link);
        let pkt_request = vec![
            224, 203, 188, 35, 94, 173, 0, 36, 155, 33, 136, 16, 8, 0, 69, 0, 0, 84, 224, 38, 0, 0,
            128, 1, 9, 194, 192, 168, 128, 8, 8, 8, 8, 8, 8, 0, 195, 196, 8, 60, 0, 24, 46, 76,
            193, 93, 0, 0, 0, 0, 113, 106, 12, 0, 0, 0, 0, 0, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46,
            47, 48, 49, 50, 51, 52, 53, 54, 55,
        ];
        let pkt_reply = vec![
            0, 36, 155, 33, 136, 16, 224, 203, 188, 35, 94, 173, 8, 0, 69, 0, 0, 84, 0, 0, 0, 0,
            53, 1, 52, 233, 8, 8, 8, 8, 192, 168, 128, 8, 0, 0, 203, 196, 8, 60, 0, 24, 46, 76,
            193, 93, 0, 0, 0, 0, 113, 106, 12, 0, 0, 0, 0, 0, 16, 17, 18, 19, 20, 21, 22, 23, 24,
            25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46,
            47, 48, 49, 50, 51, 52, 53, 54, 55,
        ];
        for i in 0..22 {
            if i % 2 == 0 {
                assert_eq!(results[0][i][0..19], pkt_request[0..19]);
                assert_eq!(results[0][i][20..25], pkt_request[20..25]);
                assert_eq!(results[0][i][26..36], pkt_request[26..36]);
                assert_eq!(results[0][i][38..41], pkt_request[38..41]);
                assert_eq!(results[0][i][43..50], pkt_request[43..50]);
                assert_eq!(results[0][i][52..], pkt_request[52..]);
            } else {
                assert_eq!(results[0][i][0..36], pkt_reply[0..36]);
                assert_eq!(results[0][i][38..41], pkt_reply[38..41]);
                assert_eq!(results[0][i][43..50], pkt_reply[43..50]);
                assert_eq!(results[0][i][52..], pkt_reply[52..]);
            }
        }
    }
}
