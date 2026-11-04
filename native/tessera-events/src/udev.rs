//! udev's device events, as `udevadm monitor --udev --property` prints them:
//! straight from udev's netlink broadcast (the same messages libudev reads),
//! so no libudev is needed. Each message carries every property udev knows
//! (ID_MODEL_FROM_DATABASE, ID_FS_LABEL...). Reported as
//!   {"src":"udev","kind":ACTION,"props":{KEY: value...}}
//! for the subsystems asked for (args {"subsystems": [...]}; none: all).

use crate::{cmd, emit, stop::Stop};
use serde_json::{json, Map, Value};
use std::mem::{size_of, zeroed};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};

const UDEV_GROUP: u32 = 2; // (1 is the kernel's own, before udev has seen it)
const MAGIC: u32 = 0xfeedcafe;

pub fn run(stop: Stop, args: Value) {
    let subsystems = cmd::strings(&args, "subsystems");
    let sock = match open() {
        Ok(s) => s,
        Err(e) => {
            emit(json!({"src": "udev", "kind": "unsupported", "error": e.to_string()}));
            return;
        }
    };
    let trusted = trusted_uids();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        if stop.wait(&[sock.as_raw_fd()], None).is_none() {
            return;
        }
        loop {
            let Some((n, groups, uid)) = recv(&sock, &mut buf) else { break };
            // Only udev's broadcast, sent by root (anyone may not fake a drive)
            if groups != UDEV_GROUP || !uid.map(|u| trusted.contains(&u)).unwrap_or(false) {
                continue;
            }
            let Some(props) = parse(&buf[..n]) else { continue };
            let sub = props.get("SUBSYSTEM").and_then(Value::as_str).unwrap_or("");
            if !subsystems.is_empty() && !subsystems.iter().any(|s| s == sub) {
                continue;
            }
            let action = props.get("ACTION").and_then(Value::as_str).unwrap_or("").to_string();
            emit(json!({"src": "udev", "kind": action, "props": props}));
        }
    }
}

fn open() -> std::io::Result<OwnedFd> {
    unsafe {
        let fd = libc::socket(libc::AF_NETLINK, libc::SOCK_RAW | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK, libc::NETLINK_KOBJECT_UEVENT);
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let sock = OwnedFd::from_raw_fd(fd);
        let one: libc::c_int = 1;
        libc::setsockopt(fd, libc::SOL_SOCKET, libc::SO_PASSCRED, &one as *const _ as *const libc::c_void, 4);
        // (a burst, a dock with ten devices, mustn't overflow it)
        let size: libc::c_int = 4 << 20;
        libc::setsockopt(fd, libc::SOL_SOCKET, libc::SO_RCVBUF, &size as *const _ as *const libc::c_void, 4);
        let mut addr: libc::sockaddr_nl = zeroed();
        addr.nl_family = libc::AF_NETLINK as u16;
        addr.nl_groups = UDEV_GROUP;
        if libc::bind(fd, &addr as *const _ as *const libc::sockaddr, size_of::<libc::sockaddr_nl>() as u32) < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(sock)
    }
}

// One message: its length, the group it was sent to, and the sender's uid
fn recv(sock: &OwnedFd, buf: &mut [u8]) -> Option<(usize, u32, Option<u32>)> {
    unsafe {
        let mut addr: libc::sockaddr_nl = zeroed();
        let mut iov = libc::iovec { iov_base: buf.as_mut_ptr() as *mut libc::c_void, iov_len: buf.len() };
        let mut cbuf = [0u8; 64];
        let mut msg: libc::msghdr = zeroed();
        msg.msg_name = &mut addr as *mut _ as *mut libc::c_void;
        msg.msg_namelen = size_of::<libc::sockaddr_nl>() as u32;
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = cbuf.as_mut_ptr() as *mut libc::c_void;
        msg.msg_controllen = cbuf.len();
        let n = libc::recvmsg(sock.as_raw_fd(), &mut msg, 0);
        if n < 0 {
            if std::io::Error::last_os_error().raw_os_error() == Some(libc::ENOBUFS) {
                return Some((0, 0, None)); // (overrun: skip, keep reading)
            }
            return None;
        }
        let mut uid = None;
        let mut c = libc::CMSG_FIRSTHDR(&msg);
        while !c.is_null() {
            if (*c).cmsg_level == libc::SOL_SOCKET && (*c).cmsg_type == libc::SCM_CREDENTIALS {
                let cred = std::ptr::read_unaligned(libc::CMSG_DATA(c) as *const libc::ucred);
                uid = Some(cred.uid);
            }
            c = libc::CMSG_NXTHDR(&msg, c);
        }
        Some((n as usize, addr.nl_groups, uid))
    }
}

