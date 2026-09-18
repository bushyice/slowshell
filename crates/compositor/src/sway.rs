use std::{
  fmt::Debug,
  io::{Read, Write},
  os::{fd::AsRawFd, unix::net::UnixStream},
  path::PathBuf,
};

use futures_channel::mpsc::UnboundedSender;
use miette::{IntoDiagnostic, Result};
use nix::sys::epoll::EpollFlags;
use serde::{Deserialize, de::DeserializeOwned};
use slowshell_config::Config;
use slowshell_core::{
  Monitor, Store, Window, Workspace,
  listeners::{ListenerAction, Listeners},
  message::Message,
  types::{Ustr, Void},
};

use crate::{Compositor, CompositorCommand, CompositorState};

const IPC_MAGIC: &[u8] = b"i3-ipc";
const IPC_HEADER_LEN: usize = 14;

const RUN_COMMAND: u32 = 0;
const GET_WORKSPACES: u32 = 1;
const SUBSCRIBE: u32 = 2;
const GET_OUTPUTS: u32 = 3;
const GET_TREE: u32 = 4;

const EVENT_WORKSPACE: u32 = 0x8000_0000;
const EVENT_OUTPUT: u32 = 0x8000_0001;
const EVENT_WINDOW: u32 = 0x8000_0003;
const EVENT_SHUTDOWN: u32 = 0x8000_0006;

fn socket_path() -> Option<PathBuf> {
  std::env::var_os("SWAYSOCK").map(PathBuf::from)
}

#[derive(Debug, Deserialize)]
struct SwayWorkspace {
  #[serde(default)]
  num: i64,
  #[serde(default)]
  name: String,
  #[serde(default)]
  visible: bool,
  #[serde(default)]
  focused: bool,
  #[serde(default)]
  urgent: bool,
  #[serde(default)]
  output: String,
}

#[derive(Debug, Deserialize)]
struct SwayRect {
  #[serde(default)]
  width: i64,
  #[serde(default)]
  height: i64,
}

#[derive(Debug, Deserialize)]
struct SwayOutput {
  #[serde(default)]
  name: String,
  rect: SwayRect,
  #[serde(default)]
  scale: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct SwayWindowProps {
  #[serde(default)]
  class: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SwayNode {
  #[serde(default)]
  id: i64,
  #[serde(default)]
  name: Option<String>,
  #[serde(default)]
  app_id: Option<String>,
  #[serde(default)]
  focused: bool,
  #[serde(default)]
  window_properties: Option<SwayWindowProps>,
  #[serde(default)]
  nodes: Vec<SwayNode>,
  #[serde(default)]
  floating_nodes: Vec<SwayNode>,
}

impl SwayNode {
  fn class(&self) -> Option<String> {
    self.app_id.clone().or_else(|| {
      self
        .window_properties
        .as_ref()
        .and_then(|p| p.class.clone())
    })
  }

  fn is_window(&self) -> bool {
    self.app_id.is_some() || self.window_properties.is_some()
  }
}

fn collect_windows<'a>(node: &'a SwayNode, out: &mut Vec<&'a SwayNode>) {
  if node.is_window() {
    out.push(node);
  }

  for child in node.nodes.iter().chain(node.floating_nodes.iter()) {
    collect_windows(child, out);
  }
}

#[derive(Default)]
pub struct WlrCompositor {
  state: CompositorState,
  stream: Option<UnixStream>,
  cmd_socket: Option<UnixStream>,
  buf: Vec<u8>,
}

impl WlrCompositor {
  pub fn new() -> Self {
    Self::default()
  }

  fn get_or_connect_cmd_socket(&mut self) -> Result<&mut UnixStream> {
    if self.cmd_socket.is_none() {
      let path = socket_path().ok_or_else(|| miette::miette!("SWAYSOCK is not set"))?;
      self.cmd_socket = Some(UnixStream::connect(path).into_diagnostic()?);
    }

    Ok(self.cmd_socket.as_mut().unwrap())
  }

  fn query<T: DeserializeOwned>(&mut self, message_type: u32) -> Result<T> {
    match self.query_inner(message_type) {
      Ok(value) => Ok(value),
      Err(error) => {
        self.cmd_socket = None;
        Err(error)
      }
    }
  }

  fn query_inner<T: DeserializeOwned>(&mut self, message_type: u32) -> Result<T> {
    let stream = self.get_or_connect_cmd_socket()?;
    write_frame(stream, message_type, &[])?;

    let (_, payload) = read_frame(stream)?;
    serde_json::from_slice(&payload).into_diagnostic()
  }

