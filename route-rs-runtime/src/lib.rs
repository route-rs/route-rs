//! In brief: The runtime is what takes the basket of computation required by the user, links it together into the desired
//! graph, and preps it to be handed to the Tokio for running.
#[macro_use]
extern crate futures;
extern crate crossbeam;
extern crate tokio;

/// Implement the basic transformations for packets in the router.
pub mod processor;

/// Implement classification functions, helping the router decide to send a packet down route A,B,C..and so on.
pub mod classifier;

/// Wrappers around Processors and Classfiers, and implement all the movement of Packets through the Router.
pub mod link;

/// Structure meant to encapsulate a router as and input and output channel. Used by graphgen.
pub mod pipeline;

/// Utilities for the Runtime. Mostly testing constructs.
pub mod utils;
