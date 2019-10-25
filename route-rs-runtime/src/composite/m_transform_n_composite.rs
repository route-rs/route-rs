use crate::element::Element;
use crate::link::{
    ElementLinkBuilder, ForkLink, JoinLink, Link, LinkBuilder, PacketStream, ProcessLink,
};

#[derive(Default)]
pub struct MtransformNComposite<E: Element + Send> {
    in_streams: Option<Vec<PacketStream<E::Input>>>,
    element: Option<E>,
    join_queue_capacity: usize,
    fork_queue_capacity: usize,
    num_egressors: Option<usize>,
}

impl<E: Element + Send> MtransformNComposite<E> {
    pub fn new() -> Self {
        MtransformNComposite {
            in_streams: None,
            element: None,
            join_queue_capacity: 10,
            fork_queue_capacity: 10,
            num_egressors: None,
        }
    }

    /// Changes join_queue_capcity, default value is 10.
    /// Valid range is 1..=1000
    pub fn join_queue_capacity(self, queue_capacity: usize) -> Self {
        assert!(
            (1..=1000).contains(&queue_capacity),
            format!(
                "join_queue_capacity: {} must be in range 1..=1000",
                queue_capacity
            )
        );

        MtransformNComposite {
            in_streams: self.in_streams,
            element: self.element,
            join_queue_capacity: queue_capacity,
            fork_queue_capacity: self.fork_queue_capacity,
            num_egressors: self.num_egressors,
        }
    }

    /// Changes tee_queue_capcity, default value is 10.
    /// Valid range is 1..=1000
    pub fn fork_queue_capacity(self, queue_capacity: usize) -> Self {
        assert!(
            (1..=1000).contains(&queue_capacity),
            format!(
                "fork_queue_capacity: {} must be in range 1..=1000",
                queue_capacity
            )
        );

        MtransformNComposite {
            in_streams: self.in_streams,
            element: self.element,
            join_queue_capacity: self.join_queue_capacity,
            fork_queue_capacity: queue_capacity,
            num_egressors: self.num_egressors,
        }
    }

    pub fn num_egressors(self, num_egressors: usize) -> Self {
        assert!(
            num_egressors <= 1000,
            format!("compsite num_egressors: {} > 1000", num_egressors)
        );
        assert_ne!(num_egressors, 0, "num_egressors must be non-zero");

        MtransformNComposite {
            in_streams: self.in_streams,
            element: self.element,
            join_queue_capacity: self.join_queue_capacity,
            fork_queue_capacity: self.fork_queue_capacity,
            num_egressors: Some(num_egressors),
        }
    }
}

impl<E: Element + Send + 'static> LinkBuilder<E::Input, E::Output> for MtransformNComposite<E> {
    fn ingressors(self, in_streams: Vec<PacketStream<E::Input>>) -> Self {
        assert!(
            in_streams.len() > 1 && in_streams.len() <= 1000,
            format!("Input streams {} not in 2..=1000", in_streams.len())
        );
        MtransformNComposite {
            in_streams: Some(in_streams),
            element: self.element,
            join_queue_capacity: self.join_queue_capacity,
            fork_queue_capacity: self.fork_queue_capacity,
            num_egressors: self.num_egressors,
        }
    }

    fn build_link(self) -> Link<E::Output> {
        if self.in_streams.is_none() {
            panic!("Cannot build link! Missing input stream");
        } else if self.num_egressors.is_none() {
            panic!("Cannot build link! Missing number of num_egressors");
        } else if self.element.is_none() {
            panic!("Cannot build link! Missing element");
        } else {
            let (mut join_runnables, join_egressors) = JoinLink::new()
                .ingressors(self.in_streams.unwrap())
                .queue_capacity(self.join_queue_capacity)
                .build_link();

            let (_, process_egressors) = ProcessLink::new()
                .ingressors(join_egressors)
                .element(self.element.unwrap())
                .build_link();

            let (mut fork_link_runnables, fork_link_egressors) = ForkLink::new()
                .ingressors(process_egressors)
                .queue_capacity(self.fork_queue_capacity)
                .num_egressors(self.num_egressors.unwrap())
                .build_link();
            fork_link_runnables.append(&mut join_runnables);

            (fork_link_runnables, fork_link_egressors)
        }
    }
}

impl<E: Element + Send + 'static> ElementLinkBuilder<E> for MtransformNComposite<E> {
    fn element(self, element: E) -> Self {
        MtransformNComposite {
            in_streams: self.in_streams,
            element: Some(element),
            join_queue_capacity: self.join_queue_capacity,
            fork_queue_capacity: self.fork_queue_capacity,
            num_egressors: self.num_egressors,
        }
    }
}

#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use super::*;
    use crate::element::TransformElement;
    use crate::link::LinkBuilder;
    use crate::utils::test::packet_generators::immediate_stream;
    use std::net::Ipv4Addr;

    use crate::utils::test::harness::run_link;

    #[test]
    fn m_transform_n_composite() {
        let packets = vec![0xDEAD_BEEF, 0xBEEF_DEAD, 0x0A00_0001, 0xFFFF_FFFF];

        let mut input_streams: Vec<PacketStream<u32>> = Vec::new();
        input_streams.push(immediate_stream(packets.clone()));
        input_streams.push(immediate_stream(packets.clone()));

        let link = MtransformNComposite::new()
            .num_egressors(5)
            .ingressors(input_streams)
            .element(TransformElement::<u32, Ipv4Addr>::new())
            .build_link();

        let results = run_link(link);
        assert_eq!(results[0].len(), packets.len() * 2);
        assert_eq!(results[1].len(), packets.len() * 2);
        assert_eq!(results[2].len(), packets.len() * 2);
        assert_eq!(results[3].len(), packets.len() * 2);
        assert_eq!(results[4].len(), packets.len() * 2);
    }
}
