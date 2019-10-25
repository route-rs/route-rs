use crate::element::Element;
use crate::link::{Link, LinkBuilder, PacketStream};
use futures::{Async, Future, Poll};

/// Link that drops all packets ingressed.
#[derive(Default)]
pub struct DropLink<E: Element> {
    in_streams: Option<Vec<PacketStream<E::Input>>>,
}

impl<E: Element> DropLink<E> {
    pub fn new() -> Self {
        DropLink { in_streams: None }
    }

    /// Appends the ingressor to the ingressors of the blackhole.
    pub fn ingressor(self, in_stream: PacketStream<E::Input>) -> Self {
        match self.in_streams {
            None => {
                let in_streams = Some(vec![in_stream]);
                DropLink { in_streams }
            }
            Some(mut in_streams) => {
                in_streams.push(in_stream);
                DropLink {
                    in_streams: Some(in_streams),
                }
            }
        }
    }
}

impl<E: Element + 'static> LinkBuilder<E::Input, ()> for DropLink<E> {
    fn ingressors(self, ingress_streams: Vec<PacketStream<E::Input>>) -> Self {
        DropLink {
            in_streams: Some(ingress_streams),
        }
    }

    fn build_link(self) -> Link<()> {
        if self.in_streams.is_none() {
            panic!("Cannot build link! Missing input streams");
        } else {
            (
                vec![Box::new(DropIngressor::<E>::new(self.in_streams.unwrap()))],
                vec![],
            )
        }
    }
}

struct DropIngressor<E: Element> {
    in_streams: Vec<PacketStream<E::Input>>,
}

impl<E: Element> DropIngressor<E> {
    fn new(in_streams: Vec<PacketStream<E::Input>>) -> Self {
        DropIngressor { in_streams }
    }
}

impl<E: Element> Future for DropIngressor<E> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        loop {
            for in_stream in self.in_streams.iter_mut() {
                if try_ready!(in_stream.poll()).is_none() {
                    return Ok(Async::Ready(()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classifier::even_link;
    use crate::element::IdentityElement;
    use crate::utils::test::harness::run_link;
    use crate::utils::test::packet_generators::{immediate_stream, PacketIntervalGenerator};
    use core::time;

    #[test]
    #[should_panic]
    fn panics_if_no_input_stream_provided() {
        DropLink::<IdentityElement<i32>>::new().build_link();
    }

    #[test]
    fn multiple_ingressor_calls_works() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];

        let link = DropLink::<IdentityElement<i32>>::new()
            .ingressor(immediate_stream(packets.clone()))
            .ingressor(immediate_stream(packets.clone()))
            .build_link();

        run_link(link);
    }

    #[test]
    fn finishes() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];

        let link = DropLink::<IdentityElement<i32>>::new()
            .ingressor(immediate_stream(packets.clone()))
            .build_link();

        let results = run_link(link);
        assert!(results.is_empty());
    }

    #[test]
    fn finishes_with_wait() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9];
        let packet_generator = PacketIntervalGenerator::new(
            time::Duration::from_millis(10),
            packets.clone().into_iter(),
        );

        let link = DropLink::<IdentityElement<i32>>::new()
            .ingressor(Box::new(packet_generator))
            .build_link();

        let results = run_link(link);
        assert!(results.is_empty());
    }

    #[test]
    fn drops_odd_packets() {
        let packet_generator = immediate_stream(vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9]);

        let (mut runnables, mut egressors) = even_link(packet_generator);

        let (mut drop_runnables, _) = DropLink::<IdentityElement<i32>>::new()
            .ingressor(egressors.pop().unwrap())
            .build_link();

        runnables.append(&mut drop_runnables);

        let link = (runnables, vec![egressors.pop().unwrap()]);
        let results: Vec<Vec<i32>> = run_link(link);
        assert_eq!(results[0], vec![0, 2, 420, 4, 6, 8]);
    }
}
