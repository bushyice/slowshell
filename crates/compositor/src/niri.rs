use std::{
  collections::HashMap,
  io::{BufRead, BufReader, Write},
  os::{fd::AsRawFd, unix::net::UnixStream},
  time::{Duration, Instant},
};

use miette::{Context, IntoDiagnostic, Result};
use niri_ipc::state::{EventStreamState, EventStreamStatePart};
use nix::sys::epoll::EpollFlags;
use slowshell_config::Config;
use slowshell_core::{
  Monitor, Store, Window, Workspace,
  listeners::{CompositorEvent, ListenerAction, Listeners},
  types::{Ustr, Void},
};

use crate::{Compositor, CompositorState};

const GEOMETRY_TTL: Duration = Duration::from_secs(10);

#[derive(Default)]
pub struct NiriCompositor {
  state: CompositorState,
  _state: EventStreamState,
  reader: Option<BufReader<UnixStream>>,
  cmd_socket: Option<niri_ipc::socket::Socket>,
  geometry: HashMap<String, (u32, u32, f64)>,
  geometry_updated_at: Option<Instant>,
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
    listeners.action(fd, ListenerAction::UpdateCompositor(CompositorEvent::ALL));

    self.reader = Some(reader);
    self.apply_compositor_state()?;

    Ok(Void)
  }

  fn update_state(&mut self, _store: Option<&Store>) -> Result<CompositorEvent> {
    let reader = self
      .reader
      .as_mut()
      .ok_or_else(|| miette::miette!("Stream not initialized"))?;

    let mut line = String::new();
    let mut changed = CompositorEvent::NONE;

    loop {
      line.clear();
      match reader.read_line(&mut line) {
        Ok(0) => {
          // EOF: socket was closed by niri
          self.reader = None;
          return Err(miette::miette!("Niri stream closed (EOF)"));
        }

        Ok(_) => match serde_json::from_str::<niri_ipc::Event>(&line) {
          Ok(event) => {
            if !accepts(&self._state, &event) {
              eprintln!("skipping niri event for unknown workspace: {event:?}");
              continue;
            }

            changed |= event_mask(&event);
            self._state.apply(event);
          }
          Err(e) => eprintln!("[compositor] failed to parse niri event ({e}): {line}"),
        },

        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
          break;
        }

        Err(e) => return Err(e).into_diagnostic(),
      }
    }

    if changed.is_empty() {
      return Ok(CompositorEvent::NONE);
    }

    self.apply_compositor_event(changed)?;

    Ok(changed)
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

  fn apply_compositor_event(&mut self, changed: CompositorEvent) -> Result<Void> {
    let workspaces_changed = changed.contains(CompositorEvent::WORKSPACES);
    let monitors_changed = changed.contains(CompositorEvent::MONITORS);

    if workspaces_changed {
      self.apply_workspaces();
    }

    if monitors_changed || self.geometry_is_stale() {
      self.refresh_geometry();
    }

    if workspaces_changed || monitors_changed {
      self.apply_monitors();
    }

    if changed.contains(CompositorEvent::WINDOWS) {
      self.apply_windows();
    } else if changed.contains(CompositorEvent::FOCUS) {
      self.apply_focus();
    }

    if changed.contains(CompositorEvent::OVERVIEW) {
      self.state.overview_active = self._state.overview.is_open;
    }

    Ok(Void)
  }

  #[inline]
  fn apply_compositor_state(&mut self) -> Result<Void> {
    self.apply_compositor_event(CompositorEvent::ALL)
  }

  fn apply_windows(&mut self) {
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
    self.apply_focus();
  }

  fn apply_focus(&mut self) {
    let focused = self
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

    let focused_id = focused.as_ref().map(|window| window.id);
    for window in &mut self.state.active_windows {
      window.is_active = Some(window.id) == focused_id;
    }

    self.state.active_window = focused;
  }

  fn apply_workspaces(&mut self) {
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
  }

  fn apply_monitors(&mut self) {
    self.state.monitors = self
      ._state
      .workspaces
      .workspaces
      .values()
      .filter_map(|ws| {
        if let Some(name) = &ws.output
          && ws.is_active
        {
          let (width, height, scale) = self.geometry.get(name).copied().unwrap_or((0, 0, 1.0));
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
  }

  fn geometry_is_stale(&self) -> bool {
    self
      .geometry_updated_at
      .map_or(true, |updated| updated.elapsed() > GEOMETRY_TTL)
  }

  fn refresh_geometry(&mut self) {
    if let Some(geometry) = self.query_output_geometry() {
      self.geometry = geometry;
      self.geometry_updated_at = Some(Instant::now());
    }
  }

  fn query_output_geometry(&mut self) -> Option<HashMap<String, (u32, u32, f64)>> {
    let mut geometry = HashMap::new();

    let socket = self.get_or_connect_cmd_socket().ok()?;
    let Ok(Ok(niri_ipc::Response::Outputs(outputs))) = socket.send(niri_ipc::Request::Outputs)
    else {
      return None;
    };

    for (name, output) in outputs {
      if let Some(logical) = output.logical {
        geometry.insert(name, (logical.width, logical.height, logical.scale));
      }
    }

    Some(geometry)
  }
}

fn accepts(state: &EventStreamState, event: &niri_ipc::Event) -> bool {
  match event {
    niri_ipc::Event::WorkspaceActivated { id, .. }
    | niri_ipc::Event::WorkspaceActiveWindowChanged {
      workspace_id: id, ..
    } => state.workspaces.workspaces.contains_key(id),
    _ => true,
  }
}

fn event_mask(event: &niri_ipc::Event) -> CompositorEvent {
  use niri_ipc::Event;

  match event {
    Event::WorkspacesChanged { .. } => CompositorEvent::WORKSPACES | CompositorEvent::MONITORS,
    Event::WorkspaceUrgencyChanged { .. } | Event::WorkspaceActivated { .. } => {
      CompositorEvent::WORKSPACES
    }
    Event::WorkspaceActiveWindowChanged { .. } => CompositorEvent::NONE,
    Event::WindowsChanged { .. }
    | Event::WindowOpenedOrChanged { .. }
    | Event::WindowClosed { .. }
    | Event::WindowFocusChanged { .. } => CompositorEvent::WINDOWS | CompositorEvent::FOCUS,
    Event::OverviewOpenedOrClosed { .. } => CompositorEvent::OVERVIEW,
    _ => CompositorEvent::NONE,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use niri_ipc::Event;

  #[test]
  fn event_masks_only_touch_affected_areas() {
    assert_eq!(
      event_mask(&Event::WorkspacesChanged {
        workspaces: Vec::new()
      }),
      CompositorEvent::WORKSPACES | CompositorEvent::MONITORS
    );
    assert_eq!(
      event_mask(&Event::WorkspaceActivated {
        id: 1,
        focused: true
      }),
      CompositorEvent::WORKSPACES
    );
    assert_eq!(
      event_mask(&Event::WorkspaceActiveWindowChanged {
        workspace_id: 1,
        active_window_id: Some(2)
      }),
      CompositorEvent::NONE
    );
    assert_eq!(
      event_mask(&Event::WindowsChanged {
        windows: Vec::new()
      }),
      CompositorEvent::WINDOWS | CompositorEvent::FOCUS
    );
    assert_eq!(
      event_mask(&Event::WindowFocusChanged { id: Some(1) }),
      CompositorEvent::WINDOWS | CompositorEvent::FOCUS
    );
    assert_eq!(
      event_mask(&Event::WindowClosed { id: 1 }),
      CompositorEvent::WINDOWS | CompositorEvent::FOCUS
    );
    assert_eq!(
      event_mask(&Event::OverviewOpenedOrClosed { is_open: true }),
      CompositorEvent::OVERVIEW
    );
    assert_eq!(
      event_mask(&Event::KeyboardLayoutSwitched { idx: 1 }),
      CompositorEvent::NONE
    );
    assert_eq!(
      event_mask(&Event::ConfigLoaded { failed: false }),
      CompositorEvent::NONE
    );
  }
}
