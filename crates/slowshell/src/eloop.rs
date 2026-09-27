use std::{
  collections::HashSet,
  os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd},
  sync::mpsc::Receiver,
  time::Duration,
};

use futures_channel::mpsc::UnboundedSender;
use miette::{IntoDiagnostic, Result};
use nix::fcntl::{FcntlArg, OFlag, fcntl};
use nix::sys::{
  epoll::{Epoll, EpollCreateFlags, EpollEvent, EpollFlags, EpollTimeout},
  time::TimeSpec,
  timer::{Expiration, TimerSetTimeFlags},
  timerfd::{ClockId, TimerFd, TimerFlags},
};
use slowshell_core::listeners::{FdCommand, ListenerAction, Listeners};
use slowshell_core::message::Message;

pub struct EventLoop {
  epoll: Epoll,
  listeners: Listeners,
  timer_fd: TimerFd,
  wake_read: OwnedFd,
  rx: Receiver<FdCommand>,
  tx: UnboundedSender<Message>,
  oneshot_fds: HashSet<i32>,
  removed_scratch: Vec<i32>,
  unwatched_scratch: Vec<i32>,
}

impl EventLoop {
  pub fn new(
    listeners: Listeners,
    tx: UnboundedSender<Message>,
    interval_secs: u64,
    rx: Receiver<FdCommand>,
  ) -> Result<(Self, OwnedFd)> {
    let epoll = Epoll::new(EpollCreateFlags::EPOLL_CLOEXEC).into_diagnostic()?;

    let timer_fd = TimerFd::new(
      ClockId::CLOCK_MONOTONIC,
      TimerFlags::TFD_NONBLOCK | TimerFlags::TFD_CLOEXEC,
    )
    .into_diagnostic()?;

    timer_fd
      .set(
        Expiration::Interval(TimeSpec::from(Duration::from_secs(interval_secs))),
        TimerSetTimeFlags::empty(),
      )
      .into_diagnostic()?;

    epoll
      .add(timer_fd.as_fd(), EpollEvent::new(EpollFlags::EPOLLIN, 0))
      .into_diagnostic()?;

    let (wake_read, wake_write) = nix::unistd::pipe().into_diagnostic()?;
    fcntl(&wake_read, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).into_diagnostic()?;
    epoll
      .add(&wake_read, EpollEvent::new(EpollFlags::EPOLLIN, 1))
      .into_diagnostic()?;

    let _ = tx.unbounded_send(Message::Tick);

    Ok((
      EventLoop {
        epoll,
        listeners,
        timer_fd,
        wake_read,
        rx,
        tx,
        oneshot_fds: HashSet::new(),
        removed_scratch: Vec::new(),
        unwatched_scratch: Vec::new(),
      },
      wake_write,
    ))
  }

  pub fn _listeners_mut(&mut self) -> &mut Listeners {
    &mut self.listeners
  }

  pub fn run(&mut self) {
    let mut events = [EpollEvent::empty(); 16];

    loop {
      self.removed_scratch.clear();
      self.listeners.drain_removed_fds(&mut self.removed_scratch);
      for &fd in &self.removed_scratch {
        let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };
        let _ = self.epoll.delete(borrowed);
        if !self.listeners.is_paused(fd) {
          self.listeners.remove_full(fd);
        } else {
          self.listeners.clear_removed(fd);
        }
      }

      self.unwatched_scratch.clear();
      self
        .listeners
        .drain_unwatched_fds(&mut self.unwatched_scratch);
      for &fd in &self.unwatched_scratch {
        let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };
        match self.epoll.add(
          borrowed,
          EpollEvent::new(self.listeners.flags(fd), fd as u64 + 100),
        ) {
          Ok(_) | Err(nix::Error::EEXIST) => self.listeners.watch(fd),
          Err(e) => eprintln!("epoll add fd {fd}: {e}"),
        }
      }

      let n = match self
        .epoll
        .wait(&mut events, nix::sys::epoll::EpollTimeout::NONE)
      {
        Ok(n) => n,
        Err(nix::Error::EINTR) => continue,
        Err(e) => {
          eprintln!("epoll_wait: {e}");
          std::thread::sleep(Duration::from_millis(100));
          continue;
        }
      };

      let mut batch: Vec<ListenerAction> = Vec::new();
      let mut seen: HashSet<String> = HashSet::new();
      let mut tick = false;

      self.collect_events(&events[..n], &mut batch, &mut seen, &mut tick);

      for _ in 0..4 {
        match self.epoll.wait(&mut events, EpollTimeout::ZERO) {
          Ok(0) => break,
          Ok(extra) => self.collect_events(&events[..extra], &mut batch, &mut seen, &mut tick),
          Err(nix::Error::EINTR) => continue,
          Err(_) => break,
        }
      }

      if tick {
        let _ = self.tx.unbounded_send(Message::Tick);
      }

      if !batch.is_empty() {
        let _ = self.tx.unbounded_send(Message::FdBatch(batch));
      }
    }
  }

  fn collect_events(
    &mut self,
    events: &[EpollEvent],
    batch: &mut Vec<ListenerAction>,
    seen: &mut HashSet<String>,
    tick: &mut bool,
  ) {
    for event in events {
      match event.data() {
        0 => {
          let mut buf = [0u8; 8];
          let _ = nix::unistd::read(self.timer_fd.as_fd(), &mut buf);
          *tick = true;
        }
        1 => {
          let mut buf = [0u8; 128];
          while let Ok(read) = nix::unistd::read(self.wake_read.as_fd(), &mut buf) {
            if read == 0 {
              break;
            }
          }
          self.drain_commands();
        }
        d if d >= 100 => {
          let fd = (d - 100) as i32;
          let flags = event.events();

          if flags.contains(EpollFlags::EPOLLHUP) || flags.contains(EpollFlags::EPOLLERR) {
            self.oneshot_fds.remove(&fd);
            self.listeners.terminate(fd);
            continue;
          }

          if let Some(action) = self.listeners.get_action(fd).cloned() {
            let key = match &action {
              ListenerAction::UpdateCompositor(mask) => Some(format!("comp:{mask:?}")),
              ListenerAction::Signal { name, fd } => Some(format!("sig:{name}:{fd}")),
              ListenerAction::Timer { fd, .. } => Some(format!("timer:{fd}")),
              _ => None,
            };

            let duplicate = key.is_some_and(|key| !seen.insert(key));

            if !duplicate {
              batch.push(action);
            }
          }

          if self.listeners.is_timer(fd) {
            let mut buf = [0u8; 8];
            let _ = nix::unistd::read(unsafe { BorrowedFd::borrow_raw(fd) }, &mut buf);
          }

          if self.oneshot_fds.remove(&fd) {
            self.listeners.terminate(fd);
          }
        }
        _ => {}
      }
    }
  }

  fn drain_commands(&mut self) {
    while let Ok(cmd) = self.rx.try_recv() {
      match cmd {
        FdCommand::Register {
          fd,
          flags,
          action,
          oneshot,
        } => {
          let raw = fd.as_raw_fd();
          self.listeners.own(raw, fd);
          self.listeners.flag(raw, flags);
          self.listeners.action(raw, action);
          if oneshot {
            self.oneshot_fds.insert(raw);
          }
        }
        FdCommand::WatchRaw { fd, flags, action } => {
          self.listeners.flag(fd, flags);
          self.listeners.action(fd, action);
        }
        FdCommand::Terminate(fd) => {
          self.oneshot_fds.remove(&fd);
          self.listeners.terminate(fd);
        }
      }
    }
  }
}
