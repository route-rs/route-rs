use crate::processor::Processor;
use futures::{Future, Stream};

/// Composites are groups of links pre-assmebled to provide higher level functionality. They are highly customizable and users of the
/// library are encourged to make their own to encourage code reuse.
pub mod composite;

// TODO: primitives docs
pub mod primitive;

// TODO: util docs
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
    /// Links that can not support the number of ingressors provided will panic. Links that already have
    /// ingressors will panic on calling this function, since we expect this is a user configuration error.
    fn ingressors(self, in_streams: Vec<PacketStream<Input>>) -> Self;

    /// Append ingressor to list of ingressors, works like push() for a Vector
    /// If the link can not support the addition of another ingressor, it will panic.
    fn ingressor(self, in_stream: PacketStream<Input>) -> Self;

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
