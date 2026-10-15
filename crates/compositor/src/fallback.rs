use std::os::fd::{AsFd, AsRawFd};

use miette::{IntoDiagnostic, Result};
use nix::sys::epoll::EpollFlags;
use wayland_client::{
  Connection, Dispatch, Proxy, QueueHandle, WEnum,
  backend::{ObjectId, WaylandError},
  event_created_child,
  protocol::{
    wl_output::{self, WlOutput},
    wl_registry::{self, WlRegistry},
    wl_seat::{self, WlSeat},
  },
};
use wayland_protocols::ext::workspace::v1::client::{
  ext_workspace_group_handle_v1::{self, ExtWorkspaceGroupHandleV1},
  ext_workspace_handle_v1::{self, ExtWorkspaceHandleV1},
  ext_workspace_manager_v1::{self, ExtWorkspaceManagerV1},
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
  zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
  zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
};

use slowshell_config::Config;
use slowshell_core::{
  Monitor, Store, Window, Workspace,
  listeners::{CompositorEvent, ListenerAction, Listeners},
  types::{Ustr, Void},
};

use crate::{Compositor, CompositorCommand, CompositorState};

fn parse_u32_array(data: &[u8]) -> Vec<u32> {
  data
    .chunks_exact(4)
    .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
    .collect()
}

fn toplevel_is_activated(state: &[u32]) -> bool {
  let activated = u32::from(zwlr_foreign_toplevel_handle_v1::State::Activated);
  state.contains(&activated)
}

fn workspace_entry_idx(name: &str, coordinates: &[u32]) -> u8 {
  name
    .parse::<i64>()
    .ok()
    .or_else(|| coordinates.first().map(|coordinate| *coordinate as i64))
    .unwrap_or(0)
    .clamp(0, u8::MAX as i64) as u8
}

struct OutputEntry {
  proxy: WlOutput,
  name: String,
  width: u32,
  height: u32,
  scale: f64,
}

struct GroupEntry {
  proxy: ExtWorkspaceGroupHandleV1,
  outputs: Vec<ObjectId>,
}

struct WorkspaceEntry {
  proxy: ExtWorkspaceHandleV1,
  id: u64,
  name: String,
  coordinates: Vec<u32>,
  state: u32,
  capabilities: u32,
  group: Option<ObjectId>,
}

impl WorkspaceEntry {
  fn idx(&self) -> u8 {
    workspace_entry_idx(&self.name, &self.coordinates)
  }

  fn is_active(&self) -> bool {
    self.state & ext_workspace_handle_v1::State::Active.bits() != 0
  }

  fn is_urgent(&self) -> bool {
    self.state & ext_workspace_handle_v1::State::Urgent.bits() != 0
  }
}

struct ToplevelEntry {
  proxy: ZwlrForeignToplevelHandleV1,
  id: u64,
  title: String,
  class: String,
  activated: bool,
}

#[derive(Default)]
struct WaylandState {
  manager: Option<ExtWorkspaceManagerV1>,
  toplevel_manager: Option<ZwlrForeignToplevelManagerV1>,
  seat: Option<WlSeat>,
  outputs: Vec<OutputEntry>,
  groups: Vec<GroupEntry>,
  workspaces: Vec<WorkspaceEntry>,
  toplevels: Vec<ToplevelEntry>,
  next_workspace_id: u64,
  next_toplevel_id: u64,
  dirty: CompositorEvent,
}

impl WaylandState {
  fn touch(&mut self, changed: CompositorEvent) {
    self.dirty.insert(changed);
  }

  fn take_dirty(&mut self) -> CompositorEvent {
    std::mem::take(&mut self.dirty)
  }

  fn find_workspace(&mut self, proxy: &ExtWorkspaceHandleV1) -> Option<&mut WorkspaceEntry> {
    self
      .workspaces
      .iter_mut()
      .find(|workspace| workspace.proxy.id() == proxy.id())
  }

  fn find_toplevel(&mut self, proxy: &ZwlrForeignToplevelHandleV1) -> Option<&mut ToplevelEntry> {
    self
      .toplevels
      .iter_mut()
      .find(|toplevel| toplevel.proxy.id() == proxy.id())
  }

  fn workspace_output(&self, workspace: &WorkspaceEntry) -> Option<String> {
    let group = self
      .groups
      .iter()
      .find(|group| Some(group.proxy.id()) == workspace.group)?;

    group
      .outputs
      .first()
      .and_then(|output| {
        self
          .outputs
          .iter()
          .find(|entry| entry.proxy.id() == *output)
      })
      .map(|output| output.name.clone())
  }

