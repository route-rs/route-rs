use crate::sockets;
use futures::{
    ready,
    task::{Context, Poll},
    Future,
};
use mio::Ready;
use std::{
    cell::UnsafeCell,
    ffi::CStr,
    io,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering::*},
        Arc,
    },
};
use tokio::io::PollEvented;

/// Represents a bound `AF_PACKET` socket for use with Tokio. At this phase in a
/// socket's lifecycle, it can be read and written from.
pub struct AsyncBoundSocket {
    sock: PollEvented<sockets::BoundSocket>,
}

impl AsyncBoundSocket {
    /// Constructs an `AsyncBoundSocket` from a network interface name.
    pub fn from_interface(iface: impl AsRef<CStr>) -> io::Result<Self> {
        let mut sock = sockets::Socket::new()?;
        sock.set_nonblocking(true)?;
        let sock = sock.bind(iface)?;
        Ok(Self {
            sock: PollEvented::new(sock)?,
        })
    }

    /// Turns promsicuous mode on or off on this NIC. Useful for recieving all packets on an
    /// interface, including those not addressed to the device.
    pub fn set_promiscuous(&mut self, p: bool) -> io::Result<()> {
        self.sock.get_mut().set_promiscuous(p)
    }

    /// Splits the `AsyncBoundSocket` into a sending and receving side.
    /// Original implementation comes from [Tokio](https://docs.rs/tokio/0.2.20/tokio/io/fn.split.html).
    ///
    /// To restore the `AsyncBoundSocket` object, use `unsplit`.
    pub fn split(self) -> (SendHalf, RecvHalf) {
        let inner = Arc::new(Inner {
            locked: AtomicBool::new(false),
            sock: UnsafeCell::new(self),
        });

        let rx = RecvHalf {
            inner: inner.clone(),
        };
        let tx = SendHalf { inner };

        (tx, rx)
    }

    /// Returns `Poll::Pending` until there is a packet available.
    pub fn poll_can_rx(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let ready = Ready::readable();
        ready!(self.sock.poll_read_ready(cx, ready))?;
        Poll::Ready(Ok(()))
    }

    /// Returns `Poll::Pending` until a packet can be sent.
    pub fn poll_can_tx(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        ready!(self.sock.poll_write_ready(cx))?;
        Poll::Ready(Ok(()))
    }

    /// Clears the "can transmit" state of the socket.
    pub fn clear_can_tx(&mut self, cx: &mut Context<'_>) -> io::Result<()> {
        self.sock.clear_write_ready(cx)
    }

    /// Clears the "can receive" state of the socket.
    pub fn clear_can_rx(&mut self, cx: &mut Context<'_>) -> io::Result<()> {
        let ready = Ready::readable();
        self.sock.clear_read_ready(cx, ready)
    }

    /// Sends a frame to the socket asynchronously.
    /// Returns `Poll::Pending` if the socket cannot be sent to.
    pub fn poll_send(&mut self, cx: &mut Context<'_>, frame: &[u8]) -> Poll<io::Result<usize>> {
        ready!(self.poll_can_tx(cx))?;
        match self.sock.get_mut().send(frame) {
            Ok(count) => Poll::Ready(Ok(count)),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.clear_can_tx(cx)?;
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    /// Receives a frame from the socket asynchronously.
    /// Returns `Poll::Pending` if the socket cannot be read from.
    pub fn poll_recv(
        &mut self,
        cx: &mut Context<'_>,
        frame: &mut [u8],
    ) -> Poll<io::Result<(usize, sockets::Addr)>> {
        ready!(self.poll_can_rx(cx))?;
        match self.sock.get_mut().recv(frame) {
            Ok(x) => {
                self.clear_can_rx(cx)?;
                Poll::Ready(Ok(x))
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                self.clear_can_rx(cx)?;
                Poll::Pending
            }
            Err(e) => Poll::Ready(Err(e)),
        }
    }

    /// Tries to send a frame. Will return `WouldBlock` if the socket cannot
    /// send the frame.
    pub fn try_send(&mut self, frame: &[u8]) -> io::Result<usize> {
        self.sock.get_mut().send(frame)
    }

    /// Tries to receive a frame. Will return `WouldBlock` if no frame is
    /// available.
    pub fn try_recv(&mut self, frame: &mut [u8]) -> io::Result<(usize, sockets::Addr)> {
        self.sock.get_mut().recv(frame)
    }

    /// Returns a `Future` that calls [`poll_send`](), enabling use of async/await.
    pub fn send<'a>(&'a mut self, frame: &'a [u8]) -> impl Future<Output = io::Result<usize>> + 'a {
        SendFuture { sock: self, frame }
    }

    /// Returns a `Future` that calls [`poll_recv`](), enabling use of async/await.
    pub fn recv<'a>(
        &'a mut self,
        frame: &'a mut [u8],
    ) -> impl Future<Output = io::Result<(usize, sockets::Addr)>> + 'a {
        RecvFuture { sock: self, frame }
    }
}

struct SendFuture<'a> {
    sock: &'a mut AsyncBoundSocket,
    frame: &'a [u8],
}

impl Unpin for SendFuture<'_> {}

impl Future for SendFuture<'_> {
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        let sock = &mut me.sock;
        let frame = &me.frame;
        sock.poll_send(cx, &frame)
    }
}

struct RecvFuture<'a> {
    sock: &'a mut AsyncBoundSocket,
    frame: &'a mut [u8],
}

impl Unpin for RecvFuture<'_> {}

impl Future for RecvFuture<'_> {
    type Output = io::Result<(usize, sockets::Addr)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        let sock = &mut me.sock;
        let mut frame = &mut me.frame;
        sock.poll_recv(cx, &mut frame)
    }
}

