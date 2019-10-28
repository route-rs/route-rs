use crate::processor::Processor;
use rand::distributions::{Bernoulli, Distribution};
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::marker::PhantomData;

/// DropProcessor
/// Drops packets with weighted randomness.
pub struct Drop<A: Send + Clone> {
    phantom: PhantomData<A>,
    bernoulli: Bernoulli,
    rng: StdRng,
}

impl<A: Send + Clone> Drop<A> {
    pub fn new() -> Self {
        Drop {
            phantom: PhantomData,
            bernoulli: Bernoulli::new(1.0).unwrap(),
            rng: StdRng::from_entropy(),
        }
    }

    pub fn drop_chance(self, chance: f64) -> Self {
        assert!(chance >= 0.0, "drop_chance must be positive");
        assert!(
            chance <= 1.0,
            "drop_chance must be less than or equal to 1.0"
        );
        Drop {
            phantom: self.phantom,
            bernoulli: Bernoulli::new(chance).unwrap(),
            rng: self.rng,
        }
    }

    pub fn seed(self, int_seed: u64) -> Self {
        Drop {
            phantom: self.phantom,
            bernoulli: self.bernoulli,
            rng: StdRng::seed_from_u64(int_seed),
        }
    }
}

impl<A: Send + Clone> Processor for Drop<A> {
    type Input = A;
    type Output = A;

    fn process(&mut self, packet: Self::Input) -> Option<Self::Output> {
        if self.bernoulli.sample(&mut self.rng) {
            None
        } else {
            Some(packet)
        }
    }
}

impl<A: Send + Clone> Default for Drop<A> {
    fn default() -> Self {
        Self::new()
    }
}
