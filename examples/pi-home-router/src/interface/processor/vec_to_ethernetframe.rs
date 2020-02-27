use route_rs_packets::EthernetFrame;
use route_rs_runtime::processor::Processor;

/// VecToEthernetFrame converts a raw Vec<u8> buffer into an EthernetFrame packet. Any
/// buffers that fail to parse into an EthernetFrame are dropped.
#[derive(Default)]
pub(crate) struct VecToEthernetFrame;

impl Processor for VecToEthernetFrame {
    type Input = Vec<u8>;
    type Output = EthernetFrame;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        match EthernetFrame::from_buffer(packet, 0) {
            Ok(packet) => Some(packet),
            Err(_) => None,
        }
    }
}
