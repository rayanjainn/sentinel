//! Per-connection TCP byte counters over `NETLINK_SOCK_DIAG` (the interface `ss -ti` uses).

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

use crate::parse::sock_diag::{Batch, DiagSocket, af_inet, af_inet6, dump_request, parse_datagram};

const RECV_BUFFER: usize = 64 * 1024;

fn netlink_socket() -> io::Result<OwnedFd> {
    // SAFETY: plain socket(2) call; the descriptor is owned immediately.
    let raw = unsafe {
        libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_RAW | libc::SOCK_CLOEXEC,
            libc::NETLINK_SOCK_DIAG,
        )
    };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `raw` is a new descriptor owned by nobody else.
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    let timeout = libc::timeval {
        tv_sec: 2,
        tv_usec: 0,
    };
    // SAFETY: valid descriptor and option buffer of the declared size.
    unsafe {
        libc::setsockopt(
            fd.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            (&timeout as *const libc::timeval).cast(),
            std::mem::size_of::<libc::timeval>() as libc::socklen_t,
        );
    }
    Ok(fd)
}

fn dump_family(fd: &OwnedFd, family: u8, sequence: u32) -> io::Result<Vec<DiagSocket>> {
    let request = dump_request(family, sequence);
    // SAFETY: zeroed sockaddr_nl addresses the kernel (pid 0).
    let mut kernel: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
    kernel.nl_family = libc::AF_NETLINK as libc::sa_family_t;
    // SAFETY: request buffer and address are valid for the lengths passed.
    let sent = unsafe {
        libc::sendto(
            fd.as_raw_fd(),
            request.as_ptr().cast(),
            request.len(),
            0,
            (&kernel as *const libc::sockaddr_nl).cast(),
            std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
        )
    };
    if sent < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut sockets = Vec::new();
    let mut buffer = vec![0u8; RECV_BUFFER];
    loop {
        // SAFETY: buffer is writable for its length.
        let received =
            unsafe { libc::recv(fd.as_raw_fd(), buffer.as_mut_ptr().cast(), buffer.len(), 0) };
        if received < 0 {
            return Err(io::Error::last_os_error());
        }
        if received == 0 {
            return Ok(sockets);
        }
        match parse_datagram(&buffer[..received as usize]) {
            Batch::More(batch) => sockets.extend(batch),
            Batch::Done(batch) => {
                sockets.extend(batch);
                return Ok(sockets);
            }
            Batch::Error(errno) => return Err(io::Error::from_raw_os_error(-errno)),
        }
    }
}

/// Every TCP connection with kernel byte counters, IPv4 and IPv6.
pub(crate) fn tcp_counters() -> io::Result<Vec<DiagSocket>> {
    let fd = netlink_socket()?;
    let mut sockets = dump_family(&fd, af_inet(), 1)?;
    match dump_family(&fd, af_inet6(), 2) {
        Ok(v6) => sockets.extend(v6),
        // IPv6 disabled at boot: the family is simply absent.
        Err(err)
            if matches!(
                err.raw_os_error(),
                Some(libc::EAFNOSUPPORT) | Some(libc::ENOENT) | Some(libc::EINVAL)
            ) => {}
        Err(err) => return Err(err),
    }
    Ok(sockets)
}