  fn output_workspace_idx(&self, output: &OutputEntry) -> u32 {
    self
      .groups
      .iter()
      .find(|group| group.outputs.contains(&output.proxy.id()))
      .and_then(|group| {
        self
          .workspaces
          .iter()
          .find(|workspace| workspace.group == Some(group.proxy.id()) && workspace.is_active())
      })
      .map(|workspace| workspace.idx() as u32)
      .unwrap_or(0)
  }

  fn sync_state(&self, state: &mut CompositorState) {
    state.monitors = self
      .outputs
      .iter()
      .map(|output| {
        (
          Ustr::from(output.name.as_str()),
          Monitor {
            active_workspace: self.output_workspace_idx(output),
            name: output.name.as_str().into(),
            width: output.width,
            height: output.height,
            scale: output.scale,
          },
        )
      })
      .collect();

    state.workspaces = self
      .workspaces
      .iter()
      .map(|workspace| Workspace {
        id: workspace.id,
        idx: workspace.idx(),
        name: (!workspace.name.is_empty()).then(|| workspace.name.clone()),
        output: self.workspace_output(workspace),
        is_urgent: workspace.is_urgent(),
        is_active: workspace.is_active(),
        is_focused: workspace.is_active(),
      })
      .collect();
    state
      .workspaces
      .sort_unstable_by(|a, b| a.output.cmp(&b.output).then_with(|| a.idx.cmp(&b.idx)));

    state.active_windows = self
      .toplevels
      .iter()
      .map(|toplevel| Window {
        id: toplevel.id,
        title: toplevel.title.clone(),
        class: toplevel.class.clone(),
        is_active: toplevel.activated,
        metadata: Default::default(),
      })
      .collect();
    state.active_window = self
      .toplevels
      .iter()
      .find(|toplevel| toplevel.activated)
      .map(|toplevel| Window {
        id: toplevel.id,
        title: toplevel.title.clone(),
        class: toplevel.class.clone(),
        is_active: true,
        metadata: Default::default(),
      });

    state.overview_active = false;
  }
}

impl Dispatch<WlRegistry, ()> for WaylandState {
  fn event(
    state: &mut Self,
    registry: &WlRegistry,
    event: wl_registry::Event,
    _data: &(),
    _conn: &Connection,
    qhandle: &QueueHandle<Self>,
  ) {
    let wl_registry::Event::Global {
      name,
      interface,
      version,
    } = event
    else {
      return;
    };

    match interface.as_str() {
      "wl_output" => {
        let proxy = registry.bind::<WlOutput, _, _>(name, version.min(4), qhandle, ());
        state.outputs.push(OutputEntry {
          proxy,
          name: format!("WL-{name}"),
          width: 0,
          height: 0,
          scale: 1.0,
        });
        state.touch(CompositorEvent::MONITORS);
      }

      "wl_seat" => {
        state.seat = Some(registry.bind::<WlSeat, _, _>(name, version.min(1), qhandle, ()));
      }

      "ext_workspace_manager_v1" => {
        state.manager =
          Some(registry.bind::<ExtWorkspaceManagerV1, _, _>(name, version.min(1), qhandle, ()));
        state.touch(CompositorEvent::WORKSPACES | CompositorEvent::MONITORS);
      }

      "zwlr_foreign_toplevel_manager_v1" => {
        state.toplevel_manager = Some(registry.bind::<ZwlrForeignToplevelManagerV1, _, _>(
          name,
          version.min(3),
          qhandle,
          (),
        ));
        state.touch(CompositorEvent::WINDOWS | CompositorEvent::FOCUS);
      }

      _ => {}
    }
  }
}

impl Dispatch<WlOutput, ()> for WaylandState {
  fn event(
    state: &mut Self,
    proxy: &WlOutput,
    event: wl_output::Event,
    _data: &(),
    _conn: &Connection,
    _qhandle: &QueueHandle<Self>,
  ) {
    let Some(output) = state
      .outputs
      .iter_mut()
      .find(|output| output.proxy.id() == proxy.id())
    else {
      return;
    };

    match event {
      wl_output::Event::Name { name } => output.name = name,
      wl_output::Event::Scale { factor } => output.scale = factor.max(1) as f64,
      wl_output::Event::Mode {
        flags,
        width,
        height,
        ..
      } => {
        if matches!(flags, WEnum::Value(mode) if mode.contains(wl_output::Mode::Current)) {
          output.width = width.max(0) as u32;
          output.height = height.max(0) as u32;
        }
      }
      _ => return,
    }

    state.touch(CompositorEvent::MONITORS);
  }
}

