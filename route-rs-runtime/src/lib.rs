//! In brief: The runtime is what takes the basket of computation required by the user, links it together into the desired
//! graph, and preps it to be handed to the desired runtime for running.
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

/// Structure that defines a complete router, with input and output interfaces that may be hooked into the host system.
pub mod pipeline;

/// Utilities for the Runtime. Mostly testing constructs.
pub mod utils;
