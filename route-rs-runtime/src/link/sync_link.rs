use crate::element::Element;
use crate::link::{ElementLinkBuilder, Link, LinkBuilder, PacketStream};
use futures::{Async, Poll, Stream};

#[derive(Default)]
pub struct SyncLinkBuilder<E: Element> {
    in_streams: Option<Vec<PacketStream<E::Input>>>,
    element: Option<E>,
}

impl<E: Element> SyncLinkBuilder<E> {
    pub fn new() -> Self {
        SyncLinkBuilder {
            in_streams: None,
            element: None,
        }
    }
}

/// Although `Link` allows an arbitrary number of ingressors and egressors, `SyncLink`
/// may only have one ingress and egress stream since it lacks some kind of queue
/// storage. In the future we might decide to restrict the interface for this link
/// for clearer intent.
impl<E: Element + Send + 'static> LinkBuilder<E::Input, E::Output> for SyncLinkBuilder<E> {
    fn ingressors(self, in_streams: Vec<PacketStream<E::Input>>) -> Self {
        assert_eq!(in_streams.len(), 1, "SyncLink may only take 1 input stream");

        SyncLinkBuilder {
            in_streams: Some(in_streams),
            element: self.element,
        }
    }

    fn build_link(self) -> Link<E::Output> {
        if self.in_streams.is_none() {
            panic!("Cannot build link! Missing input streams");
        } else if self.element.is_none() {
            panic!("Cannot build link! Missing element");
        } else {
            let processor = SyncProcessor::new(self.in_streams.unwrap(), self.element.unwrap());
            (vec![], vec![Box::new(processor)])
        }
    }
}

impl<E: Element + Send + 'static> ElementLinkBuilder<E> for SyncLinkBuilder<E> {
    fn element(self, element: E) -> Self {
        SyncLinkBuilder {
            in_streams: self.in_streams,
            element: Some(element),
        }
    }
}

/// The single egressor of SyncLink
struct SyncProcessor<E: Element> {
    in_streams: Vec<PacketStream<E::Input>>,
    element: E,
}

impl<E: Element> SyncProcessor<E> {
    fn new(in_streams: Vec<PacketStream<E::Input>>, element: E) -> Self {
        SyncProcessor {
            in_streams,
            element,
        }
    }
}

impl<E: Element> Stream for SyncProcessor<E> {
    type Item = E::Output;
    type Error = ();

