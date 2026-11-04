//! A stop flag a source's thread can wait on next to its own file
//! descriptors (an eventfd), so a source that's turned off ends at once
//! instead of at its next event or tick.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct Stop(Arc<OwnedFd>);

impl Stop {
    pub fn new() -> Stop {
        let fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        assert!(fd >= 0, "eventfd: {}", std::io::Error::last_os_error());
        Stop(Arc::new(unsafe { OwnedFd::from_raw_fd(fd) }))
    }

    pub fn set(&self) {
        let one: u64 = 1;
        unsafe { libc::write(self.0.as_raw_fd(), &one as *const u64 as *const libc::c_void, 8) };
    }

    pub fn is_set(&self) -> bool {
        self.wait(&[], Some(Duration::ZERO)).is_none()
    }

    /// Sleeps for `d`; false when stopped meanwhile.
    pub fn sleep(&self, d: Duration) -> bool {
        self.wait(&[], Some(d)).is_some()
    }

    /// Waits until one of `fds` is readable (or `timeout` passes): Some(the
    /// readable ones, as flags in `fds`' order), or None once stopped.
    pub fn wait(&self, fds: &[RawFd], timeout: Option<Duration>) -> Option<Vec<bool>> {
        let mut p: Vec<libc::pollfd> = fds
            .iter()
            .map(|&fd| libc::pollfd { fd, events: libc::POLLIN, revents: 0 })
            .collect();
        p.push(libc::pollfd { fd: self.0.as_raw_fd(), events: libc::POLLIN, revents: 0 });
        let ms = timeout.map(|d| d.as_millis().min(i32::MAX as u128) as i32).unwrap_or(-1);
        loop {
            let n = unsafe { libc::poll(p.as_mut_ptr(), p.len() as libc::nfds_t, ms) };
            if n < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            break;
        }
        if p.last().map(|x| x.revents != 0).unwrap_or(false) {
            return None;
        }
        Some(p[..fds.len()].iter().map(|x| x.revents != 0).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_a_wait() {
        let s = Stop::new();
        assert!(!s.is_set());
        assert!(s.sleep(Duration::from_millis(1)));
        let t = s.clone();
        let h = std::thread::spawn(move || t.wait(&[], None));
        s.set();
        assert!(h.join().unwrap().is_none());
        assert!(s.is_set());
    }
}
