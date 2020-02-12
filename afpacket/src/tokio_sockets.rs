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

    pub fn poll_can_rx(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let ready = Ready::readable();
        ready!(self.sock.poll_read_ready(cx, ready))?;
        Poll::Ready(Ok(()))
    }

    pub fn poll_can_tx(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        ready!(self.sock.poll_write_ready(cx))?;
        Poll::Ready(Ok(()))
    }

    pub fn clear_can_tx(&mut self, cx: &mut Context<'_>) -> io::Result<()> {
        self.sock.clear_write_ready(cx)
    }

    pub fn clear_can_rx(&mut self, cx: &mut Context<'_>) -> io::Result<()> {
        let ready = Ready::readable();
        self.sock.clear_read_ready(cx, ready)
    }

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

    pub fn try_send(&mut self, frame: &[u8]) -> io::Result<usize> {
        self.sock.get_mut().send(frame)
    }

    pub fn try_recv(&mut self, frame: &mut [u8]) -> io::Result<(usize, sockets::Addr)> {
        self.sock.get_mut().recv(frame)
    }

    pub fn send<'a>(&'a mut self, frame: &'a [u8]) -> impl Future<Output = io::Result<usize>> + 'a {
        SendFuture { sock: self, frame }
    }

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

pub struct RecvHalf {
    inner: Arc<Inner>,
}

pub struct SendHalf {
    inner: Arc<Inner>,
}

pub struct SendHalfLock<'a> {
    guard: Guard<'a>,
}

impl RecvHalf {
    pub fn is_pair_of(&self, other: &SendHalf) -> bool {
        other.is_pair_of(self)
    }

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

    pub fn poll_can_rx(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let mut inner = ready!(self.inner.poll_lock(cx));
        inner.sock_pin().poll_can_rx(cx)
    }

    pub fn poll_recv(
        &mut self,
        cx: &mut Context<'_>,
        frame: &mut [u8],
    ) -> Poll<io::Result<(usize, sockets::Addr)>> {
        let mut inner = ready!(self.inner.poll_lock(cx));
        inner.sock_pin().poll_recv(cx, frame)
    }

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
    pub fn is_pair_of(&self, other: &RecvHalf) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    pub fn poll_lock_tx<'a>(
        &'a mut self,
        cx: &mut Context<'_>,
    ) -> Poll<io::Result<SendHalfLock<'a>>> {
        let mut inner = ready!(self.inner.poll_lock(cx));
        ready!(inner.sock_pin().poll_can_tx(cx))?;
        Poll::Ready(Ok(SendHalfLock { guard: inner }))
    }

    pub fn poll_send(&mut self, cx: &mut Context<'_>, frame: &[u8]) -> Poll<io::Result<usize>> {
        let mut inner = ready!(self.inner.poll_lock(cx));
        inner.sock_pin().poll_send(cx, frame)
    }

    pub fn send<'a>(&'a mut self, frame: &'a [u8]) -> impl Future<Output = io::Result<usize>> + 'a {
        SendHalfSendFuture { half: self, frame }
    }
}

impl SendHalfLock<'_> {
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

unsafe impl Send for RecvHalf {}
unsafe impl Send for SendHalf {}
unsafe impl Sync for RecvHalf {}
unsafe impl Sync for SendHalf {}

// implementation details follow
// inspired by tokio::split
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
