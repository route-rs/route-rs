//! # What are they?
//!
//! Pipelines are abstractions used by graphgen to define a grouping of links that are run by the runtime, in our case Tokio. The graphgen
//! program takes the user provided graph, and generates a pipeline. It connects input and output channels to the pipe, so that it can be
//! hooked up to packet sources and sinks, and drops the pipeline into the runtime. Generally can be abstracted to the concept, router.

mod runner;
pub use self::runner::*;
