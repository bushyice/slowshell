use std::{
  collections::{HashMap, HashSet},
  io::{Read, Write},
  os::{fd::AsRawFd, unix::net::UnixStream},
  sync::{Mutex, mpsc::Receiver},
};

use futures_lite::StreamExt;
use hyprland::{
  data::{Client, Clients, Monitors, Workspaces},
  dispatch::{
    Dispatch, DispatchType, MonitorIdentifier, WindowIdentifier, WorkspaceIdentifierWithSpecial,
  },
  event_listener::{Event, EventStream},
  instance::Instance,
  shared::{Address, HyprData, HyprDataActiveOptional, HyprDataVec},
};
use miette::{IntoDiagnostic, Result};
use nix::sys::epoll::EpollFlags;
use slowshell_config::Config;
use slowshell_core::{
  Monitor, Store, Window, Workspace,
  listeners::{CompositorEvent, ListenerAction, Listeners},
  types::{Ustr, Void},
};

use crate::{Compositor, CompositorCommand, CompositorState};

fn address_id(address: &Address) -> u64 {
  let text = address.to_string();
  let hex = text.strip_prefix("0x").unwrap_or(text.as_str());
  u64::from_str_radix(hex, 16).unwrap_or(0)
}

fn workspace_idx(name: &str, id: i32) -> u8 {
  name
    .parse::<i64>()
    .unwrap_or(id as i64)
    .clamp(0, u8::MAX as i64) as u8
}

fn event_mask(event: &Event) -> CompositorEvent {
  match event {
    Event::WorkspaceChanged(_)
    | Event::WorkspaceDeleted(_)
    | Event::WorkspaceAdded(_)
    | Event::WorkspaceMoved(_)
    | Event::WorkspaceRenamed(_)
    | Event::SpecialRemoved(_)
    | Event::ChangedSpecial(_) => CompositorEvent::WORKSPACES,

    Event::MonitorAdded(_) | Event::MonitorRemoved(_) => {
      CompositorEvent::MONITORS | CompositorEvent::WORKSPACES
    }

    Event::ActiveMonitorChanged(_) | Event::UrgentStateChanged(_) => CompositorEvent::WORKSPACES,

    Event::ActiveWindowChanged(_)
    | Event::ActiveWindowChangedV1(_)
    | Event::ActiveWindowChangedV2(_)
    | Event::WindowOpened(_)
    | Event::WindowClosed(_)
    | Event::WindowMoved(_)
    | Event::WindowTitleChanged(_) => CompositorEvent::WINDOWS | CompositorEvent::FOCUS,

    _ => CompositorEvent::NONE,
  }
}

fn to_window(client: &Client) -> Window {
  Window {
    id: address_id(&client.address),
    title: client.title.clone(),
    class: client.class.clone(),
    is_active: false,
    metadata: Default::default(),
  }
}

fn spawn_event_bridge() -> Result<(UnixStream, Receiver<Event>)> {
  let (wake_read, mut wake_write) = UnixStream::pair().into_diagnostic()?;
  let (tx, rx) = std::sync::mpsc::channel();

  std::thread::spawn(move || {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
    else {
      return;
    };

    runtime.block_on(async move {
      let mut stream = EventStream::new();
      while let Some(Ok(event)) = stream.next().await {
        if tx.send(event).is_err() || wake_write.write_all(&[1]).is_err() {
          break;
        }
      }
    });
  });

  Ok((wake_read, rx))
}

#[derive(Default)]
pub struct HyprlandCompositor {
  state: CompositorState,
  wake: Option<UnixStream>,
  events: Option<Mutex<Receiver<Event>>>,
  urgent: HashSet<Address>,
  client_workspaces: HashMap<Address, String>,
}

impl HyprlandCompositor {
  pub fn new() -> Self {
    Self::default()
  }

  fn apply_compositor_state(&mut self) -> Result<Void> {
    self.refresh_windows()?;
    self.refresh_workspaces()
  }

  fn refresh_windows(&mut self) -> Result<Void> {
    let clients = Clients::get().into_diagnostic()?.to_vec();
    let active = Client::get_active().into_diagnostic()?;

    self
      .urgent
      .retain(|address| clients.iter().any(|client| &client.address == address));
    self.client_workspaces = clients
      .iter()
      .map(|client| (client.address.clone(), client.workspace.name.clone()))
      .collect();

    self.state.active_windows = clients
      .iter()
      .filter(|client| client.mapped)
      .map(to_window)
      .collect();
    self
      .state
      .active_windows
      .sort_unstable_by_key(|window| window.id);

    let focused = active
      .as_ref()
      .map(|client| (address_id(&client.address), to_window(client)));
    if let Some(client) = active {
      self.urgent.remove(&client.address);
    }

    if let Some((id, mut window)) = focused {
      window.is_active = true;
      self.state.active_window = Some(window);
      for window in &mut self.state.active_windows {
        window.is_active = window.id == id;
      }
    } else {
      self.state.active_window = None;
      for window in &mut self.state.active_windows {
        window.is_active = false;
      }
    }

    self.state.overview_active = false;

    Ok(Void)
  }

