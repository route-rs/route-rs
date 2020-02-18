use crate::link::{EgressLinkBuilder, IngressLinkBuilder, Link, LinkBuilder, TokioRunnable};
use crate::utils::test::packet_collectors::ExhaustiveCollector;
use crossbeam::crossbeam_channel;
use std::fmt::Debug;
use tokio::runtime;
use tokio::task::JoinHandle;

/// Runner is a user facing helper function for running the constructed router.
///
/// Its only argument is a function pointer that takes no arguments and returns a Link type. This
/// master link should contain all the runnables and outputs of the router, which in turn allows
/// this function to initialize and start the Router.
///
/// In general, the `Link` returned by the router should contain only TokioRunnables and no PacketStreams,
/// since production routers are self contained with all their output going to links that push the packets
/// out the routers physical ports.
///
/// However, if the link your `link_builder()` fn provides does return egressors, this function will automatically
/// hook them into `PacketCollector` links, and whatever packets come out of those egressors will be returned to you once
/// the router completes operation and joins. In a production router, the router likely never stops running so
/// nothing will ever get returned.  Use this functionality only for testing.  
pub fn runner<OutputPacket: Debug + Send + Clone + 'static>(
    link_builder: fn() -> Link<OutputPacket>,
) -> Vec<Vec<OutputPacket>> {
    let mut runtime = runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        let (mut runnables, egressors) = link_builder();

        let (mut consumers, receivers): (
            Vec<TokioRunnable>,
            Vec<crossbeam_channel::Receiver<OutputPacket>>,
        ) = egressors
            .into_iter()
            .map(|egressor| {
                let (s, r) = crossbeam_channel::unbounded::<OutputPacket>();
                // TODO: Do we care about consumer IDs? Are they helpful to debug test examples?
                let consumer: TokioRunnable = Box::new(ExhaustiveCollector::new(0, egressor, s));
                (consumer, r)
            })
            .unzip();

        runnables.append(&mut consumers);

        let handles: Vec<JoinHandle<()>> = runnables.into_iter().map(tokio::spawn).collect();
        // 🏃💨💨
        for handle in handles {
            handle.await.unwrap();
        }

        receivers
            .into_iter()
            .map(|receiver| receiver.iter().collect())
            .collect()
    })
}


/// `build_and_run_router` is designed to assemble ingress and egress links for integrating to a
/// variety of I/O implementations (e.g. af_packet, pcap, etc). This allows us to generically
/// hook up a router for testing purposes without changing the internals.
pub fn build_and_run_router<
    IngressPacket,
    IngressLink: IngressLinkBuilder<IngressPacket>,
    EgressPacket,
    EgressLink: EgressLinkBuilder<EgressPacket>,
    Router: LinkBuilder<IngressPacket, EgressPacket>,
>(
    ingress_receivers: Vec<IngressLink::Receiver>,
    egress_senders: Vec<EgressLink::Sender>,
    router: Router,
) {
    let mut runtime = runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        // Create Ingress Links from the receivers
        let ingressors = ingress_receivers
            .into_iter()
            .map(|r| IngressLink::new().channel(r).build_link())
            .collect::<Vec<Link<IngressPacket>>>();

        // Collapse runnables and egressors so that we can pass them along
        let mut ingress_runnables = vec![];
        let mut ingress_egressors = vec![];
        ingressors
            .into_iter()
            .for_each(|(mut runnables, mut egressors)| {
                ingress_runnables.append(&mut runnables);
                ingress_egressors.append(&mut egressors);
            });

        // Set up the actual business logic router
        let (mut router_runnables, router_egressors) =
            router.ingressors(ingress_egressors).build_link();

        // We know there should be a 1:1 correspondence between egressors from the router and senders,
        // so we can zip them together and then build the egress links
        let egressors = router_egressors
            .into_iter()
            .zip(egress_senders.into_iter())
            .map(|(egressor, sender)| {
                EgressLink::new()
                    .ingressor(egressor)
                    .channel(sender)
                    .build_link()
            })
            .collect::<Vec<Link<()>>>();

        // Egress links don't have any egressors, so all we need to do is pick up the runnables
        let mut egress_runnables = vec![];
        egressors
            .into_iter()
            .for_each(|(mut runnables, _egressors)| {
                egress_runnables.append(&mut runnables);
            });

        let mut all_runnables = vec![];
        all_runnables.append(&mut ingress_runnables);
        all_runnables.append(&mut router_runnables);
        all_runnables.append(&mut egress_runnables);

        let handles: Vec<JoinHandle<()>> = all_runnables.into_iter().map(tokio::spawn).collect();
        // 🏃💨💨
        for handle in handles {
            handle.await.unwrap();
        }
    })
}
