use std::{
  collections::{HashMap, HashSet},
  os::fd::{AsFd, AsRawFd, OwnedFd},
  sync::mpsc::Sender,
};

use nix::sys::epoll::EpollFlags;
use nix::sys::timerfd::TimerFd;

use crate::types::{PayloadBox, Ustr};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct IpcCommand {
  pub name: Ustr,
  pub command: Ustr,
  pub args: Vec<Ustr>,
}

impl Default for IpcCommand {
  fn default() -> Self {
    Self {
      name: "unknown".into(),
      command: "".into(),
      args: Default::default(),
    }
  }
}

#[derive(Debug, Clone)]
pub enum ListenerAction {
  UpdateCompositor,
  Named(Ustr),
  Timer {
    name: Ustr,
    fd: i32,
  },
  Signal {
    name: Ustr,
    fd: i32,
  },
  Payload {
    name: Ustr,
    payload: Option<PayloadBox>,
  },
  Ipc(IpcCommand),
  StartUp,
  FocusWorkspace(u8),
  None,
}

pub enum FdLoc {
  Owned(OwnedFd),
  Timer(TimerFd), // maybe more?
}

impl From<OwnedFd> for FdLoc {
  fn from(value: OwnedFd) -> Self {
    FdLoc::Owned(value)
  }
}

impl From<TimerFd> for FdLoc {
  fn from(value: TimerFd) -> Self {
    FdLoc::Timer(value)
  }
}

impl AsRawFd for FdLoc {
  fn as_raw_fd(&self) -> i32 {
    match self {
      FdLoc::Owned(fd) => fd.as_raw_fd(),
      FdLoc::Timer(timer) => timer.as_fd().as_raw_fd(),
    }
  }
}

#[derive(Default)]
pub struct Listeners {
  actions: HashMap<i32, ListenerAction>,
  fd: HashMap<i32, FdLoc>,
  flags: HashMap<i32, EpollFlags>,
  unwatched_fds: HashSet<i32>,
  watched_fds: HashSet<i32>,
  removed_fds: HashSet<i32>,
  paused_fds: HashSet<i32>,
}

impl Listeners {
  pub fn register_fd(&mut self, res: i32) {
    if !self.watched_fds.contains(&res) {
      self.unwatched_fds.insert(res);
    }
  }

  pub fn watch(&mut self, res: i32) {
    self.unwatched_fds.remove(&res);
    self.watched_fds.insert(res);
  }

  pub fn unwatched_fds(&mut self) -> std::collections::HashSet<i32> {
    std::mem::take(&mut self.unwatched_fds)
  }

  pub fn removed_fds(&mut self) -> std::collections::HashSet<i32> {
    std::mem::take(&mut self.removed_fds)
  }

  pub fn action(&mut self, res: i32, act: impl Into<ListenerAction>) {
    self.actions.insert(res, act.into());
    self.register_fd(res);
  }

  pub fn get_action(&self, res: i32) -> Option<&ListenerAction> {
    self.actions.get(&res)
  }

  pub fn pause(&mut self, res: i32) {
    if self.watched_fds.remove(&res) {
      self.removed_fds.insert(res);
      self.paused_fds.insert(res);
    }
  }

  pub fn resume(&mut self, res: i32) {
    if self.paused_fds.remove(&res) {
      self.removed_fds.remove(&res);
      self.register_fd(res);
    }
  }

  pub fn is_paused(&self, res: i32) -> bool {
    self.paused_fds.contains(&res)
  }

  pub fn clear_removed(&mut self, res: i32) {
    self.removed_fds.remove(&res);
  }

  pub fn own(&mut self, res: i32, fd: impl Into<FdLoc>) {
    self.fd.insert(res, fd.into());
  }

  pub fn is_timer(&self, res: i32) -> bool {
    matches!(self.fd.get(&res), Some(FdLoc::Timer(_)))
  }

  pub fn terminate(&mut self, res: i32) {
    self.unwatched_fds.remove(&res);
    if self.watched_fds.remove(&res) {
      self.removed_fds.insert(res);
      self.flags.remove(&res);
    };
    self.actions.remove(&res);
  }

  pub fn remove_full(&mut self, res: i32) {
    self.removed_fds.remove(&res);
    self.fd.remove(&res);
  }

  pub fn flags(&self, res: i32) -> EpollFlags {
    self.flags.get(&res).map_or(EpollFlags::EPOLLIN, |x| *x)
  }

  pub fn flag(&mut self, res: i32, flags: EpollFlags) {
    self.flags.insert(res, flags);
  }
}

pub enum FdCommand {
  Register {
    fd: FdLoc,
    flags: EpollFlags,
    action: ListenerAction,
    oneshot: bool,
  },
  WatchRaw {
    fd: i32,
    flags: EpollFlags,
    action: ListenerAction,
  },
  Terminate(i32),
}

