use std::os::fd::{AsRawFd, OwnedFd};

pub fn notify_signal(fd: &OwnedFd) {
  let buf = 1u64.to_ne_bytes();
  unsafe {
    libc::write(fd.as_raw_fd(), buf.as_ptr() as *const libc::c_void, 8);
  }
}

// drainage
pub fn drain_signal_fd(fd: i32) {
  let mut buf = [0u8; 128];
  loop {
    let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
    if n <= 0 {
      break;
    }
  }
}
