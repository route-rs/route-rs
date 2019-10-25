#[macro_use]
extern crate futures;
extern crate crossbeam;
extern crate tokio;

/// Processors are the unit of transformation in route-rs. The Processor is defined by a trait that requires that
/// creators of processors implement a process function, that the underlying link will call on every packet that
/// moves through it. You can use processors to classify, modify headers, count packets, update state, create new packets,
/// join packets, whatever you want! As long as an processor conforms to the trait specification, the processor can be run inside a route-rs.
/// While there are many provided processors that can be used to implement a router, users of route-rs that need specifc functionality
/// in their router most likely will implement their own custom processors, conforming to the laid out processor standard.
pub mod processor;

pub mod classifier;

/// Links are an abstraction used by the runtime to link processors together, and manage the flow of packets through the router. Processors, which
/// implement all the non-flow business logic of the router, are loaded into links to create specfic behavior. Links are joined together via their
/// interfaces, and the links are then dumped into a runtime to being pulling packets through the router. In general, users of route-rs are not
/// expected to have to implement their own links, because all of the desired flows should already be representable with the provided selection.
/// Links are usually declared, connected, and placed into a runtime via generated code, where the user lays out their desired graph of processors
/// and our graphgen program creates the pipeline that defines the desired router. If you are looking for a place to start, start there.
pub mod link;

/// Pipelines are abstractions used by graphgen to define a grouping of links that are run by the runtime, in our case Tokio. The graphgen
/// program takes the user provided graph, and generates a pipeline. It connects input and output channels to the pipe, so that it can be
/// hooked up to packet sources and sinks, and drops the pipeline into the runtime. Generally can be abstracted to the concept, router.
pub mod pipeline;

/// Utility module
mod utils;

/// Composites are groups of links pre-assmebled to provide higher level functionality. They are highly customizable and users of the
/// library are encourged to make their own to encourage code reuse.
pub mod composite;
