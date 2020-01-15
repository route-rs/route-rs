use tokio::runtime;
use crate::link::{TokioRunnable};
use tokio::task::JoinHandle;

/// Runner is a user facing helper function for running the constructed router.TokioRunnable
/// 
/// Its only argument is a function pointer that takes no arguments and returns a Link type. This
/// master link should contain all the runnables and outputs of the router, which in turn allows
/// this function to initialize and start the Router.
/// 
/// In general, the `Link` returned by the router should contain only TokioRunnables and no PacketStreams,
/// since production routers are self contained with all their output going to links that push the packets
/// out the routers physical ports.   
pub fn runner(link_builder: fn() -> Vec<TokioRunnable>) {
    let mut runtime = runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async {
        let runnables = link_builder();
        let handles: Vec<JoinHandle<()>> = runnables.into_iter().map(|runnable| tokio::spawn(runnable)).collect();
        // 🏃💨💨
        for handle in handles {
            handle.await.unwrap();
        }
    });
}