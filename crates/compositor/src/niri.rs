use std::{
  collections::HashMap,
  io::{BufRead, BufReader, Write},
  os::{fd::AsRawFd, unix::net::UnixStream},
};

use futures_channel::mpsc::UnboundedSender;
use miette::{Context, IntoDiagnostic, Result};
use niri_ipc::state::{EventStreamState, EventStreamStatePart};
use nix::sys::epoll::EpollFlags;
use slowshell_config::Config;
use slowshell_core::{
  Monitor, Store, Window, Workspace,
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
  cmd_socket: Option<niri_ipc::socket::Socket>,
}

impl Compositor for NiriCompositor {
  fn state(&self) -> &CompositorState {
    &self.state
  }

  fn send_cmd(&mut self, cmd: crate::CompositorCommand) -> miette::Result<Void> {
    match cmd {
      crate::CompositorCommand::FocusWorkspace(idx) => {
        let socket = self.get_or_connect_cmd_socket()?;

        let reply = socket.send(niri_ipc::Request::Action(
          niri_ipc::Action::FocusWorkspace {
            reference: niri_ipc::WorkspaceReferenceArg::Index(idx as u8),
          },
        ));

        match reply {
          Ok(Err(e)) => Err(miette::miette!("niri failed to focus workspace: {e}")),
          Ok(Ok(_)) => Ok(Void),
          Err(e) => {
            self.cmd_socket = None;
            Err(e).into_diagnostic()
          }
        }
      }
      crate::CompositorCommand::FocusWindow(id) => {
        let socket = self.get_or_connect_cmd_socket()?;

        let reply = socket.send(niri_ipc::Request::Action(niri_ipc::Action::FocusWindow {
          id,
        }));

        match reply {
          Ok(Err(e)) => Err(miette::miette!("niri failed to focus window: {e}")),
          Ok(Ok(_)) => Ok(Void),
          Err(e) => {
            self.cmd_socket = None;
            Err(e).into_diagnostic()
          }
        }
      }
    }
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
  ) -> miette::Result<Void> {
    let socket_path = std::env::var_os("NIRI_SOCKET")
      .or_else(|| std::env::var_os("NIRI_SOCKET_PATH"))
      .ok_or_else(|| {
        miette::miette!("NIRI_SOCKET or NIRI_SOCKET_PATH environment variable not set")
      })?;

    let mut stream = UnixStream::connect(socket_path).into_diagnostic()?;

    let req_str = serde_json::to_string(&niri_ipc::Request::EventStream).into_diagnostic()?;
    stream
      .write_all(format!("{req_str}\n").as_bytes())
      .into_diagnostic()?;
    stream.flush().into_diagnostic()?;

    let mut reader = BufReader::new(stream);

    let mut line = String::new();
    reader.read_line(&mut line).into_diagnostic()?;

    let reply: niri_ipc::Reply = serde_json::from_str(&line)
      .into_diagnostic()
      .context("Failed to parse handshake")?;
    if let Err(e) = reply {
      return Err(miette::miette!("Niri refused EventStream: {}", e));
    }

    self._state = niri_ipc::state::EventStreamState::default();
    reader
      .get_ref()
      .set_read_timeout(Some(std::time::Duration::from_millis(500)))
      .into_diagnostic()?;
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
        Err(e) => return Err(e).into_diagnostic(),
      }
    }
    reader.get_ref().set_read_timeout(None).into_diagnostic()?;

    reader.get_ref().set_nonblocking(true).into_diagnostic()?;

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

  fn get_or_connect_cmd_socket(&mut self) -> miette::Result<&mut niri_ipc::socket::Socket> {
    if self.cmd_socket.is_none() {
      let socket_path = std::env::var_os("NIRI_SOCKET")
        .or_else(|| std::env::var_os("NIRI_SOCKET_PATH"))
        .ok_or_else(|| {
          miette::miette!("NIRI_SOCKET or NIRI_SOCKET_PATH environment variable not set")
        })?;

      self.cmd_socket = Some(niri_ipc::socket::Socket::connect_to(socket_path).into_diagnostic()?);
    }

    Ok(self.cmd_socket.as_mut().unwrap())
  }

  #[inline]
  fn update_state_inner(&mut self, tx: Option<UnboundedSender<Message>>) -> Result<Void> {
    let reader = self
      .reader
      .as_mut()
      .ok_or_else(|| miette::miette!("Stream not initialized"))?;

    let mut line = String::new();
    let mut dirty = false;

    loop {
      line.clear();
      match reader.read_line(&mut line) {
        Ok(0) => {
          // EOF: socket was closed by niri
          self.reader = None;
          return Err(miette::miette!("Niri stream closed (EOF)"));
        }

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
                  let _ = tx.unbounded_send(Message::UpdateMonitors);
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
              dirty = is_important(&event);
              self._state.apply(event);
            } else {
              eprintln!("skipping niri event for unknown workspace: {event:?}");
            }
          }
        }

        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
          break;
        }

        Err(e) => return Err(e).into_diagnostic(),
      }
    }

    if dirty {
      self.apply_compositor_state()
    } else {
      Ok(Void)
    }
  }

  #[inline]
  fn apply_compositor_state(&mut self) -> Result<Void> {
    let geometry = self.output_geometry();

    self.state.active_window = self
      ._state
      .windows
      .windows
      .values()
      .find(|w| w.is_focused)
      .map(|w| Window {
        id: w.id,
        title: w.title.clone().unwrap_or_default(),
        class: w.app_id.clone().unwrap_or_default(),
        is_active: true,
        metadata: Default::default(),
      });

    self.state.active_windows = self
      ._state
      .windows
      .windows
      .values()
      .filter_map(|win| {
        Some(Window {
          id: win.id,
          title: win.title.clone()?,
          class: win.app_id.clone()?,
          is_active: win.is_focused,
          metadata: Default::default(),
        })
      })
      .collect();

    self.state.active_windows.sort_unstable_by_key(|w| w.id);

    self.state.overview_active = self._state.overview.is_open;

    self.state.workspaces = self
      ._state
      .workspaces
      .workspaces
      .values()
      .map(|ws| Workspace {
        id: ws.id,
        idx: ws.idx,
        is_active: ws.is_active,
        is_focused: ws.is_focused,
        is_urgent: ws.is_urgent,
        name: ws.name.clone(),
        output: ws.output.clone(),
      })
      .collect();

    self
      .state
      .workspaces
      .sort_unstable_by(|a, b| a.output.cmp(&b.output).then_with(|| a.idx.cmp(&b.idx)));

    self.state.monitors = self
      ._state
      .workspaces
      .workspaces
      .values()
      .filter_map(|ws| {
        if let Some(name) = &ws.output
          && ws.is_active
        {
          let (width, height, scale) = geometry.get(name).copied().unwrap_or((0, 0, 1.0));
          Some((
            Ustr::from(name),
            Monitor {
              active_workspace: ws.id as u32,
              name: name.into(),
              width,
              height,
              scale,
            },
          ))
        } else {
          None
        }
      })
      .collect();

    Ok(Void)
  }

  fn output_geometry(&mut self) -> HashMap<String, (u32, u32, f64)> {
    let mut geometry = HashMap::new();

    let Ok(socket) = self.get_or_connect_cmd_socket() else {
      return geometry;
    };

    let Ok(Ok(niri_ipc::Response::Outputs(outputs))) = socket.send(niri_ipc::Request::Outputs)
    else {
      return geometry;
    };

    for (name, output) in outputs {
      if let Some(logical) = output.logical {
        geometry.insert(name, (logical.width, logical.height, logical.scale));
      }
    }

    geometry
  }
}

fn is_important(event: &niri_ipc::Event) -> bool {
  use niri_ipc::Event;

  matches!(
    event,
    Event::WorkspacesChanged { .. }
      | Event::WorkspaceUrgencyChanged { .. }
      | Event::WorkspaceActivated { .. }
      | Event::WorkspaceActiveWindowChanged { .. }
      | Event::WindowsChanged { .. }
      | Event::WindowOpenedOrChanged { .. }
      | Event::WindowClosed { .. }
      | Event::WindowFocusChanged { .. }
      | Event::WindowUrgencyChanged { .. }
      | Event::OverviewOpenedOrClosed { .. }
  )
}