  fn refresh_workspaces(&mut self) -> Result<Void> {
    let workspaces = Workspaces::get().into_diagnostic()?.to_vec();
    let monitors = Monitors::get().into_diagnostic()?.to_vec();

    let urgent: HashSet<&str> = self
      .urgent
      .iter()
      .filter_map(|address| self.client_workspaces.get(address).map(String::as_str))
      .collect();

    self.state.workspaces = workspaces
      .iter()
      .map(|workspace| Workspace {
        id: workspace.id.max(0) as u64,
        idx: workspace_idx(&workspace.name, workspace.id),
        name: (!workspace.name.is_empty()).then(|| workspace.name.clone()),
        output: (!workspace.monitor.is_empty()).then(|| workspace.monitor.clone()),
        is_urgent: urgent.contains(workspace.name.as_str()),
        is_active: monitors
          .iter()
          .any(|monitor| monitor.active_workspace.name == workspace.name),
        is_focused: monitors
          .iter()
          .any(|monitor| monitor.focused && monitor.active_workspace.name == workspace.name),
      })
      .collect();
    self
      .state
      .workspaces
      .sort_unstable_by(|a, b| a.output.cmp(&b.output).then_with(|| a.idx.cmp(&b.idx)));

    self.state.monitors = monitors
      .iter()
      .filter(|monitor| !monitor.disabled)
      .map(|monitor| {
        (
          Ustr::from(monitor.name.as_str()),
          Monitor {
            active_workspace: workspace_idx(
              &monitor.active_workspace.name,
              monitor.active_workspace.id,
            ) as u32,
            name: monitor.name.as_str().into(),
            width: monitor.width as u32,
            height: monitor.height as u32,
            scale: if monitor.scale > 0.0 {
              monitor.scale as f64
            } else {
              1.0
            },
          },
        )
      })
      .collect();

    Ok(Void)
  }

  fn apply_event(&mut self, event: &Event) -> CompositorEvent {
    match event {
      Event::UrgentStateChanged(address) => {
        self.urgent.insert(address.clone());
        CompositorEvent::WORKSPACES
      }

      Event::ActiveWindowChanged(Some(window)) => {
        self.urgent.remove(&window.address);
        CompositorEvent::WINDOWS | CompositorEvent::FOCUS
      }

      Event::WindowClosed(address) => {
        self.urgent.remove(address);
        CompositorEvent::WINDOWS | CompositorEvent::FOCUS
      }

      _ => event_mask(event),
    }
  }

  fn drain_events(&mut self) -> Result<CompositorEvent> {
    let mut pending = Vec::new();
    if let Some(events) = self.events.as_ref()
      && let Ok(events) = events.lock()
    {
      while let Ok(event) = events.try_recv() {
        pending.push(event);
      }
    }

    let mut changed = CompositorEvent::NONE;
    for event in &pending {
      changed |= self.apply_event(event);
    }

    Ok(changed)
  }
}

impl Compositor for HyprlandCompositor {
  fn state(&self) -> &CompositorState {
    &self.state
  }

  fn send_cmd(&mut self, cmd: CompositorCommand) -> Result<Void> {
    match cmd {
      CompositorCommand::FocusWorkspace(idx) => {
        let name = self
          .state
          .workspaces
          .iter()
          .find(|workspace| workspace.idx == idx as u8)
          .and_then(|workspace| workspace.name.clone())
          .unwrap_or_else(|| idx.to_string());

        let identifier = match name.parse::<i32>() {
          Ok(id) => WorkspaceIdentifierWithSpecial::Id(id),
          Err(_) => WorkspaceIdentifierWithSpecial::Name(name.as_str()),
        };

        Dispatch::call(DispatchType::Workspace(identifier)).into_diagnostic()?;
        Ok(Void)
      }

      CompositorCommand::FocusWindow(id) => {
        Dispatch::call(DispatchType::FocusWindow(WindowIdentifier::Address(
          Address::new(format!("0x{id:x}")),
        )))
        .into_diagnostic()?;
        Ok(Void)
      }

      CompositorCommand::FocusMonitor(name) => {
        Dispatch::call(DispatchType::FocusMonitor(MonitorIdentifier::Name(&name)))
          .into_diagnostic()?;
        Ok(Void)
      }

      CompositorCommand::Custom(content) => {
        Dispatch::call(
          content
            .as_this::<DispatchType>()
            .cloned()
            .ok_or(miette::miette!(
              "hyprland failed to send command: unsupported action"
            ))?,
        )
        .into_diagnostic()?;
        Ok(Void)
      }
    }
  }

  fn is_active(&self, _config: &Config) -> bool {
    Instance::from_current_env().is_ok()
  }

  fn initialize(&mut self, _config: &Config, listeners: &mut Listeners) -> Result<Void> {
    let (wake, events) = spawn_event_bridge()?;
    wake.set_nonblocking(true).into_diagnostic()?;

    let fd = wake.as_raw_fd();
    listeners.flag(fd, EpollFlags::EPOLLIN | EpollFlags::EPOLLET);
    listeners.action(fd, ListenerAction::UpdateCompositor(CompositorEvent::ALL));

    self.wake = Some(wake);
    self.events = Some(Mutex::new(events));

    self.apply_compositor_state()
  }

  fn update_state(&mut self, _store: Option<&Store>) -> Result<CompositorEvent> {
    let mut closed = false;
    {
      let wake = self
        .wake
        .as_mut()
        .ok_or_else(|| miette::miette!("hyprland event stream is not initialized"))?;

      loop {
        let mut chunk = [0u8; 256];
        match wake.read(&mut chunk) {
          Ok(0) => {
            closed = true;
            break;
          }
          Ok(_) => {}
          Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
          Err(e) => return Err(e).into_diagnostic(),
        }
      }
    }

    if closed {
      self.wake = None;
      self.events = None;
      return Err(miette::miette!("hyprland event stream closed"));
    }

    let changed = self.drain_events()?;
    if changed.is_empty() {
      return Ok(CompositorEvent::NONE);
    }

    if changed.intersects(CompositorEvent::WINDOWS | CompositorEvent::FOCUS) {
      self.refresh_windows()?;
    }

    if changed.intersects(CompositorEvent::WORKSPACES | CompositorEvent::MONITORS) {
      self.refresh_workspaces()?;
    }

    Ok(changed)
  }
}