    /// Intro to `Stream`s:
    /// 4 cases: `Async::Ready(Some)`, `Async::Ready(None)`, `Async::NotReady`, `Err`
    ///
    /// `Async::Ready(Some)`: We have a packet ready to process from the upstream element.
    /// It's passed to our core's process function for... processing
    ///
    /// `Async::Ready(None)`: The input_stream doesn't have anymore input. Semantically,
    /// it's like an iterator has exhausted it's input. We should return `Ok(Async::Ready(None))`
    /// to signify to our downstream components that there's no more input to process.
    /// Our Elements should rarely return `Async::Ready(None)` since it will effectively
    /// kill the Stream chain.
    ///
    /// `Async::NotReady`: There is more input for us to process, but we can't make any more
    /// progress right now. The contract for Streams asks us to register with a Reactor so we
    /// will be woken up again by an Executor, but we will be relying on Tokio to do that for us.
    /// This case is handled by the `try_ready!` macro, which will automatically return
    /// `Ok(Async::NotReady)` if the input stream gives us NotReady.
    ///
    /// `Err`: is also handled by the `try_ready!` macro.
    ///
    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        loop {
            for ingress_stream in self.in_streams.iter_mut() {
                let input_packet_option: Option<E::Input> = try_ready!(ingress_stream.poll());
                match input_packet_option {
                    None => return Ok(Async::Ready(None)),
                    Some(input_packet) => {
                        // if `element.process` returns None, do nothing, loop around and try polling again.
                        if let Some(output_packet) = self.element.process(input_packet) {
                            return Ok(Async::Ready(Some(output_packet)));
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{DropElement, IdentityElement, TransformElement};
    use crate::utils::test::packet_collectors::ExhaustiveCollector;
    use crate::utils::test::packet_generators::{immediate_stream, PacketIntervalGenerator};
    use core::time;

    #[test]
    #[should_panic]
    fn panics_when_built_without_input_streams() {
        let identity_element: IdentityElement<i32> = IdentityElement::new();

        SyncLinkBuilder::new()
            .element(identity_element)
            .build_link();
    }

    #[test]
    #[should_panic]
    fn panics_when_built_without_element() {
        let packets: Vec<i32> = vec![];
        let packet_generator: PacketStream<i32> = immediate_stream(packets.clone());

        SyncLinkBuilder::<IdentityElement<i32>>::new()
            .ingressors(vec![Box::new(packet_generator)])
            .build_link();
    }

    #[test]
    fn builder_methods_work_in_any_order() {
        let packets: Vec<i32> = vec![];

        let packet_generator0 = immediate_stream(packets.clone());
        let identity_element0 = IdentityElement::new();

        SyncLinkBuilder::new()
            .ingressors(vec![Box::new(packet_generator0)])
            .element(identity_element0)
            .build_link();

        let packet_generator1 = immediate_stream(packets.clone());
        let identity_element1 = IdentityElement::new();

        SyncLinkBuilder::new()
            .element(identity_element1)
            .ingressors(vec![Box::new(packet_generator1)])
            .build_link();
    }

    #[test]
    fn sync_link() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = immediate_stream(packets.clone());

        let identity_element = IdentityElement::new();

        let (_, mut identity_egressors) = SyncLinkBuilder::new()
            .ingressors(vec![Box::new(packet_generator)])
            .element(identity_element)
            .build_link();

        let (s, r) = crossbeam::crossbeam_channel::unbounded();
        let consumer = ExhaustiveCollector::new(1, identity_egressors.remove(0), s);

        tokio::run(consumer);

        let router_output: Vec<_> = r.iter().collect();
        assert_eq!(router_output, packets);
    }

    #[test]
    fn wait_between_packets() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = PacketIntervalGenerator::new(
            time::Duration::from_millis(10),
            packets.clone().into_iter(),
        );

        let identity_element = IdentityElement::new();

        let (_, mut identity_egressors) = SyncLinkBuilder::new()
            .ingressors(vec![Box::new(packet_generator)])
            .element(identity_element)
            .build_link();

        let (s, r) = crossbeam::crossbeam_channel::unbounded();
        let consumer = ExhaustiveCollector::new(1, identity_egressors.remove(0), s);

        tokio::run(consumer);

        let router_output: Vec<_> = r.iter().collect();
        assert_eq!(router_output, packets);
    }

    #[test]
    fn transform_element() {
        let packets = "route-rs".chars();
        let packet_generator = immediate_stream(packets.clone());

        let transform_element = TransformElement::new();

        let (_, mut transform_egressors) = SyncLinkBuilder::new()
            .ingressors(vec![Box::new(packet_generator)])
            .element(transform_element)
            .build_link();

        let (s, r) = crossbeam::crossbeam_channel::unbounded();
        let consumer = ExhaustiveCollector::new(1, transform_egressors.remove(0), s);

        tokio::run(consumer);

        let router_output: Vec<u32> = r.iter().collect();
        let expected_output: Vec<u32> = packets.map(|p| p.into()).collect();
        assert_eq!(router_output, expected_output);
    }

    #[test]
    fn drop_element() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = immediate_stream(packets.clone());

        let drop_element = DropElement::new();

        let (_, mut drop_egressors) = SyncLinkBuilder::new()
            .ingressors(vec![Box::new(packet_generator)])
            .element(drop_element)
            .build_link();

        let (s, r) = crossbeam::crossbeam_channel::unbounded();
        let consumer = ExhaustiveCollector::new(1, drop_egressors.remove(0), s);

        tokio::run(consumer);

        let router_output: Vec<u32> = r.iter().collect();
        assert_eq!(router_output, []);
    }
}