pub struct FdHandle {
  tx: Sender<FdCommand>,
  wake: OwnedFd,
}

impl FdHandle {
  pub fn new(tx: Sender<FdCommand>, wake: OwnedFd) -> Self {
    Self { tx, wake }
  }

  pub fn watch(&self, fd: impl Into<OwnedFd>, action: impl Into<ListenerAction>) {
    let _ = self.tx.send(FdCommand::Register {
      fd: FdLoc::Owned(fd.into()),
      flags: EpollFlags::EPOLLIN,
      action: action.into(),
      oneshot: false,
    });
    self.wake();
  }

  pub fn watch_with_flags(
    &self,
    fd: impl Into<OwnedFd>,
    flags: EpollFlags,
    action: impl Into<ListenerAction>,
  ) {
    let _ = self.tx.send(FdCommand::Register {
      fd: FdLoc::Owned(fd.into()),
      flags,
      action: action.into(),
      oneshot: false,
    });
    self.wake();
  }

  pub fn watch_raw(&self, fd: i32, flags: EpollFlags, action: impl Into<ListenerAction>) {
    let _ = self.tx.send(FdCommand::WatchRaw {
      fd,
      flags,
      action: action.into(),
    });
    self.wake();
  }

  pub fn terminate(&self, fd: i32) {
    let _ = self.tx.send(FdCommand::Terminate(fd));
    self.wake();
  }

  fn wake(&self) {
    let buf = 1u64.to_ne_bytes();
    unsafe {
      libc::write(self.wake.as_raw_fd(), buf.as_ptr() as *const _, 8);
    }
  }

  fn get_timer_fd(
    &self,
    duration: std::time::Duration,
    oneshot: bool,
  ) -> anyhow::Result<nix::sys::timerfd::TimerFd> {
    use nix::sys::{
      time::TimeSpec,
      timer::{Expiration, TimerSetTimeFlags},
      timerfd::{ClockId, TimerFd, TimerFlags},
    };

    let timer_fd = TimerFd::new(
      ClockId::CLOCK_MONOTONIC,
      TimerFlags::TFD_NONBLOCK | TimerFlags::TFD_CLOEXEC,
    )?;

    timer_fd.set(
      if oneshot {
        Expiration::OneShot(TimeSpec::from(duration))
      } else {
        Expiration::Interval(TimeSpec::from(duration))
      },
      TimerSetTimeFlags::empty(),
    )?;

    Ok(timer_fd)
  }

  pub fn set_timer(
    &self,
    duration: std::time::Duration,
    action: ListenerAction,
  ) -> anyhow::Result<i32> {
    let fd = self.get_timer_fd(duration, true)?;
    let raw = fd.as_fd().as_raw_fd();
    let _ = self.tx.send(FdCommand::Register {
      fd: fd.into(),
      flags: EpollFlags::EPOLLIN,
      action,
      oneshot: true,
    });
    self.wake();
    Ok(raw)
  }

  pub fn set_named_timer(
    &self,
    duration: std::time::Duration,
    name: impl Into<Ustr>,
  ) -> anyhow::Result<i32> {
    let fd = self.get_timer_fd(duration, true)?;
    let raw = fd.as_fd().as_raw_fd();
    let _ = self.tx.send(FdCommand::Register {
      fd: fd.into(),
      flags: EpollFlags::EPOLLIN,
      action: ListenerAction::Timer {
        name: name.into(),
        fd: raw,
      },
      oneshot: true,
    });
    self.wake();
    Ok(raw)
  }

  pub fn set_interval(
    &self,
    duration: std::time::Duration,
    action: ListenerAction,
  ) -> anyhow::Result<i32> {
    let fd = self.get_timer_fd(duration, false)?;
    let raw = fd.as_fd().as_raw_fd();
    let _ = self.tx.send(FdCommand::Register {
      fd: fd.into(),
      flags: EpollFlags::EPOLLIN,
      action,
      oneshot: false,
    });
    self.wake();
    Ok(raw)
  }

  pub fn set_named_interval(
    &self,
    duration: std::time::Duration,
    name: impl Into<Ustr>,
  ) -> anyhow::Result<i32> {
    let fd = self.get_timer_fd(duration, false)?;
    let raw = fd.as_fd().as_raw_fd();
    let _ = self.tx.send(FdCommand::Register {
      fd: fd.into(),
      flags: EpollFlags::EPOLLIN,
      action: ListenerAction::Timer {
        name: name.into(),
        fd: raw,
      },
      oneshot: false,
    });
    self.wake();
    Ok(raw)
  }

  pub fn stop_timer(&self, fd: i32) {
    let _ = self.tx.send(FdCommand::Terminate(fd));
    self.wake();
  }
}