/// The receive half of an `AsyncBoundSocket`.
pub struct RecvHalf {
    inner: Arc<Inner>,
}

/// The send half of an `AsyncBoundSocket`.
pub struct SendHalf {
    inner: Arc<Inner>,
}

/// Represents a reservation for space in the socket's transmit queue.
pub struct SendHalfLock<'a> {
    guard: Guard<'a>,
}

impl RecvHalf {
    /// Returns true if `other` was created from the same `split` call.
    pub fn is_pair_of(&self, other: &SendHalf) -> bool {
        other.is_pair_of(self)
    }

    /// Joins the `SendHalf` with this `RecvHalf` to recover the original `AsyncBoundSocket`.
    pub fn unsplit(self, tx: SendHalf) -> AsyncBoundSocket {
        if self.is_pair_of(&tx) {
            drop(tx);

            let inner = Arc::try_unwrap(self.inner)
                .ok()
                .expect("Arc::try_unwrap failed");

            inner.sock.into_inner()
        } else {
            panic!("Unrelated SendHalf passed to unsplit")
        }
    }

    /// Returns `Poll::Pending` if there is no packet available.
    pub fn poll_can_rx(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut inner = ready!(self.inner.poll_lock(cx));
        inner.sock_pin().poll_can_rx(cx)
    }

    /// Receives a frame asynchronously from the socket. Returns `Poll::Pending`
    /// if there is no frame available.
    pub fn poll_recv(
        &mut self,
        cx: &mut Context<'_>,
        frame: &mut [u8],
    ) -> Poll<io::Result<(usize, sockets::Addr)>> {
        let mut inner = ready!(self.inner.poll_lock(cx));
        inner.sock_pin().poll_recv(cx, frame)
    }

    /// Returns a `Future` that calls [`poll_recv`](). Enables use in async/await.
    pub fn recv<'a>(
        &'a mut self,
        frame: &'a mut [u8],
    ) -> impl Future<Output = io::Result<(usize, sockets::Addr)>> + 'a {
        RecvHalfRecvFuture { half: self, frame }
    }
}

struct RecvHalfRecvFuture<'a> {
    half: &'a mut RecvHalf,
    frame: &'a mut [u8],
}

impl Unpin for RecvHalfRecvFuture<'_> {}

impl Future for RecvHalfRecvFuture<'_> {
    type Output = io::Result<(usize, sockets::Addr)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        let half = &mut me.half;
        let mut frame = &mut me.frame;
        half.poll_recv(cx, &mut frame)
    }
}

impl SendHalf {
    /// Returns true if `other` was created from the same `split` call.
    pub fn is_pair_of(&self, other: &RecvHalf) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    /// Returns `Poll::Pending` until there is space available in the transmit
    /// queue, then provides a [`SendHalfLock`]() to guard that space.
    pub fn poll_lock_tx<'a>(
        &'a mut self,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<SendHalfLock<'a>>> {
        let mut inner = ready!(self.inner.poll_lock(cx));
        ready!(inner.sock_pin().poll_can_tx(cx))?;
        Poll::Ready(Ok(SendHalfLock { guard: inner }))
    }

    /// Sends a frame to a socket asynchronously. Returns `Poll::Pending` until there is space
    /// in the transmit queue.
    pub fn poll_send(&mut self, cx: &mut Context<'_>, frame: &[u8]) -> Poll<io::Result<usize>> {
        let mut inner = ready!(self.inner.poll_lock(cx));
        inner.sock_pin().poll_send(cx, frame)
    }

    /// Returns a `Future` that calls [`poll_send`](). Used in async/await.
    pub fn send<'a>(&'a mut self, frame: &'a [u8]) -> impl Future<Output = io::Result<usize>> + 'a {
        SendHalfSendFuture { half: self, frame }
    }
}

impl SendHalfLock<'_> {
    /// Attempts to send a frame. This should always succeed,
    /// as the `SendHalf` has been successfully locked.
    pub fn try_send(&mut self, frame: &[u8]) -> io::Result<usize> {
        self.guard.sock_pin().try_send(frame)
    }
}

struct SendHalfSendFuture<'a> {
    half: &'a mut SendHalf,
    frame: &'a [u8],
}

impl Unpin for SendHalfSendFuture<'_> {}

impl Future for SendHalfSendFuture<'_> {
    type Output = io::Result<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        let half = &mut me.half;
        let frame = &me.frame;
        half.poll_send(cx, &frame)
    }
}

// These implementations are safe because of the Guard abstraction below.
unsafe impl Send for RecvHalf {}
unsafe impl Send for SendHalf {}
unsafe impl Sync for RecvHalf {}
unsafe impl Sync for SendHalf {}

// implementation details follow
// inspired by tokio::split
// original implemention licensed under MIT/Apache 2.0,
// copyright notice reproduced below:
//
// Copyright (c) 2019 Tokio Contributors
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

struct Inner {
    locked: AtomicBool,
    sock: UnsafeCell<AsyncBoundSocket>,
}

struct Guard<'a> {
    inner: &'a Inner,
}

impl Inner {
    fn poll_lock(&self, cx: &mut Context<'_>) -> Poll<Guard<'_>> {
        if !self.locked.compare_and_swap(false, true, Acquire) {
            Poll::Ready(Guard { inner: self })
        } else {
            // spin

            std::thread::yield_now();
            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }
}

impl Guard<'_> {
    fn sock_pin(&mut self) -> Pin<&mut AsyncBoundSocket> {
        // this is safe because the Inner is stored in an Arc
        // and the Guard guarantees mutual exclusion
        unsafe { Pin::new_unchecked(&mut *self.inner.sock.get()) }
    }
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        self.inner.locked.store(false, Release);
    }
}
