use crate::link::{Link, TokioRunnable};
use crate::utils::test::packet_collectors::ExhaustiveCollector;
use crossbeam::crossbeam_channel;
use std::fmt::Debug;
use tokio::{
    runtime,
    time::{timeout, Duration},
};

/// The utils::test::harness module should be able to help Link authors abstract away the
/// complexity of dealing with the Tokio runtime. Tests should be expressed with the
/// typical "Given, When, Then" structure (https://martinfowler.com/bliki/GivenWhenThen.html).

/// "Given" refers to the state of the world before the behavior under test runs.
/// The initial context needed to test a link is the incoming packet stream(s).

/// "When" refers to the behavior under test.
/// This is the Link configuration we're trying to test.

/// "Then" refers to the expected changes to the system due to executing the behavior under test
/// against the initial context.
/// This is the state of packet collectors after the input has been exhausted and run through
/// our Link under test.

/// Since the initial context of "a link's input streams" are coupled to the construction of Links,
/// let's just expose a function that takes a connected Link, runs it's runnables and collectors
/// through Tokio, and extracts the output packets into vectors representing egress streams.

pub fn initialize_runtime() -> runtime::Runtime {
    runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .build()
        .unwrap()
}

/// test_link
///
/// A testing harness to be used to run a link you wish to test. It takes a link to run
/// and a max_test_duration option. If you have a link that should complete, ie it tears
/// down, then put a None for Duration. If you have a link that is not expected to end
///
pub async fn test_link<OutputPacket: Debug + Send + Clone + 'static>(
    link: Link<OutputPacket>,
    max_test_duration: Option<Duration>,
) -> Vec<Vec<OutputPacket>> {
    let (mut runnables, egressors) = link;

    // generate consumers for each egressors
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

    // gather link's runnables and tokio-driven consumers into one collection
    runnables.append(&mut consumers);

    // 🏃💨💨
    match max_test_duration {
        None => {
            spawn_runnables(runnables).await;
        }
        Some(duration) => {
            let _res = timeout(duration, spawn_runnables(runnables)).await;
        }
    }

    // collect packets from consumers via receiver channels
    receivers
        .into_iter()
        .map(|receiver| receiver.try_iter().collect())
        .collect()
}

async fn spawn_runnables(runnables: Vec<TokioRunnable>) {
    let mut handles = vec![];
    for runnable in runnables {
        handles.push(tokio::spawn(runnable));
    }
    await_handles(handles).await;
}

async fn await_handles(handles: Vec<tokio::task::JoinHandle<()>>) {
    for handle in handles {
        handle.await.unwrap();
    }
}