// Root, as this process sees it. In a user namespace (a rootless container)
// the host's root shows as the overflow uid: it counts too there, since
// only root in the host's network namespace can send to udev's group.
fn trusted_uids() -> Vec<u32> {
    let mut t = vec![0];
    let map = std::fs::read_to_string("/proc/self/uid_map").unwrap_or_default();
    let init_ns = map.split_whitespace().collect::<Vec<_>>() == ["0", "0", "4294967295"];
    if !init_ns {
        let o = std::fs::read_to_string("/proc/sys/kernel/overflowuid").ok().and_then(|s| s.trim().parse().ok());
        t.push(o.unwrap_or(65534));
    }
    t
}

/// A udev netlink message's properties: after libudev's header
/// ("libudev\0", magic, header size, where the properties are and how
/// long), NUL-separated KEY=value pairs.
pub fn parse(msg: &[u8]) -> Option<Map<String, Value>> {
    if msg.len() < 40 || &msg[..8] != b"libudev\0" {
        return None;
    }
    let word = |i: usize| u32::from_ne_bytes(msg[i..i + 4].try_into().unwrap());
    if u32::from_be_bytes(msg[8..12].try_into().unwrap()) != MAGIC {
        return None;
    }
    let (off, len) = (word(16) as usize, word(20) as usize);
    let end = off.checked_add(len)?;
    if off < 40 || end > msg.len() {
        return None;
    }
    let mut props = Map::new();
    for kv in msg[off..end].split(|&b| b == 0) {
        let kv = String::from_utf8_lossy(kv);
        if let Some((k, v)) = kv.split_once('=') {
            if !k.is_empty() {
                props.insert(k.to_string(), Value::from(v));
            }
        }
    }
    if props.contains_key("ACTION") {
        Some(props)
    } else {
        None
    }
}

#[cfg(test)]
pub fn message(props: &[&str]) -> Vec<u8> {
    let body: Vec<u8> = props.iter().flat_map(|p| p.bytes().chain([0])).collect();
    let mut m = b"libudev\0".to_vec();
    m.extend(MAGIC.to_be_bytes());
    for w in [40u32, 40, body.len() as u32, 0, 0, 0, 0] {
        m.extend(w.to_ne_bytes());
    }
    m.extend(body);
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drive() {
        let m = message(&["ACTION=add", "DEVPATH=/devices/pci0000:00/usb1/1-2", "SUBSYSTEM=block", "DEVTYPE=partition",
                          "ID_BUS=usb", "ID_FS_LABEL=My\\x20Disk", "DEVNAME=/dev/sdb1", "SEQNUM=4242"]);
        let p = parse(&m).unwrap();
        assert_eq!(p["ACTION"], "add");
        assert_eq!(p["SUBSYSTEM"], "block");
        assert_eq!(p["ID_FS_LABEL"], "My\\x20Disk"); // (as udevadm prints it; the shell unescapes)
        assert_eq!(p["DEVNAME"], "/dev/sdb1");
        assert_eq!(p.len(), 8);
    }

    #[test]
    fn not_udev() {
        // the kernel's own uevents ("add@/devices/..."), a bad magic, a short one, lies about lengths
        assert!(parse(b"add@/devices/foo\0ACTION=add\0SUBSYSTEM=usb\0").is_none());
        let mut m = message(&["ACTION=add"]);
        m[8] ^= 0xff;
        assert!(parse(&m).is_none());
        assert!(parse(b"libudev\0").is_none());
        let mut m = message(&["ACTION=add"]);
        m[20..24].copy_from_slice(&9999u32.to_ne_bytes());
        assert!(parse(&m).is_none());
        // no ACTION
        assert!(parse(&message(&["SUBSYSTEM=usb"])).is_none());
    }
}
