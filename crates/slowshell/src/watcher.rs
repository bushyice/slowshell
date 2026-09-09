use std::os::fd::{FromRawFd, OwnedFd};
use std::path::Path;

pub struct ConfigWatcher {
  fd: OwnedFd,
}

impl ConfigWatcher {
  pub fn watch<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
    let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
    if fd < 0 {
      return Err(anyhow::anyhow!(
        "failed to initialize inotify: {}",
        std::io::Error::last_os_error()
      ));
    }

    let path_str = path.as_ref().to_string_lossy();
    let c_path = match std::ffi::CString::new(path_str.as_bytes()) {
      Ok(c) => c,
      Err(e) => {
        unsafe {
          libc::close(fd);
        }
        return Err(anyhow::anyhow!("invalid path for inotify: {e}"));
      }
    };

    let mask = libc::IN_CLOSE_WRITE | libc::IN_MOVED_TO | libc::IN_CREATE | libc::IN_DELETE;
    let wd = unsafe { libc::inotify_add_watch(fd, c_path.as_ptr(), mask) };
    if wd < 0 {
      let err = std::io::Error::last_os_error();
      unsafe {
        libc::close(fd);
      }
      return Err(anyhow::anyhow!(
        "failed to add inotify watch on {path_str}: {err}"
      ));
    }

    Ok(Self {
      fd: unsafe { OwnedFd::from_raw_fd(fd) },
    })
  }

  pub fn into_owned_fd(self) -> OwnedFd {
    self.fd
  }
}

pub fn drain_inotify_fd(fd: i32) {
  let mut buf = [0u8; 4096];
  while unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) } > 0 {}
}
