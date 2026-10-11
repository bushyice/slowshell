use std::{
  os::fd::{AsRawFd, OwnedFd},
  sync::Mutex,
};

use slowshell_core::{
  Store,
  listeners::{FdHandle, ListenerAction},
};
use tokio::sync::mpsc::UnboundedReceiver;

pub(crate) struct ServiceSlot<C> {
  rx: Mutex<Option<UnboundedReceiver<C>>>,
}

impl<C> ServiceSlot<C> {
  pub(crate) fn new(rx: UnboundedReceiver<C>) -> Self {
    Self {
      rx: Mutex::new(Some(rx)),
    }
  }

  pub(crate) fn take(&self) -> Option<UnboundedReceiver<C>> {
    self.rx.lock().unwrap().take()
  }

  #[cfg_attr(not(feature = "notifications"), allow(dead_code))]
  pub(crate) fn restore(&self, rx: UnboundedReceiver<C>) {
    *self.rx.lock().unwrap() = Some(rx);
  }

  pub(crate) fn running(&self) -> bool {
    self.rx.lock().unwrap().is_none()
  }
}

pub(crate) fn watch_notify(store: &Store, event: &str) -> Option<OwnedFd> {
  use nix::{
    fcntl::{FcntlArg, OFlag, fcntl},
    sys::epoll::EpollFlags,
  };

  let handle = store.borrow::<FdHandle>()?;
  let (read_fd, write_fd) = nix::unistd::pipe().ok()?;
  fcntl(&read_fd, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).ok()?;

  let raw = read_fd.as_raw_fd();
  handle.watch_with_flags(
    read_fd,
    EpollFlags::EPOLLIN | EpollFlags::EPOLLET,
    ListenerAction::Signal {
      name: event.into(),
      fd: raw,
    },
  );

  Some(write_fd)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn slot_picks_up_the_receiver_once() {
    let (_tx, rx) = tokio::sync::mpsc::unbounded_channel::<u32>();
    let slot = ServiceSlot::new(rx);

    assert!(!slot.running());
    assert!(slot.take().is_some());
    assert!(slot.take().is_none());
    assert!(slot.running());
  }

  #[test]
  fn slot_can_give_the_receiver_back() {
    let (_tx, rx) = tokio::sync::mpsc::unbounded_channel::<u32>();
    let slot = ServiceSlot::new(rx);

    let rx = slot.take().unwrap();
    assert!(slot.running());
    slot.restore(rx);
    assert!(!slot.running());
    assert!(slot.take().is_some());
  }
}