  fn send_cmd_inner(&mut self, cmd: CompositorCommand) -> Result<Void> {
    let command = match cmd {
      CompositorCommand::FocusWorkspace(idx) => format!("workspace number {idx}"),
      CompositorCommand::FocusWindow(id) => format!("[con_id={id}] focus"),
    };

    let stream = self.get_or_connect_cmd_socket()?;
    let payload = serde_json::to_vec(&command).into_diagnostic()?;

    write_frame(stream, RUN_COMMAND, &payload)?;

    let (_, reply) = read_frame(stream)?;
    let reply: serde_json::Value = serde_json::from_slice(&reply).into_diagnostic()?;

    let results = match &reply {
      serde_json::Value::Array(results) => results.as_slice(),
      other => std::slice::from_ref(other),
    };

    if let Some(failure) = results
      .iter()
      .find(|result| result.get("success").and_then(serde_json::Value::as_bool) == Some(false))
    {
      return Err(miette::miette!(
        "sway rejected `{command}`: {}",
        failure
          .get("error")
          .and_then(serde_json::Value::as_str)
          .unwrap_or("unknown error")
      ));
    }

    Ok(Void)
  }

  fn apply_compositor_state(&mut self) -> Result<Void> {
    let workspaces: Vec<SwayWorkspace> = self.query(GET_WORKSPACES)?;
    let outputs: Vec<SwayOutput> = self.query(GET_OUTPUTS)?;
    let tree: SwayNode = self.query(GET_TREE)?;

    self.state.active_window = focused_window(&tree).map(|node| Window {
      id: node.id.max(0) as u64,
      title: node.name.clone().unwrap_or_default(),
      class: node.class().unwrap_or_default(),
      is_active: true,
      metadata: Default::default(),
    });

    let mut windows = Vec::new();
    collect_windows(&tree, &mut windows);

    self.state.active_windows = windows
      .into_iter()
      .map(|win| Window {
        id: win.id.max(0) as u64,
        title: win.name.clone().unwrap_or_default(),
        class: win.class().unwrap_or_default(),
        is_active: win.focused,
        metadata: Default::default(),
      })
      .collect();

    self.state.overview_active = false;

    self.state.workspaces = workspaces
      .iter()
      .map(|ws| Workspace {
        id: ws.num.max(0) as u64,
        idx: ws.num.clamp(0, u8::MAX as i64) as u8,
        name: (!ws.name.is_empty()).then(|| ws.name.clone()),
        output: (!ws.output.is_empty()).then(|| ws.output.clone()),
        is_urgent: ws.urgent,
        is_active: ws.visible,
        is_focused: ws.focused,
      })
      .collect();
    self
      .state
      .workspaces
      .sort_unstable_by(|a, b| a.output.cmp(&b.output).then_with(|| a.idx.cmp(&b.idx)));

    self.state.monitors = outputs
      .iter()
      .map(|output| {
        let active = workspaces
          .iter()
          .find(|ws| ws.output == output.name && ws.focused)
          .or_else(|| {
            workspaces
              .iter()
              .find(|ws| ws.output == output.name && ws.visible)
          });

        (
          Ustr::from(output.name.as_str()),
          Monitor {
            active_workspace: active.map(|ws| ws.num.max(0) as u32).unwrap_or(0),
            name: output.name.as_str().into(),
            width: output.rect.width.max(0) as u32,
            height: output.rect.height.max(0) as u32,
            scale: output.scale.filter(|scale| *scale > 0.0).unwrap_or(1.0),
          },
        )
      })
      .collect();

    Ok(Void)
  }
}

impl Compositor for WlrCompositor {
  fn state(&self) -> &CompositorState {
    &self.state
  }

  fn send_cmd(&mut self, cmd: CompositorCommand) -> Result<Void> {
    match self.send_cmd_inner(cmd) {
      Ok(void) => Ok(void),
      Err(error) => {
        self.cmd_socket = None;
        Err(error)
      }
    }
  }

  fn is_active(&self, _config: &Config) -> bool {
    socket_path().is_some()
  }

