---
title: "ROUTE-RS"
---

{{% notice warning %}}
This project is in its very early stages of developmnet and is in **extreme alpha.**  Do not expect packets to be routed, APIs to be stable, or for anything to work whatsoever.
{{% /notice %}}

# route-rs

## What is it?
[Route-rs](http://github.com/route-rs/route-rs) is multithreaded, modular, software defined, router library, written in rust. Safe, fast, and extensible.

* [Original click modular router](https://github.com/kohler/click)
* [Click Paper](https://dl.acm.org/citation.cfm?id=354874)


## Main Features 
* Concurrency by default (Using [Tokio-rs](https://github.com/tokio-rs/tokio) as a runtime)
* Type safe (Written in Rust)
* Easily Extensible
* Runs in userspace

### Overview

Much like the original Click, units of computation are loosely defined around `processors`, which are objects that
implement the `process` function. `Processors` are wrapped by `ProcessorLink`s, which is what the library will use to chain computation together, producing a functioning, modular, software-defined router. `Processors` come in either synchronous or asynchronous flavors. In general, synchronous `processors` should be used for short transformations; asynchronous `processors` are intended to carry out more computationally heavy tasks. They may also carry out tasks that wait for some period of time before returning, such as an `processor` that calls out to a seperate database to make a classificiation.

The router is laid out in a pull fashion, where the asynchronous `processors` drive the synchronous `processors` ahead of them, and the asynchronous processors are polled by the runtime.  The last processor in the chain, generally a `to_device processor` that communicates with the networking stack, drives much of the router by trying to fetch packets from the `processors` connected to its input ports. This provides a nice feature; back-pressure. The `processors` stop processing packets when they have nowhere to put them, since most `processors` are "lazy" and do not attempt to fetch new packets unless asked by the `processor` connected to their output.
