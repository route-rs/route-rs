//! # What are they for?
//!
//! Links are an abstraction used by the runtime to link processors and classifiers together, and manage the flow of packets through the router.
//! Processors and Classifiers, which implement all the non-flow business logic of the router, are loaded into links to create specfic behavior.
//! Links are joined together via their interfaces, and the links are then dumped into a runtime to begin pulling packets through the router.
//! When the library user uses Graphgen, Links are declared, connected, and placed into a runtime via generated code. The user does this by laying
//! out their desired graph of Processors and Classifiers in a graph file, and Graphgen does all the work from there! Additionally, since all links
//! must implement the LinkBuilder trait, Links are transparently composable at no cost during runtime! The most basic links that implement Poll,
//! Stream and Future are Primitives. Links that are made by wrapping a collection of Links are called Composites. Link composition is a great way
//! to reduce the complexity of your router, and make its design more inspectable. Users of the library are encouraged to create their own Composite
//! Links, but should refrain from defining new Primitive Links. This prevents the user from having to worry about the complexities of generically
//! chaining asynchronous computation together around Channels, and you can instead focus on your packets.

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
