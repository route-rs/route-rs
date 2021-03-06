#![deny(missing_docs)]

use crate::linux;
use libc;
use std::{
    ffi::CStr,
    io::{self, Read, Write},
    mem::{self, MaybeUninit},
    os::unix::io::RawFd,
    ptr,
};

#[cfg(feature = "tokio-support")]
use mio::{event::Evented, unix::EventedFd, Poll, PollOpt, Ready, Token};

/// Represents a link-local address.
/// At this time, it's not particularly useful.
pub struct Addr {
    _inner: libc::sockaddr_storage,
    _len: libc::socklen_t,
}

/// Represents an unbound `AF_PACKET` socket.  At this phase of a socket's lifecycle, it can be
/// configured.
pub struct Socket {
    fd: RawFd,
}

/// Represents a bound `AF_PACKET` socket. At this phase of a socket's lifecycle, it can be read
/// to/written from.
pub struct BoundSocket {
    fd: RawFd,
    iface: linux::ifreq,
    send_addr: libc::sockaddr_ll,
}

impl Socket {
    /// Creates a new unbound socket.
    pub fn new() -> io::Result<Self> {
        // This block must be marked as unsafe because it uses FFI with C code. We believe the code
        // in this block to be safe because it does not interact with any memory owned by Rust
        // code, nor does it violate the invariant of the Socket type -- namely, that it return an
        // Err if it fails to initialize.
        let fd = unsafe {
            // Resources:
            // https://beej.us/guide/bgnet/html/multi/syscalls.html#socket
            // man 7 packet
            let fd = libc::socket(libc::AF_PACKET, libc::SOCK_RAW, libc::ETH_P_ALL.to_be());
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            fd
        };
        Ok(Self { fd })
    }

    /// Binds the socket to a network interface. This function consumes the `Socket` instance, as
    /// no more configuration options may be safely changed.
    pub fn bind(self, iface: impl AsRef<CStr>) -> io::Result<BoundSocket> {
        // This block is marked as unsafe because it uses FFI, however, we believe it to be safe
        // because 1) it handles FFI failures in accordance with the bound API's conventions, and
        // 2) it safely borrows the &CStr passed in. We will test this functionality with `miri` to
        // confirm the above assumptions.
        let (send_addr, iface) = unsafe {
            // get the index of the interface
            let mut ifr: linux::ifreq = MaybeUninit::zeroed().assume_init();
            ptr::copy_nonoverlapping(
                iface.as_ref().as_ptr(),
                ifr.ifr_ifrn.ifrn_name.as_mut_ptr(),
                libc::IFNAMSIZ,
            );
            // ioctl(SIOCGIFINDEX) fills in the index field of the ifreq object
            // Resources:
            // man 7 netdevice
            let err = libc::ioctl(self.fd, linux::SIOCGIFINDEX, &ifr);
            if err < 0 {
                return Err(io::Error::last_os_error());
            }

            // bind the socket
            let mut ll: libc::sockaddr_ll = MaybeUninit::zeroed().assume_init();
            ll.sll_family = libc::AF_PACKET as libc::sa_family_t;
            ll.sll_protocol = (libc::ETH_P_ALL as u16).to_be();
            ll.sll_ifindex = ifr.ifr_ifru.ifru_ivalue; // expanded from `ifr_ifindex` in kernel headers
                                                       // Resources:
                                                       // https://beej.us/guide/bgnet/html/multi/syscalls.html#bind
                                                       // man 7 packet regarding sockaddr_ll
            let err = libc::bind(
                self.fd,
                &mut ll as *mut _ as *mut libc::sockaddr,
                mem::size_of::<libc::sockaddr_ll>() as libc::c_uint,
            );
            if err < 0 {
                return Err(io::Error::last_os_error());
            }

            (ll, ifr)
        };
        let fd = self.fd;
        // This ensures that `self` does not attempt to close the file descriptor, as the file
        // descriptor is transferred to the BoundSocket we're returning. This doesn't cause any
        // resource leaks since the stack-bound `self` is consumed and deallocated in
        // `mem::forget`.
        mem::forget(self);
        Ok(BoundSocket {
            fd,
            iface,
            send_addr,
        })
    }