impl Dispatch<WlSeat, ()> for WaylandState {
  fn event(
    _state: &mut Self,
    _proxy: &WlSeat,
    _event: wl_seat::Event,
    _data: &(),
    _conn: &Connection,
    _qhandle: &QueueHandle<Self>,
  ) {
  }
}

impl Dispatch<ExtWorkspaceManagerV1, ()> for WaylandState {
  fn event(
    state: &mut Self,
    _proxy: &ExtWorkspaceManagerV1,
    event: ext_workspace_manager_v1::Event,
    _data: &(),
    _conn: &Connection,
    _qhandle: &QueueHandle<Self>,
  ) {
    match event {
      ext_workspace_manager_v1::Event::WorkspaceGroup { workspace_group } => {
        state.groups.push(GroupEntry {
          proxy: workspace_group,
          outputs: Vec::new(),
        });
        state.touch(CompositorEvent::WORKSPACES | CompositorEvent::MONITORS);
      }

      ext_workspace_manager_v1::Event::Workspace { workspace } => {
        state.next_workspace_id += 1;
        state.workspaces.push(WorkspaceEntry {
          proxy: workspace,
          id: state.next_workspace_id,
          name: String::new(),
          coordinates: Vec::new(),
          state: 0,
          capabilities: 0,
          group: None,
        });
        state.touch(CompositorEvent::WORKSPACES);
      }

      ext_workspace_manager_v1::Event::Finished => {
        state.manager = None;
        state.touch(CompositorEvent::WORKSPACES | CompositorEvent::MONITORS);
      }

      _ => {}
    }
  }

  event_created_child!(WaylandState, ExtWorkspaceManagerV1, [
    ext_workspace_manager_v1::EVT_WORKSPACE_GROUP_OPCODE => (ExtWorkspaceGroupHandleV1, ()),
    ext_workspace_manager_v1::EVT_WORKSPACE_OPCODE => (ExtWorkspaceHandleV1, ()),
  ]);
}

impl Dispatch<ExtWorkspaceGroupHandleV1, ()> for WaylandState {
  fn event(
    state: &mut Self,
    proxy: &ExtWorkspaceGroupHandleV1,
    event: ext_workspace_group_handle_v1::Event,
    _data: &(),
    _conn: &Connection,
    _qhandle: &QueueHandle<Self>,
  ) {
    let group_id = proxy.id();

    match event {
      ext_workspace_group_handle_v1::Event::OutputEnter { output } => {
        if let Some(group) = state
          .groups
          .iter_mut()
          .find(|group| group.proxy.id() == group_id)
        {
          group.outputs.push(output.id());
          state.touch(CompositorEvent::MONITORS | CompositorEvent::WORKSPACES);
        }
      }

      ext_workspace_group_handle_v1::Event::OutputLeave { output } => {
        if let Some(group) = state
          .groups
          .iter_mut()
          .find(|group| group.proxy.id() == group_id)
        {
          group.outputs.retain(|id| *id != output.id());
          state.touch(CompositorEvent::MONITORS | CompositorEvent::WORKSPACES);
        }
      }

      ext_workspace_group_handle_v1::Event::WorkspaceEnter { workspace } => {
        if let Some(entry) = state.find_workspace(&workspace) {
          entry.group = Some(group_id);
          state.touch(CompositorEvent::WORKSPACES | CompositorEvent::MONITORS);
        }
      }

      ext_workspace_group_handle_v1::Event::WorkspaceLeave { workspace } => {
        if let Some(entry) = state.find_workspace(&workspace) {
          entry.group = None;
          state.touch(CompositorEvent::WORKSPACES | CompositorEvent::MONITORS);
        }
      }

      ext_workspace_group_handle_v1::Event::Removed => {
        state.groups.retain(|group| group.proxy.id() != group_id);
        state.touch(CompositorEvent::WORKSPACES | CompositorEvent::MONITORS);
      }

      _ => {}
    }
  }
}

