# route-rs

A multithreaded, modular software defined router library, written in rust.  Safe, fast, and extensible.

Original click modular router: https://github.com/kohler/click
Click Paper: https://dl.acm.org/citation.cfm?id=354874

We plan to follow the general concepts of Click, but we want to build Route-rs the way that Click would be built today.
Namely:

* Concurrency by default (Using Tokio-rs as a runtime)
* Type safe (Written in Rust)
* Easily Extensible
* Runs in userspace

Much like the original Click, units of computation are loosely defined around `elements`, which are objects that
implement the `process` function. `Elements` are wrapped by `ElementLink`s, which is what the library will use to chain computation together, producing a functioning, modular, software-defined router. `Elements` come in either synchronous or asynchronous flavors. In general, synchronous `elements` should be used for short transformations; asynchronous `elements` are intended to carry out more computationally heavy tasks. They may also carry out tasks that wait for some period of time before returning, such as an `element` that calls out to a seperate database to make a classificiation.

The router is laid out in a pull fashion, where the asynchronous `elements` drive the synchronous `elements` ahead of them, and the asynchronous elements are polled by the runtime.  The last element in the chain, generally a `to_device element` that communicates with the networking stack, drives much of the router by trying to fetch packets from the `elements` connected to its input ports. This provides a nice feature; back-pressure. The `elements` stop processing packets when they have nowhere to put them, since most `elements` are "lazy" and do not attempt to fetch new packets unless asked by the `element` connected to their output.
