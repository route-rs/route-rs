//! This crate exposes Linux's AF_PACKET sockets in a safe way.
#![deny(missing_docs)]
#![cfg(target_os = "linux")]
mod linux;
mod sockets;

#[cfg(feature = "tokio-support")]
mod tokio_sockets;

pub use sockets::{BoundSocket, Socket};
#[cfg(feature = "tokio-support")]
pub use tokio_sockets::{AsyncBoundSocket, RecvHalf, SendHalf, SendHalfLock};