impl Dispatch<ExtWorkspaceHandleV1, ()> for WaylandState {
  fn event(
    state: &mut Self,
    proxy: &ExtWorkspaceHandleV1,
    event: ext_workspace_handle_v1::Event,
    _data: &(),
    _conn: &Connection,
    _qhandle: &QueueHandle<Self>,
  ) {
    match event {
      ext_workspace_handle_v1::Event::Name { name } => {
        if let Some(entry) = state.find_workspace(proxy) {
          entry.name = name;
          state.touch(CompositorEvent::WORKSPACES);
        }
      }

      ext_workspace_handle_v1::Event::Coordinates { coordinates } => {
        if let Some(entry) = state.find_workspace(proxy) {
          entry.coordinates = parse_u32_array(&coordinates);
          state.touch(CompositorEvent::WORKSPACES);
        }
      }

      ext_workspace_handle_v1::Event::State { state: workspace } => {
        if let Some(entry) = state.find_workspace(proxy) {
          entry.state = u32::from(workspace);
          state.touch(CompositorEvent::WORKSPACES | CompositorEvent::MONITORS);
        }
      }

      ext_workspace_handle_v1::Event::Capabilities { capabilities } => {
        if let Some(entry) = state.find_workspace(proxy) {
          entry.capabilities = u32::from(capabilities);
        }
      }

      ext_workspace_handle_v1::Event::Removed => {
        state
          .workspaces
          .retain(|entry| entry.proxy.id() != proxy.id());
        state.touch(CompositorEvent::WORKSPACES | CompositorEvent::MONITORS);
      }

      _ => {}
    }
  }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for WaylandState {
  fn event(
    state: &mut Self,
    _proxy: &ZwlrForeignToplevelManagerV1,
    event: zwlr_foreign_toplevel_manager_v1::Event,
    _data: &(),
    _conn: &Connection,
    _qhandle: &QueueHandle<Self>,
  ) {
    match event {
      zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } => {
        state.next_toplevel_id += 1;
        state.toplevels.push(ToplevelEntry {
          proxy: toplevel,
          id: state.next_toplevel_id,
          title: String::new(),
          class: String::new(),
          activated: false,
        });
        state.touch(CompositorEvent::WINDOWS | CompositorEvent::FOCUS);
      }

      zwlr_foreign_toplevel_manager_v1::Event::Finished => {
        state.toplevel_manager = None;
        state.touch(CompositorEvent::WINDOWS | CompositorEvent::FOCUS);
      }

      _ => {}
    }
  }

  event_created_child!(WaylandState, ZwlrForeignToplevelManagerV1, [
    zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ()),
  ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for WaylandState {
  fn event(
    state: &mut Self,
    proxy: &ZwlrForeignToplevelHandleV1,
    event: zwlr_foreign_toplevel_handle_v1::Event,
    _data: &(),
    _conn: &Connection,
    _qhandle: &QueueHandle<Self>,
  ) {
    match event {
      zwlr_foreign_toplevel_handle_v1::Event::Title { title } => {
        if let Some(entry) = state.find_toplevel(proxy) {
          entry.title = title;
          state.touch(CompositorEvent::WINDOWS);
        }
      }

      zwlr_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
        if let Some(entry) = state.find_toplevel(proxy) {
          entry.class = app_id;
          state.touch(CompositorEvent::WINDOWS);
        }
      }

      zwlr_foreign_toplevel_handle_v1::Event::State { state: window } => {
        if let Some(entry) = state.find_toplevel(proxy) {
          entry.activated = toplevel_is_activated(&parse_u32_array(&window));
          state.touch(CompositorEvent::WINDOWS | CompositorEvent::FOCUS);
        }
      }

      zwlr_foreign_toplevel_handle_v1::Event::Closed => {
        state
          .toplevels
          .retain(|entry| entry.proxy.id() != proxy.id());
        state.touch(CompositorEvent::WINDOWS | CompositorEvent::FOCUS);
      }

      _ => {}
    }
  }
}

#[derive(Default)]
pub struct FallbackCompositor {
  state: CompositorState,
  conn: Option<Connection>,
  queue: Option<wayland_client::EventQueue<WaylandState>>,
  inner: WaylandState,
}

impl FallbackCompositor {
  pub fn new() -> Self {
    Self::default()
  }

  fn flush(&self) -> Result<Void> {
    let conn = self
      .conn
      .as_ref()
      .ok_or_else(|| miette::miette!("wayland is not initialized"))?;

    conn.flush().into_diagnostic()?;
    Ok(Void)
  }

