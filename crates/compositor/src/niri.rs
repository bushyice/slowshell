use std::{
  io::{BufRead, BufReader, Write},
  os::{fd::AsRawFd, unix::net::UnixStream},
};

use anyhow::{Context, Result};
use futures_channel::mpsc::UnboundedSender;
use niri_ipc::state::{EventStreamState, EventStreamStatePart};
use nix::sys::epoll::EpollFlags;
use slowshell_config::Config;
use slowshell_core::{
  Monitor, Store, Window,
  listeners::{ListenerAction, Listeners},
  message::Message,
  types::{Ustr, Void},
};

use crate::{Compositor, CompositorState};

#[derive(Default)]
pub struct NiriCompositor {
  state: CompositorState,
  _state: EventStreamState,
  reader: Option<BufReader<UnixStream>>,
}

impl Compositor for NiriCompositor {
  fn state(&self) -> &CompositorState {
    &self.state
  }

  fn send_cmd(&self, _cmd: crate::CompositorCommand) -> anyhow::Result<Void> {
    Ok(Void)
  }

  fn is_active(&self, _config: &Config) -> bool {
    std::env::var_os("NIRI_SOCKET")
      .or_else(|| std::env::var_os("NIRI_SOCKET_PATH"))
      .is_some()
  }

  fn initialize(
    &mut self,
    _config: &slowshell_config::Config,
    listeners: &mut Listeners,
  ) -> anyhow::Result<Void> {
    let socket_path = std::env::var_os("NIRI_SOCKET")
      .or_else(|| std::env::var_os("NIRI_SOCKET_PATH"))
      .ok_or_else(|| {
        anyhow::anyhow!("NIRI_SOCKET or NIRI_SOCKET_PATH environment variable not set")
      })?;

    let mut stream = UnixStream::connect(socket_path)?;

    stream
      .write_all({ serde_json::to_string(&niri_ipc::Request::EventStream)? + "\n" }.as_bytes())?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    reader.read_line(&mut line)?;

    let reply: niri_ipc::Reply =
      serde_json::from_str(&line).context("Failed to parse handshake")?;
    if let Err(e) = reply {
      eprintln!("niri is dumb: {e}");
      return Err(anyhow::anyhow!("Niri refused EventStream: {}", e));
    }

    self._state = niri_ipc::state::EventStreamState::default();
    reader
      .get_ref()
      .set_read_timeout(Some(std::time::Duration::from_millis(500)))?;
    loop {
      let mut init_line = String::new();
      match reader.read_line(&mut init_line) {
        Ok(0) => break,
        Ok(_) => {
          if let Ok(event) = serde_json::from_str::<niri_ipc::Event>(&init_line) {
            self._state.apply(event);
          }
        }
        Err(e)
          if e.kind() == std::io::ErrorKind::WouldBlock
            || e.kind() == std::io::ErrorKind::TimedOut =>
        {
          break;
        }
        Err(e) => return Err(e.into()),
      }
    }
    reader.get_ref().set_read_timeout(None)?;

    reader.get_ref().set_nonblocking(true)?;

    let fd = reader.get_ref().as_raw_fd();
    listeners.flag(fd, EpollFlags::EPOLLIN | EpollFlags::EPOLLET);
    listeners.action(fd, ListenerAction::UpdateCompositor);

    self.reader = Some(reader);
    self.apply_compositor_state()?;

    Ok(Void)
  }

  fn update_state(&mut self, _store: Option<&Store>, tx: UnboundedSender<Message>) -> Result<Void> {
    self.update_state_inner(Some(tx))
  }
}

impl NiriCompositor {
  pub fn new() -> Self {
    Self::default()
  }

  #[inline]
  fn update_state_inner(&mut self, tx: Option<UnboundedSender<Message>>) -> Result<Void> {
    let reader = self
      .reader
      .as_mut()
      .ok_or(anyhow::anyhow!("Stream not initialized"))?;

    loop {
      let mut line = String::new();

      match reader.read_line(&mut line) {
        Ok(0) => break,

        Ok(_) => {
          if let Ok(event) = serde_json::from_str::<niri_ipc::Event>(&line) {
            if match &event {
              niri_ipc::Event::WorkspaceActivated { id, .. }
              | niri_ipc::Event::WorkspaceActiveWindowChanged {
                workspace_id: id, ..
              } => self._state.workspaces.workspaces.contains_key(id),
              _ => true,
            } {
              match (&event, &tx) {
                (niri_ipc::Event::WorkspacesChanged { workspaces: _ }, Some(tx)) => {
                  tx.unbounded_send(Message::UpdateMonitors)?;
                }
                // (niri_ipc::Event::WindowClosed { .. }, Some(tx)) => {
                //   tx.unbounded_send(Message::FdUpdate(ListenerAction::Named(
                //     "spotlight.toggle".into(),
                //   )))?;
                // }
                // (_, Some(tx)) => tx.unbounded_send(Message::FdUpdate(ListenerAction::Named(
                //   "notification.new".into(),
                // )))?,
                _ => {}
              }
              self._state.apply(event);
            } else {
              eprintln!("skipping niri event for unknown workspace (not yet in map): {event:?}");
            }
          }
        }

        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
          break;
        }

        Err(e) => return Err(e.into()),
      }
    }

    self.apply_compositor_state()
  }

  #[inline]
  fn apply_compositor_state(&mut self) -> Result<Void> {
    self.state.active_window = self
      ._state
      .windows
      .windows
      .values()
      .find(|w| w.is_focused)
      .map(|w| Window {
        title: w.title.clone().unwrap_or_default(),
        class: w.app_id.clone().unwrap_or_default(),
        metadata: Default::default(),
      });

    self.state.monitors = self
      ._state
      .workspaces
      .workspaces
      .values()
      .filter_map(|ws| {
        if let Some(name) = &ws.output
          && ws.is_active
        {
          Some((
            Ustr::from(name),
            Monitor {
              active_workspace: ws.id as u32,
              name: name.into(),
            },
          ))
        } else {
          None
        }
      })
      .collect();

    Ok(Void)
  }
}
