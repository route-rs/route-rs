use route_rs_packets::EthernetFrame;
use route_rs_runtime::processor::Processor;

/// EthernetFrameToVec converts EthernetFrames into a raw Vec<u8>. This is done
/// by stripping the data field out of the EthernetFrame structure.
#[derive(Default)]
pub(crate) struct EthernetFrameToVec;

impl Processor for EthernetFrameToVec {
    type Input = EthernetFrame;
    type Output = Vec<u8>;

    fn process(&mut self, mut packet: Self::Input) -> Option<Self::Output> {
        if packet.layer2_offset == 0 {
            //Save an allocation
            Some(packet.data)
        } else {
            Some(packet.data.split_off(packet.layer2_offset))
        }
    }
}