  fn activate_workspace(&self, entry: &WorkspaceEntry) -> Result<Void> {
    let manager = self
      .inner
      .manager
      .clone()
      .ok_or_else(|| miette::miette!("ext-workspace is not available on this compositor"))?;

    let activate = ext_workspace_handle_v1::WorkspaceCapabilities::Activate;
    if entry.capabilities & activate.bits() == 0 {
      return Err(miette::miette!(
        "workspace {} cannot be activated",
        entry.idx()
      ));
    }

    entry.proxy.activate();
    manager.commit();
    self.flush()
  }
}

impl Compositor for FallbackCompositor {
  fn state(&self) -> &CompositorState {
    &self.state
  }

  fn send_cmd(&mut self, cmd: CompositorCommand) -> Result<Void> {
    match cmd {
      CompositorCommand::FocusWorkspace(idx) => {
        let entry = self
          .inner
          .workspaces
          .iter()
          .find(|workspace| workspace.idx() == idx as u8)
          .ok_or_else(|| miette::miette!("workspace {idx} was not found"))?;

        self.activate_workspace(entry)
      }

      CompositorCommand::FocusWindow(id) => {
        let seat = self
          .inner
          .seat
          .clone()
          .ok_or_else(|| miette::miette!("no wl_seat is available to focus windows"))?;

        let entry = self
          .inner
          .toplevels
          .iter()
          .find(|toplevel| toplevel.id == id)
          .ok_or_else(|| miette::miette!("window {id} was not found"))?;

        entry.proxy.activate(&seat);
        self.flush()
      }

      CompositorCommand::FocusMonitor(name) => {
        let entry =
          self
            .inner
            .outputs
            .iter()
            .find(|output| output.name == name)
            .and_then(|output| {
              self
                .inner
                .groups
                .iter()
                .find(|group| group.outputs.contains(&output.proxy.id()))
            })
            .and_then(|group| {
              self.inner.workspaces.iter().find(|workspace| {
                workspace.group == Some(group.proxy.id()) && workspace.is_active()
              })
            })
            .ok_or_else(|| miette::miette!("no active workspace on monitor {name}"))?;

        self.activate_workspace(entry)
      }

      CompositorCommand::Custom(_) => Err(miette::miette!(
        "fallback compositor does not support custom commands"
      )),
    }
  }

  fn is_active(&self, _config: &Config) -> bool {
    let Ok(conn) = Connection::connect_to_env() else {
      return false;
    };

    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let _registry = conn.display().get_registry(&qh, ());

    let mut probe = WaylandState::default();
    queue.roundtrip(&mut probe).is_ok()
      && (probe.manager.is_some() || probe.toplevel_manager.is_some())
  }

  fn initialize(&mut self, _config: &Config, listeners: &mut Listeners) -> Result<Void> {
    let conn = Connection::connect_to_env().into_diagnostic()?;
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let _registry = conn.display().get_registry(&qh, ());

    queue.roundtrip(&mut self.inner).into_diagnostic()?;
    queue.roundtrip(&mut self.inner).into_diagnostic()?;
    self.inner.take_dirty();

    let fd = conn.as_fd().as_raw_fd();
    listeners.flag(fd, EpollFlags::EPOLLIN | EpollFlags::EPOLLET);
    listeners.action(fd, ListenerAction::UpdateCompositor(CompositorEvent::ALL));

    self.inner.sync_state(&mut self.state);
    self.conn = Some(conn);
    self.queue = Some(queue);

    Ok(Void)
  }

  fn update_state(&mut self, _store: Option<&Store>) -> Result<CompositorEvent> {
    let queue = self
      .queue
      .as_mut()
      .ok_or_else(|| miette::miette!("wayland is not initialized"))?;

    loop {
      let Some(guard) = queue.prepare_read() else {
        queue.dispatch_pending(&mut self.inner).into_diagnostic()?;
        continue;
      };

      match guard.read() {
        Ok(_) => {}
        Err(WaylandError::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock => break,
        Err(WaylandError::Io(e)) => return Err(e).into_diagnostic(),
        Err(e) => return Err(miette::miette!("wayland connection failed: {e}")),
      }
    }

    queue.dispatch_pending(&mut self.inner).into_diagnostic()?;

    let changed = self.inner.take_dirty();
    if changed.is_empty() {
      return Ok(CompositorEvent::NONE);
    }

    self.inner.sync_state(&mut self.state);

    Ok(changed)
  }
}
