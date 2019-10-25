use crate::classifier::Classifier;
use crate::link::{primitive::ClassifyLink, Link, LinkBuilder, PacketStream};

pub enum FizzBuzzVariant {
    FizzBuzz,
    Fizz,
    Buzz,
    None,
}

#[derive(Default)]
pub struct FizzBuzz {}

impl FizzBuzz {
    pub fn new() -> Self {
        FizzBuzz {}
    }
}

impl Classifier for FizzBuzz {
    type Packet = i32;
    type Class = FizzBuzzVariant;

    fn classify(&self, packet: &Self::Packet) -> Self::Class {
        if packet % 3 == 0 && packet % 5 == 0 {
            FizzBuzzVariant::FizzBuzz
        } else if packet % 3 == 0 {
            FizzBuzzVariant::Fizz
        } else if packet % 5 == 0 {
            FizzBuzzVariant::Buzz
        } else {
            FizzBuzzVariant::None
        }
    }
}

pub fn fizz_buzz_link(stream: PacketStream<i32>) -> Link<i32> {
    ClassifyLink::new()
        .ingressor(stream)
        .num_egressors(4)
        .classifier(FizzBuzz::new())
        .dispatcher(Box::new(|fb| match fb {
            FizzBuzzVariant::FizzBuzz => 0,
            FizzBuzzVariant::Fizz => 1,
            FizzBuzzVariant::Buzz => 2,
            FizzBuzzVariant::None => 3,
        }))
        .build_link()
}
