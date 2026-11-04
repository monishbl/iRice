//! The kernel's links and addresses changing, as `ip monitor link address`
//! sees them (for systems without NetworkManager: iwd, systemd-networkd...).
//! Only a cue: {"src":"net","kind":"change"}, once per burst of messages;
//! the shell then looks at the interfaces itself.

use crate::{emit, stop::Stop};
use serde_json::{json, Value};
use std::mem::{size_of, zeroed};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::time::Duration;

const GROUPS: u32 = 0x1 | 0x10 | 0x100; // RTMGRP_LINK | RTMGRP_IPV4_IFADDR | RTMGRP_IPV6_IFADDR

pub fn run(stop: Stop, _args: Value) {
    let sock = match open() {
        Ok(s) => s,
        Err(e) => {
            emit(json!({"src": "net", "kind": "unsupported", "error": e.to_string()}));
            return;
        }
    };
    let mut buf = vec![0u8; 32 * 1024];
    loop {
        if stop.wait(&[sock.as_raw_fd()], None).is_none() {
            return;
        }
        let mut kernel = false;
        while let Some(from_kernel) = recv(&sock, &mut buf) {
            kernel |= from_kernel;
        }
        if kernel {
            emit(json!({"src": "net", "kind": "change"}));
            // (a connection makes several at once: let them gather)
            if !stop.sleep(Duration::from_millis(100)) {
                return;
            }
        }
    }
}

fn open() -> std::io::Result<OwnedFd> {
    unsafe {
        let fd = libc::socket(libc::AF_NETLINK, libc::SOCK_RAW | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK, libc::NETLINK_ROUTE);
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let sock = OwnedFd::from_raw_fd(fd);
        let mut addr: libc::sockaddr_nl = zeroed();
        addr.nl_family = libc::AF_NETLINK as u16;
        addr.nl_groups = GROUPS;
        if libc::bind(fd, &addr as *const _ as *const libc::sockaddr, size_of::<libc::sockaddr_nl>() as u32) < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(sock)
    }
}

// Some(whether it came from the kernel), None when there's nothing more
fn recv(sock: &OwnedFd, buf: &mut [u8]) -> Option<bool> {
    unsafe {
        let mut addr: libc::sockaddr_nl = zeroed();
        let mut len = size_of::<libc::sockaddr_nl>() as u32;
        let n = libc::recvfrom(sock.as_raw_fd(), buf.as_mut_ptr() as *mut libc::c_void, buf.len(), 0,
                               &mut addr as *mut _ as *mut libc::sockaddr, &mut len);
        if n < 0 {
            return match std::io::Error::last_os_error().raw_os_error() {
                Some(libc::ENOBUFS) => Some(true), // (overrun: something changed)
                _ => None,
            };
        }
        Some(addr.nl_pid == 0)
    }
}
