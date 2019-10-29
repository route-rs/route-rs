use crate::processor::Processor;
use futures::{Future, Stream};

/// Composites are groups of links pre-assmebled to provide higher level functionality. They are highly customizable and users of the
/// library are encourged to make their own to encourage code reuse.
pub mod composite;

/// Primitive links are links that individually implement all the flow based logic that can be combined to create more compilcated
/// composite links. Users of the library should not have to implement their own primitive links, but should rather combine them into
/// their own custom composite links. 
pub mod primitive;

/// Commmon utilities used by links, for instance the `task_park` utility used in primitive links to facilite sleeping and waking.
mod utils;

/// All Links communicate through streams of packets. This allows them to be composable.
pub type PacketStream<Input> = Box<dyn Stream<Item = Input, Error = ()> + Send>;
/// Some Links may need to be driven by Tokio. This represents a handle to something Tokio can run.
pub type TokioRunnable = Box<dyn Future<Item = (), Error = ()> + Send + 'static>;
/// LinkBuilders build this.
pub type Link<Output> = (Vec<TokioRunnable>, Vec<PacketStream<Output>>);

/// `LinkBuilder` applies a builder pattern to create `Links`! `Links` should be created this way
/// so they can be composed together
///
/// The two type parameters, Input and Output, refer to the input and output types of the Link.
/// You can tell because the ingress/egress streams are of type `PacketStream<Input>`/`PacketStream<Output>` respectively.
pub trait LinkBuilder<Input, Output> {
    /// Links need a way to receive input from upstream.
    /// Some Links such as `ProcessLink` will only need at most 1, but others can accept many.
    fn ingressors(self, in_streams: Vec<PacketStream<Input>>) -> Self;

    /// Provides any tokio-driven Futures needed to drive the Link, as well as handles for downstream
    /// `Link`s to use. This method consumes the `Link` since we want to move ownership of a `Link`'s
    /// runnables and egressors to the caller.
    fn build_link(self) -> Link<Output>;
}

/// `ProcessLink` and `QueueLink` should impl `ProcessorLink`, since they are required to have their
/// Inputs and Outputs match that of their `Processor`.
pub trait ProcessLinkBuilder<P: Processor>: LinkBuilder<P::Input, P::Output> {
    fn processor(self, processor: P) -> Self;
}
