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
