#[macro_use]
extern crate futures;
extern crate tokio;
extern crate crossbeam;

pub mod api;
mod utils;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AsyncElementLink, AsyncElement};
    use crate::api::element::{ElementLink, Element};
    use crate::utils::test::packet_generators::{immediate_stream, LinearIntervalGenerator, PacketIntervalGenerator};
    use crate::utils::test::packet_collectors::{ExhaustiveDrain, ExhaustiveCollector};
    use core::time;
    use crossbeam::crossbeam_channel;

    use futures::future::lazy;

    #[allow(dead_code)]
    struct IdentityElement {
        id: i32
    }

    impl Element for IdentityElement {
        type Input = i32;
        type Output = i32;

        fn process(&mut self, packet: Self::Input) -> Self::Output {
            packet
        }
    }

    #[allow(dead_code)]
    struct AsyncIdentityElement {
        id: i32
    }

    impl AsyncElement for AsyncIdentityElement {
        type Input = i32;
        type Output = i32;

        fn process(&mut self, packet: Self::Input) -> Self::Output {
            packet
        }
    }


    #[test]
    fn one_async_element_immediate_yield() {
        let default_channel_size = 10;
        let packet_generator = immediate_stream(0..=20);


        let elem0 = AsyncIdentityElement { id: 0 };

        let elem0_link = AsyncElementLink::new(Box::new(packet_generator), elem0, default_channel_size);

        let elem0_drain = elem0_link.consumer;
        let elem0_consumer = ExhaustiveDrain::new(1, Box::new(elem0_link.provider));

        tokio::run(lazy (|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem0_consumer);
            Ok(())
        }));
    }

    #[test]
    fn two_async_elements_immediate_yield() {
        let default_channel_size = 10;
        let packet_generator = immediate_stream(0..=20);

        let elem0 = AsyncIdentityElement { id: 0 };
        let elem1 = AsyncIdentityElement { id: 1 };

        let elem0_link = AsyncElementLink::new(Box::new(packet_generator), elem0, default_channel_size);
        let elem1_link = AsyncElementLink::new(Box::new(elem0_link.provider), elem1, default_channel_size);

        let elem0_drain = elem0_link.consumer;
        let elem1_drain = elem1_link.consumer;

        let elem1_consumer = ExhaustiveDrain::new(1, Box::new(elem1_link.provider));

        tokio::run(lazy (|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem1_drain);
            tokio::spawn(elem1_consumer);
            Ok(())
        }));
    }

    #[test]
    fn series_sync_and_async_immediate_yield() {
        let default_channel_size = 10;
        let packet_generator = immediate_stream(0..=20);

        let elem0 = IdentityElement { id: 0 };
        let elem1 = AsyncIdentityElement { id: 1 };
        let elem2 = IdentityElement { id: 2 };
        let elem3 = AsyncIdentityElement { id: 3 };

        let elem0_link = ElementLink::new(Box::new(packet_generator), elem0);
        let elem1_link = AsyncElementLink::new(Box::new(elem0_link), elem1, default_channel_size);
        let elem2_link = ElementLink::new(Box::new(elem1_link.provider), elem2);
        let elem3_link = AsyncElementLink::new(Box::new(elem2_link), elem3, default_channel_size);

        let elem1_drain = elem1_link.consumer;
        let elem3_drain = elem3_link.consumer;

        let elem3_consumer = ExhaustiveDrain::new(0, Box::new(elem3_link.provider));

        tokio::run(lazy (|| {
            tokio::spawn(elem1_drain);
            tokio::spawn(elem3_drain); 
            tokio::spawn(elem3_consumer);
            Ok(())
        }));
    }

        #[test]
    fn one_async_element_interval_yield() {
        let default_channel_size = 10;
        let packet_generator = LinearIntervalGenerator::new(time::Duration::from_millis(100), 20);

        let elem0 = AsyncIdentityElement { id: 0 };

        let elem0_link = AsyncElementLink::new(Box::new(packet_generator), elem0, default_channel_size);

        let elem0_drain = elem0_link.consumer;
        let elem0_consumer = ExhaustiveDrain::new(0, Box::new(elem0_link.provider));

        tokio::run(lazy (|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem0_consumer);
            Ok(())
        }));
    }

    #[test]
    fn two_async_elements_interval_yield() {
        let default_channel_size = 10;
        let packet_generator = LinearIntervalGenerator::new(time::Duration::from_millis(100), 20);

        let elem0 = AsyncIdentityElement { id: 0 };
        let elem1 = AsyncIdentityElement { id: 1 };

        let elem0_link = AsyncElementLink::new(Box::new(packet_generator), elem0, default_channel_size);
        let elem1_link = AsyncElementLink::new(Box::new(elem0_link.provider), elem1, default_channel_size);

        let elem0_drain = elem0_link.consumer;
        let elem1_drain = elem1_link.consumer;

        let elem1_consumer = ExhaustiveDrain::new(0, Box::new(elem1_link.provider));

        tokio::run(lazy (|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem1_drain);
            tokio::spawn(elem1_consumer);
            Ok(())
        }));
    }

    #[test]
    fn series_sync_and_async_interval_yield() {
        let default_channel_size = 10;
        let packet_generator = LinearIntervalGenerator::new(time::Duration::from_millis(100), 20);

        let elem0 = IdentityElement { id: 0 };
        let elem1 = AsyncIdentityElement { id: 1 };
        let elem2 = IdentityElement { id: 2 };
        let elem3 = AsyncIdentityElement { id: 3 };

        let elem0_link = ElementLink::new(Box::new(packet_generator), elem0);
        let elem1_link = AsyncElementLink::new(Box::new(elem0_link), elem1, default_channel_size);
        let elem2_link = ElementLink::new(Box::new(elem1_link.provider), elem2);
        let elem3_link = AsyncElementLink::new(Box::new(elem2_link), elem3, default_channel_size);

        let elem1_drain = elem1_link.consumer;
        let elem3_drain = elem3_link.consumer;

        let elem3_consumer = ExhaustiveDrain::new(2, Box::new(elem3_link.provider));

        tokio::run(lazy (|| {
            tokio::spawn(elem1_drain);
            tokio::spawn(elem3_drain); 
            tokio::spawn(elem3_consumer);
            Ok(())
        }));
    }

    #[test]
    fn one_async_element_collected_yield() {
        let default_channel_size = 10;
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7];
        let packet_generator = PacketIntervalGenerator::new(time::Duration::from_millis(100), packets.clone().into_iter());

        let elem0 = AsyncIdentityElement { id: 0 };

        let elem0_link = AsyncElementLink::new(Box::new(packet_generator), elem0, default_channel_size);

        let (s, r) = crossbeam_channel::unbounded();
        let elem0_drain = elem0_link.consumer;
        let elem0_collector = ExhaustiveCollector::new(0, Box::new(elem0_link.provider), s);

        tokio::run(lazy (|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem0_collector);
            Ok(())
        }));

        let router_output: Vec<_> = r.iter().collect();
        assert_eq!(router_output, packets);
    }

    #[test]
    fn two_async_elements_collected_yield() {
        let default_channel_size = 10;
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7];
        let packet_generator = PacketIntervalGenerator::new(time::Duration::from_millis(100), packets.clone().into_iter());

        let elem0 = AsyncIdentityElement { id: 0 };
        let elem1 = AsyncIdentityElement { id: 1 };

        let elem0_link = AsyncElementLink::new(Box::new(packet_generator), elem0, default_channel_size);
        let elem1_link = AsyncElementLink::new(Box::new(elem0_link.provider), elem1, default_channel_size);

        let elem0_drain = elem0_link.consumer;
        let elem1_drain = elem1_link.consumer;

        let (s, r) = crossbeam_channel::unbounded();
        let elem1_collector = ExhaustiveCollector::new(0, Box::new(elem1_link.provider), s);

        tokio::run(lazy (|| {
            tokio::spawn(elem0_drain);
            tokio::spawn(elem1_drain);
            tokio::spawn(elem1_collector);
            Ok(())
        }));

        let router_output: Vec<_> = r.iter().collect();
        assert_eq!(router_output, packets);
    }

    #[test]
    fn series_sync_and_async_collected_yield() {
        let default_channel_size = 10;
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7];
        let packet_generator = PacketIntervalGenerator::new(time::Duration::from_millis(100), packets.clone().into_iter());

        let elem0 = IdentityElement { id: 0 };
        let elem1 = AsyncIdentityElement { id: 1 };
        let elem2 = IdentityElement { id: 2 };
        let elem3 = AsyncIdentityElement { id: 3 };

        let elem0_link = ElementLink::new(Box::new(packet_generator), elem0);
        let elem1_link = AsyncElementLink::new(Box::new(elem0_link), elem1, default_channel_size);
        let elem2_link = ElementLink::new(Box::new(elem1_link.provider), elem2);
        let elem3_link = AsyncElementLink::new(Box::new(elem2_link), elem3, default_channel_size);

        let elem1_drain = elem1_link.consumer;
        let elem3_drain = elem3_link.consumer;

        let (s, r) = crossbeam_channel::unbounded();
        let elem3_collector = ExhaustiveCollector::new(0, Box::new(elem3_link.provider), s);

        tokio::run(lazy (|| {
            tokio::spawn(elem1_drain);
            tokio::spawn(elem3_drain); 
            tokio::spawn(elem3_collector);
            Ok(())
        }));

        let router_output: Vec<_> = r.iter().collect();
        assert_eq!(router_output, packets);
    }
}