    /// Configures the socket's non-blocking status.
    pub fn set_nonblocking(&mut self, nonblocking: bool) -> io::Result<()> {
        // This block is marked as unsafe because it uses FFI, however, we assume this code to be
        // safe because we handle fcntl's failures properly. Additionally, we do not borrow any
        // Rust-owned memory.
        // Resources used to write syscall code:
        // https://beej.us/guide/bgnet/html/multi/advanced.html#blocking
        // man 2 fcntl
        unsafe {
            let flags = libc::fcntl(self.fd, libc::F_GETFL);
            if flags < 0 {
                return Err(io::Error::last_os_error());
            }
            let new_flags = if nonblocking {
                flags | libc::O_NONBLOCK
            } else {
                flags & (!libc::O_NONBLOCK)
            };
            let err = libc::fcntl(self.fd, libc::F_SETFL, new_flags);
            if err < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    /// Returns true if the socket is configured not to block, false otherwise.
    pub fn is_nonblocking(&self) -> io::Result<bool> {
        // See comments on block above (in set_nonblocking).
        let flags = unsafe {
            let flags = libc::fcntl(self.fd, libc::F_GETFL);
            if flags < 0 {
                return Err(io::Error::last_os_error());
            }
            flags
        };
        Ok(flags & libc::O_NONBLOCK == libc::O_NONBLOCK)
    }
}

impl BoundSocket {
    /// Turns promsicuous mode on or off on this NIC. Useful for recieving all packets on an
    /// interface, including those not addressed to the device.
    pub fn set_promiscuous(&mut self, p: bool) -> io::Result<()> {
        // This block is unsafe because it uses FFI. We believe this code to be safe, as it
        // conforms to the invariants of the BoundSocket API and the underlying C library.
        unsafe {
            let mut mreq: linux::packet_mreq = MaybeUninit::zeroed().assume_init();
            mreq.mr_ifindex = self.iface.ifr_ifru.ifru_ivalue; // expanded from `ifr_ifindex` in kernel headers
            mreq.mr_type = linux::PACKET_MR_PROMISC as u16;

            // Resources for the next two setsockopt invocations:
            // man 7 packet
            let err = if p {
                libc::setsockopt(
                    self.fd,
                    linux::SOL_PACKET,
                    linux::PACKET_ADD_MEMBERSHIP,
                    &mreq as *const _ as *const libc::c_void,
                    mem::size_of::<linux::packet_mreq>() as u32,
                )
            } else {
                libc::setsockopt(
                    self.fd,
                    linux::SOL_PACKET,
                    linux::PACKET_DROP_MEMBERSHIP,
                    &mreq as *const _ as *const libc::c_void,
                    mem::size_of::<linux::packet_mreq>() as u32,
                )
            };
            if err < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    /// Sends a frame to the NIC.
    pub fn send(&mut self, frame: &[u8]) -> io::Result<usize> {
        // This block is marked as unsafe because it uses FFI. We believe this code to be safe,
        // because it safely borrows the Rust-owned frame and passes the length of the frame to the
        // libc function, so it should not exhibit any C-side undefined behaviour.
        unsafe {
            // Resources:
            // https://beej.us/guide/bgnet/html/multi/syscalls.html#sendtorecv
            let bytes = libc::sendto(
                self.fd,
                frame.as_ptr() as *const _,
                frame.len(),
                0,
                &self.send_addr as *const _ as *const libc::sockaddr,
                mem::size_of::<libc::sockaddr_ll>() as libc::socklen_t,
            );
            if bytes < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(bytes as usize)
            }
        }
    }

    /// Receives a frame from the NIC.
    pub fn recv(&mut self, frame: &mut [u8]) -> io::Result<(usize, Addr)> {
        // Note comment in `send` call.
        unsafe {
            let mut storage = MaybeUninit::<libc::sockaddr_storage>::zeroed();
            let mut addrlen = mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;

            // Resources:
            // https://beej.us/guide/bgnet/html/multi/syscalls.html#sendtorecv
            let bytes = libc::recvfrom(
                self.fd,
                frame.as_mut_ptr() as *mut _,
                frame.len(),
                0,
                storage.as_mut_ptr() as *mut _,
                &mut addrlen,
            );
            if bytes < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok((
                    bytes as usize,
                    Addr {
                        _inner: storage.assume_init(),
                        _len: addrlen,
                    },
                ))
            }
        }
    }
}

impl Read for BoundSocket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.recv(buf) {
            Ok((sz, _)) => Ok(sz),
            Err(e) => Err(e),
        }
    }
}

impl Write for BoundSocket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.send(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(feature = "tokio-support")]
impl Evented for BoundSocket {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.fd).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.fd).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        EventedFd(&self.fd).deregister(poll)
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

impl Drop for BoundSocket {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}
