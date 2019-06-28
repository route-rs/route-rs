#[macro_use]
extern crate futures;
extern crate tokio;
extern crate crossbeam;

pub mod api;
mod utils;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{ElementLink, Element, AsyncElement};
    use crate::utils::{LinearIntervalGenerator, ExhaustiveConsumer};
    use core::time;
    use futures::Stream;

    struct TrivialElement {
        id: i32
    }

    impl Element for TrivialElement {
        type Input = i32;
        type Output = i32;

        fn process(&mut self, packet: Self::Input) -> Self::Output {
            println!("Got packet {} in core {}", packet, self.id);
            packet + 1
        }
    }

    #[test]
    fn impl_sync_element_works() {
        let packet_generator = LinearIntervalGenerator::new(time::Duration::from_millis(100), 10);

        let elem1 = TrivialElement { id: 0 };
        let elem2 = TrivialElement { id: 1 };

        // core_elem1 to! core_elem2

        let elem1_link = ElementLink::new(Box::new(packet_generator), elem1);
        let elem2_link = ElementLink::new(Box::new(elem1_link), elem2);

        let consumer = ExhaustiveConsumer::new(Box::new(elem2_link));

        tokio::run(consumer);
    }
}
