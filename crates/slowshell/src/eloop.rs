use std::{
  collections::HashSet,
  os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd},
  sync::mpsc::Receiver,
  time::Duration,
};

use anyhow::Result;
use futures_channel::mpsc::UnboundedSender;
use nix::fcntl::{FcntlArg, OFlag, fcntl};
use nix::sys::{
  epoll::{Epoll, EpollCreateFlags, EpollEvent, EpollFlags},
  time::TimeSpec,
  timer::{Expiration, TimerSetTimeFlags},
  timerfd::{ClockId, TimerFd, TimerFlags},
};
use slowshell_core::listeners::{FdCommand, Listeners};
use slowshell_core::message::Message;

pub struct EventLoop {
  epoll: Epoll,
  listeners: Listeners,
  timer_fd: TimerFd,
  wake_read: OwnedFd,
  rx: Receiver<FdCommand>,
  tx: UnboundedSender<Message>,
  oneshot_fds: HashSet<i32>,
}

impl EventLoop {
  pub fn new(
    listeners: Listeners,
    tx: UnboundedSender<Message>,
    interval_secs: u64,
    rx: Receiver<FdCommand>,
  ) -> Result<(Self, OwnedFd)> {
    let epoll = Epoll::new(EpollCreateFlags::EPOLL_CLOEXEC)?;

    let timer_fd = TimerFd::new(
      ClockId::CLOCK_MONOTONIC,
      TimerFlags::TFD_NONBLOCK | TimerFlags::TFD_CLOEXEC,
    )?;

    timer_fd.set(
      Expiration::Interval(TimeSpec::from(Duration::from_secs(interval_secs))),
      TimerSetTimeFlags::empty(),
    )?;

    epoll.add(timer_fd.as_fd(), EpollEvent::new(EpollFlags::EPOLLIN, 0))?;

    let (wake_read, wake_write) = nix::unistd::pipe()?;
    fcntl(&wake_read, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;
    epoll.add(&wake_read, EpollEvent::new(EpollFlags::EPOLLIN, 1))?;

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
      for fd in self.listeners.removed_fds() {
        let borrowed = unsafe { BorrowedFd::borrow_raw(fd) };
        let _ = self.epoll.delete(borrowed);
        if !self.listeners.is_paused(fd) {
          self.listeners.remove_full(fd);
        } else {
          self.listeners.clear_removed(fd);
        }
      }

      for fd in self.listeners.unwatched_fds() {
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

      for i in 0..n {
        let event = events[i];
        match event.data() {
          0 => {
            let mut buf = [0u8; 8];
            let _ = nix::unistd::read(self.timer_fd.as_fd(), &mut buf);
            let _ = self.tx.unbounded_send(Message::Tick);
          }
          1 => {
            let mut buf = [0u8; 128];
            while let Ok(n) = nix::unistd::read(self.wake_read.as_fd(), &mut buf) {
              if n == 0 {
                break;
              }
            }
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
          d if d >= 100 => {
            let fd = (d - 100) as i32;
            let flags = event.events();
            if flags.contains(EpollFlags::EPOLLHUP) || flags.contains(EpollFlags::EPOLLERR) {
              self.oneshot_fds.remove(&fd);
              self.listeners.terminate(fd);
              continue;
            }

            if let Some(action) = self.listeners.get_action(fd).cloned() {
              let _ = self.tx.unbounded_send(Message::FdUpdate(action));
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
  }
}
