use futures::{Future, Stream};

/// A simple pull based link.  It is pull based in the sense that packets are only fetched on the input
/// when a packet is requested from the output. This link does not have the abilty store packets internally,
/// so all packets that enter either immediatly leave or are dropped, as dictated by the element. Both sides of
/// this link are on the same thread, hence the label synchronous.
mod sync_link;
pub use self::sync_link::*;

/// Input packets are placed into an intermediate channel that are pulled from the output asynchronously.
/// Asynchronous in that a packets may enter and leave this link asynchronously to each other.  This link is
/// useful for creating queues in the router, buffering, and creating `Task` boundries that can be processed on
/// different threads, or even different cores. Before packets are placed into the queue to be output, they are run
/// through the element defined process function, often performing some sort of transformation.
mod queue_link;
pub use self::queue_link::*;

/// Uses element defined classifications to sort input into different channels, a good example would
/// be a flow that splits IPv4 and IPv6 packets, asynchronous.
mod classify_link;
pub use self::classify_link::*;

/// Fairly combines all inputs into a single output, asynchronous.
mod join_link;
pub use self::join_link::*;

/// Copies all input to each of its outputs, asynchronous.
mod clone_link;
pub use self::clone_link::*;

/// Drops all packets that are ingressed, asynchronous.
mod blackhole_link;
pub use self::blackhole_link::*;
use crate::element::Element;

/// All Links communicate through streams of packets. This allows them to be composable.
pub type PacketStream<Input> = Box<dyn Stream<Item = Input, Error = ()> + Send>;
/// Some Links may need to be driven by Tokio. This represents a handle to something Tokio can run.
pub type TokioRunnable = Box<dyn Future<Item = (), Error = ()> + Send>;
/// LinkBuilders build this.
pub type Link<Output> = (Vec<TokioRunnable>, Vec<PacketStream<Output>>);

/// `LinkBuilder` applies a builder pattern to create `Links`! `Links` should be created this way
/// so they can be composed together
///
/// The two type parameters, Input and Output, refer to the input and output types of the Link.
/// You can tell because the ingress/egress streams are of type `PacketStream<Input>`/`PacketStream<Output>` respectively.
pub trait LinkBuilder<Input, Output> {
    /// Links need a way to receive input from upstream.
    /// Some Links such as `SyncLink` will only need at most 1, but others can accept many.
    fn ingressors(self, in_streams: Vec<PacketStream<Input>>) -> Self;

    /// Provides any tokio-driven Futures needed to drive the Link, as well as handles for downstream
    /// `Link`s to use. This method consumes the `Link` since we want to move ownership of a `Link`'s
    /// runnables and egressors to the caller.
    fn build_link(self) -> Link<Output>;
}

/// `SyncLink` and `AsyncLink` should impl `ElementLink`, since they are required to have their
/// Inputs and Outputs match that of their `Element`.
pub trait ElementLinkBuilder<E: Element>: LinkBuilder<E::Input, E::Output> {
    fn element(self, element: E) -> Self;
}

/// Task Park is a structure for tasks to place their task handles when sleeping, and where they can
/// check for other tasks that need to be awoken.  As an example, the ingressor and egressor side of
/// an `queue_link` both may attempt to sleep when they are unable to work because they are waiting on
/// an action from the other side of the link. Generally, this occurs when the channel joining the `ingressor`
/// and `egressor` encounter a full or empty channel, respectively. They can place their task handle in the `task_park`
/// and expect that when the blocker has been cleared, the other side of the link will awaken them by calling `task.notify()`.
/// `task_park` also contains logic to prevent one side from sleeping when the other side will be unable to awaken them,
/// in order to prevent deadlocks.
mod task_park;
