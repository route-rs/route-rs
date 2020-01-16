use crate::link::{Link, TokioRunnable};
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
/// hook them into Collector links, and whatever packets come out of those egressors will be returned to you once
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