  fn initialize(&mut self, _config: &Config, listeners: &mut Listeners) -> Result<Void> {
    let path = socket_path().ok_or_else(|| miette::miette!("SWAYSOCK is not set"))?;

    let mut stream = UnixStream::connect(path).into_diagnostic()?;
    let subscribe = serde_json::to_vec(&["workspace", "window", "output", "mode", "shutdown"])
      .into_diagnostic()?;
    write_frame(&mut stream, SUBSCRIBE, &subscribe)?;

    let (_ty, reply) = read_frame(&mut stream)?;
    let reply: serde_json::Value = serde_json::from_slice(&reply).into_diagnostic()?;
    if reply.get("success").and_then(|value| value.as_bool()) != Some(true) {
      return Err(miette::miette!(
        "sway refused the event subscription: {reply}"
      ));
    }

    self.apply_compositor_state()?;

    stream.set_nonblocking(true).into_diagnostic()?;
    let fd = stream.as_raw_fd();
    listeners.flag(fd, EpollFlags::EPOLLIN | EpollFlags::EPOLLET);
    listeners.action(fd, ListenerAction::UpdateCompositor);
    self.stream = Some(stream);

    Ok(Void)
  }

  fn update_state(&mut self, _store: Option<&Store>, tx: UnboundedSender<Message>) -> Result<Void> {
    let mut closed = false;
    {
      let stream = self
        .stream
        .as_mut()
        .ok_or_else(|| miette::miette!("sway event stream is not initialized"))?;

      loop {
        let mut chunk = [0u8; 8192];
        match stream.read(&mut chunk) {
          Ok(0) => {
            closed = true;
            break;
          }
          Ok(read) => self.buf.extend_from_slice(&chunk[..read]),
          Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
          Err(e) => return Err(e).into_diagnostic(),
        }
      }
    }

    if closed {
      self.stream = None;
      return Err(miette::miette!("sway IPC stream closed"));
    }

    let mut dirty = false;
    let mut monitors_changed = false;

    while self.buf.len() >= IPC_HEADER_LEN {
      if &self.buf[..IPC_MAGIC.len()] != IPC_MAGIC {
        return Err(miette::miette!("invalid i3/sway IPC magic"));
      }

      let len = u32::from_le_bytes(self.buf[6..10].try_into().unwrap()) as usize;
      let message_type = u32::from_le_bytes(self.buf[10..14].try_into().unwrap());
      if self.buf.len() < IPC_HEADER_LEN + len {
        break;
      }
      self.buf.drain(0..IPC_HEADER_LEN + len);

      match message_type {
        EVENT_WORKSPACE | EVENT_OUTPUT => {
          dirty = true;
          monitors_changed = true;
        }
        EVENT_WINDOW => dirty = true,
        EVENT_SHUTDOWN => {
          self.stream = None;
          return Err(miette::miette!("sway is shutting down"));
        }
        _ => {}
      }
    }

    if monitors_changed {
      let _ = tx.unbounded_send(Message::UpdateMonitors);
    }
    if dirty {
      self.apply_compositor_state()?;
    }

    Ok(Void)
  }
}

fn focused_window(node: &SwayNode) -> Option<&SwayNode> {
  if node.focused && (node.app_id.is_some() || node.window_properties.is_some()) {
    return Some(node);
  }

  node
    .nodes
    .iter()
    .chain(node.floating_nodes.iter())
    .find_map(focused_window)
}

fn write_frame(stream: &mut UnixStream, message_type: u32, payload: &[u8]) -> Result<()> {
  let mut frame = Vec::with_capacity(IPC_HEADER_LEN + payload.len());
  frame.extend_from_slice(IPC_MAGIC);
  frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
  frame.extend_from_slice(&message_type.to_le_bytes());
  frame.extend_from_slice(payload);

  stream.write_all(&frame).into_diagnostic()?;
  stream.flush().into_diagnostic()?;

  Ok(())
}

fn read_frame(stream: &mut UnixStream) -> Result<(u32, Vec<u8>)> {
  let mut header = [0u8; IPC_HEADER_LEN];
  stream.read_exact(&mut header).into_diagnostic()?;

  if &header[..IPC_MAGIC.len()] != IPC_MAGIC {
    return Err(miette::miette!("invalid i3/sway IPC magic"));
  }

  let len = u32::from_le_bytes(header[6..10].try_into().unwrap()) as usize;
  let message_type = u32::from_le_bytes(header[10..14].try_into().unwrap());
  let mut payload = vec![0u8; len];
  stream.read_exact(&mut payload).into_diagnostic()?;

  Ok((message_type, payload))
}
