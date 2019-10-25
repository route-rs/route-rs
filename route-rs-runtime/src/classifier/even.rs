use crate::classifier::Classifier;
use crate::link::{primitive::ClassifyLink, Link, LinkBuilder, PacketStream};

#[derive(Default)]
pub struct Even {}

impl Even {
    pub fn new() -> Self {
        Even {}
    }
}

impl Classifier for Even {
    type Packet = i32;
    type Class = bool;

    fn classify(&self, packet: &Self::Packet) -> Self::Class {
        packet % 2 == 0
    }
}

pub fn even_link(stream: PacketStream<i32>) -> Link<i32> {
    ClassifyLink::new()
        .ingressor(stream)
        .num_egressors(2)
        .classifier(Even::new())
        .dispatcher(Box::new(|is_even| if is_even { 0 } else { 1 }))
        .build_link()
}
