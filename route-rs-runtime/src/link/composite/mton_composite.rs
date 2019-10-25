use crate::link::{
    primitive::{ForkLink, JoinLink},
    Link, LinkBuilder, PacketStream,
};

#[derive(Default)]
pub struct MtoNComposite<Packet: Sized + Send + Clone> {
    in_streams: Option<Vec<PacketStream<Packet>>>,
    join_queue_capacity: usize,
    fork_queue_capacity: usize,
    num_egressors: Option<usize>,
}

impl<Packet: Sized + Send + Clone> MtoNComposite<Packet> {
    pub fn new() -> Self {
        MtoNComposite {
            in_streams: None,
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
                "queue_capacity: {}, must be in range 1..=1000",
                queue_capacity
            )
        );

        MtoNComposite {
            in_streams: self.in_streams,
            join_queue_capacity: queue_capacity,
            fork_queue_capacity: self.fork_queue_capacity,
            num_egressors: self.num_egressors,
        }
    }

    /// Changes tee_queue_capcity, default value is 10.
    /// Valid range is 1..=1000
    pub fn tee_queue_capacity(self, queue_capacity: usize) -> Self {
        assert!(
            (1..=1000).contains(&queue_capacity),
            format!(
                "queue_capacity: {}, must be in range 1..=1000",
                queue_capacity
            )
        );

        MtoNComposite {
            in_streams: self.in_streams,
            join_queue_capacity: self.join_queue_capacity,
            fork_queue_capacity: queue_capacity,
            num_egressors: self.num_egressors,
        }
    }

    pub fn num_egressors(self, num_egressors: usize) -> Self {
        assert!(
            (1..=1000).contains(&num_egressors),
            format!(
                "num_egressors: {}, must be in range 1..=1000",
                num_egressors
            )
        );

        MtoNComposite {
            in_streams: self.in_streams,
            join_queue_capacity: self.join_queue_capacity,
            fork_queue_capacity: self.fork_queue_capacity,
            num_egressors: Some(num_egressors),
        }
    }
}

impl<Packet: Sized + Send + Clone + 'static> LinkBuilder<Packet, Packet> for MtoNComposite<Packet> {
    fn ingressors(self, in_streams: Vec<PacketStream<Packet>>) -> Self {
        assert!(
            (1..=1000).contains(&in_streams.len()),
            format!(
                "Input streams: {} must be in range 1..=1000",
                in_streams.len()
            )
        );
        MtoNComposite {
            in_streams: Some(in_streams),
            join_queue_capacity: self.join_queue_capacity,
            fork_queue_capacity: self.fork_queue_capacity,
            num_egressors: self.num_egressors,
        }
    }

    fn build_link(self) -> Link<Packet> {
        if self.in_streams.is_none() {
            panic!("Cannot build link! Missing input stream");
        } else if self.num_egressors.is_none() {
            panic!("Cannot build link! Missing number of num_egressors");
        } else {
            let (mut join_runnables, join_egressors) = JoinLink::new()
                .ingressors(self.in_streams.unwrap())
                .queue_capacity(self.join_queue_capacity)
                .build_link();
            let (mut fork_link_runnables, fork_link_egressors) = ForkLink::new()
                .ingressors(join_egressors)
                .queue_capacity(self.fork_queue_capacity)
                .num_egressors(self.num_egressors.unwrap())
                .build_link();
            fork_link_runnables.append(&mut join_runnables);
            (fork_link_runnables, fork_link_egressors)
        }
    }
}

#[cfg(test)]
#[allow(dead_code)]
mod tests {
    use super::*;
    use crate::link::LinkBuilder;
    use crate::utils::test::packet_generators::immediate_stream;

    use crate::utils::test::harness::run_link;

    #[test]
    fn mton_composite() {
        let packets = vec![0, 1, 2, 420, 1337, 3, 4, 5, 6, 7, 8, 9, 11];

        let mut input_streams: Vec<PacketStream<usize>> = Vec::new();
        input_streams.push(immediate_stream(packets.clone()));
        input_streams.push(immediate_stream(packets.clone()));

        let link = MtoNComposite::new()
            .num_egressors(5)
            .ingressors(input_streams)
            .build_link();

        let results = run_link(link);
        assert_eq!(results[0].len(), packets.len() * 2);
        assert_eq!(results[1].len(), packets.len() * 2);
        assert_eq!(results[2].len(), packets.len() * 2);
        assert_eq!(results[3].len(), packets.len() * 2);
        assert_eq!(results[4].len(), packets.len() * 2);
    }
}
