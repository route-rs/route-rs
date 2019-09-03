#![cfg(target_os = "linux")]
mod linux;
mod sockets;

pub use sockets::Socket;
