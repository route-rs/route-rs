use route_rs_packets::EthernetFrame;
use route_rs_runtime::link::primitive::{InputChannelLink, OutputChannelLink};
use route_rs_runtime::link::{LinkBuilder, PacketStream, TokioRunnable};
use tokio::task::JoinHandle;

mod classifier;
mod processor;
mod route;

fn main() {
    let mut runtime = tokio::runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .build()
        .unwrap();

    let ingress_wan = crossbeam::channel::unbounded();
    let ingress_lan = crossbeam::channel::unbounded();
    let ingress_host = crossbeam::channel::unbounded();

    let egress_wan = crossbeam::channel::unbounded();
    let egress_lan = crossbeam::channel::unbounded();
    let egress_host = crossbeam::channel::unbounded();

    drop(ingress_wan.0);
    drop(ingress_lan.0);
    drop(ingress_host.0);

    run_route(
        vec![ingress_wan.1, ingress_lan.1, ingress_host.1],
        vec![egress_wan.0, egress_lan.0, egress_host.0],
        &mut runtime,
        route::Route::new(),
    );

    assert_eq!(egress_wan.1.len(), 0, "Egress WAN is not empty");
    assert_eq!(egress_lan.1.len(), 0, "Egress LAN is not empty");
    assert_eq!(egress_host.1.len(), 0, "Egress Host is not empty");

    println!("Congratulations! Your router successfully does nothing!")
}

// TODO: Move this to route-rs-runtime
fn run_route(
    ingressors: Vec<crossbeam::Receiver<EthernetFrame>>,
    egressors: Vec<crossbeam::Sender<EthernetFrame>>,
    runtime: &mut tokio::runtime::Runtime,
    route: route::Route,
) {
    let mut all_runnables = vec![];

    let (_, upstreams): (
        Vec<Vec<TokioRunnable>>,
        Vec<Vec<PacketStream<EthernetFrame>>>,
    ) = ingressors
        .into_iter()
        .map(|receiver| InputChannelLink::new().channel(receiver).build_link())
        .unzip();

    let (mut route_runnables, downstreams) = route
        .ingressors(upstreams.into_iter().flatten().collect())
        .build_link();

    all_runnables.append(&mut route_runnables);

    let egressor_inputs = downstreams.into_iter().zip(egressors.into_iter());

    let (egress_runnables, _): (Vec<Vec<TokioRunnable>>, Vec<Vec<PacketStream<_>>>) =
        egressor_inputs
            .map(|(ingressor, sender)| {
                OutputChannelLink::new()
                    .ingressor(ingressor)
                    .channel(sender)
                    .build_link()
            })
            .unzip();

    all_runnables.append(&mut egress_runnables.into_iter().flatten().collect());

    runtime.block_on(async {
        let handles: Vec<JoinHandle<()>> = all_runnables.into_iter().map(tokio::spawn).collect();

        for handle in handles {
            handle.await.unwrap();
        }
    })
}
