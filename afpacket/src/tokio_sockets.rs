use crate::sockets;
use std::{ffi::CStr, io};
use tokio::io::{AsyncReadExt, AsyncWriteExt, PollEvented};

pub struct AsyncBoundSocket {
    sock: PollEvented<sockets::BoundSocket>,
}

impl AsyncBoundSocket {
    pub fn from_interface(iface: impl AsRef<CStr>) -> io::Result<Self> {
        let mut sock = sockets::Socket::new()?;
        sock.set_nonblocking(true)?;
        let sock = sock.bind(iface)?;
        Ok(Self {
            sock: PollEvented::new(sock)?,
        })
    }

    pub fn set_promiscuous(&mut self, p: bool) -> io::Result<()> {
        self.sock.get_mut().set_promiscuous(p)
    }

    pub async fn send(&mut self, frame: &[u8]) -> io::Result<usize> {
        self.sock.write(frame).await
    }

    pub async fn recv(&mut self, frame: &mut [u8]) -> io::Result<usize> {
        self.sock.read(frame).await
    }
}
