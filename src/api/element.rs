use crate::api::ElementStream;
use futures::{Async, Poll, Stream};

pub trait Element {
    type Input: Sized;
    type Output: Sized;

    fn process(&mut self, packet: Self::Input) -> Self::Output;
}

pub struct ElementLink<E: Element> {
    input_stream: ElementStream<E::Input>,
    element: E,
}

impl<E: Element> ElementLink<E> {
    pub fn new(input_stream: ElementStream<E::Input>, element: E) -> Self {
        ElementLink {
            input_stream,
            element,
        }
    }
}

impl<E: Element> Stream for ElementLink<E> {
    type Item = E::Output;
    type Error = ();

    /*
    4 cases: Async::Ready(Some), Async::Ready(None), Async::NotReady, Err

    Async::Ready(Some): We have a packet ready to process from the upstream element. It's passed to
    our core's process function for... processing

    Async::Ready(None): The input_stream doesn't have anymore input. Semantically, it's like an
    iterator has exhausted it's input. We should return "Ok(Async::Ready(None))" to signify to our
    downstream components that there's no more input to process. Our Elements should rarely
    return "Async::Ready(None)" since it will effectively kill the Stream chain.

    Async::NotReady: There is more input for us to process, but we can't make any more progress right
    now. The contract for Streams asks us to register with a Reactor so we will be woken up again by
    an Executor, but we will be relying on Tokio to do that for us. This case is handled by the
    "try_ready!" macro, which will automatically return "Ok(Async::NotReady)" if the input stream
    gives us NotReady.

    Err: is also handled by the "try_ready!" macro.
    */
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let input_packet_option: Option<E::Input> = try_ready!(self.input_stream.poll());
        match input_packet_option {
            None => Ok(Async::Ready(None)),
            Some(input_packet) => {
                let output_packet: E::Output = self.element.process(input_packet);
                Ok(Async::Ready(Some(output_packet)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test::packet_collectors::ExhaustiveCollector;
    use crate::utils::test::packet_generators::{immediate_stream, PacketIntervalGenerator};
    use core::time;

    #[allow(dead_code)]
    struct IdentityElement {
        id: i32,
    }

    impl Element for IdentityElement {
        type Input = i32;
        type Output = i32;

        fn process(&mut self, packet: Self::Input) -> Self::Output {
            packet
        }
    }

    #[test]
    fn one_sync_element() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = immediate_stream(packets.clone());

        let elem1 = IdentityElement { id: 0 };
        let elem2 = IdentityElement { id: 1 };

        let elem1_link = ElementLink::new(Box::new(packet_generator), elem1);
        let elem2_link = ElementLink::new(Box::new(elem1_link), elem2);

        let (s, r) = crossbeam::crossbeam_channel::unbounded();
        let consumer = ExhaustiveCollector::new(1, Box::new(elem2_link), s);

        tokio::run(consumer);

        let router_output: Vec<_> = r.iter().collect();
        assert_eq!(router_output, packets);
    }

    #[test]
    fn one_sync_element_wait_between_packets() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = PacketIntervalGenerator::new(
            time::Duration::from_millis(100),
            packets.clone().into_iter(),
        );

        let elem1 = IdentityElement { id: 0 };
        let elem2 = IdentityElement { id: 1 };

        let elem1_link = ElementLink::new(Box::new(packet_generator), elem1);
        let elem2_link = ElementLink::new(Box::new(elem1_link), elem2);

        let (s, r) = crossbeam::crossbeam_channel::unbounded();
        let consumer = ExhaustiveCollector::new(1, Box::new(elem2_link), s);

        tokio::run(consumer);

        let router_output: Vec<_> = r.iter().collect();
        assert_eq!(router_output, packets);
    }
}
