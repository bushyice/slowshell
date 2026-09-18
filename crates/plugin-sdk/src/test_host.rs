use crate::*;
use core::ffi::c_void;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicPtr, Ordering};

const CTX: *mut c_void = core::ptr::NonNull::<c_void>::dangling().as_ptr();

enum Register {
  Component(String, *const SlComponentVtable),
  Renderable(String, *const SlRenderableVtable),
  Payload(String, *const SlPayloadVtable),
  DesktopItem(String, *const SlDesktopItemVtable),
  Compositor(String, *const SlCompositorVtable),
}

#[derive(Default)]
struct State {
  config: HashMap<String, String>,
  options: HashMap<String, String>,
  monitor: Option<String>,
  styles: HashMap<String, StyleSheet>,
  theme: Theme,
  compositor: Option<CompositorState>,
  registers: Vec<Register>,
  timers: Vec<(u32, bool, u32)>,
  fds: Vec<(i32, u32)>,
  redraws: u32,
  logs: Vec<(i32, String)>,
  errors: Vec<(u32, String)>,
  notifications: Vec<Notification>,
  parsers: Vec<(String, SlConfigParserFn)>,
  canvases: Vec<Vec<u8>>,
  compositor_pushes: usize,
  compositor_fds: Vec<(i32, u32)>,
  spotlights: Vec<(String, *const SlSpotlightVtable)>,
  registries: HashMap<usize, usize>,
  registry_subs: Vec<(usize, usize, SlRegistryNotifyFn)>,
  dispatched: Vec<String>,
  audio: Option<AudioState>,
  bluetooth: Option<BluetoothState>,
  network: Option<NetworkState>,
  system: Option<SystemState>,
  tray: Option<TrayState>,
  power: Option<PowerState>,
  persistence: HashMap<String, String>,
}

thread_local! {
  static STATE: RefCell<State> = RefCell::new(State::default());
  static TEXT_STAGE: RefCell<Option<Box<str>>> = const { RefCell::new(None) };
  static STYLE_STAGE: RefCell<Option<(Vec<String>, Vec<SlStyleSheetEntry>)>> =
    const { RefCell::new(None) };
  static COMPOSITOR_STAGE: RefCell<Option<CompositorStage>> = const { RefCell::new(None) };
}

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
  STATE.with(|state| f(&mut state.borrow_mut()))
}

fn stage_text(value: &str) -> SlStr {
  TEXT_STAGE.with(|cell| {
    let mut staged = cell.borrow_mut();
    *staged = Some(value.to_owned().into_boxed_str());
    let value = staged.as_ref().expect("just stored");
    SlStr {
      ptr: value.as_ptr(),
      len: value.len(),
    }
  })
}

fn theme_to_sl(theme: &Theme) -> SlTheme {
  let c = |color: Color| SlColor {
    r: color.r,
    g: color.g,
    b: color.b,
    a: color.a,
  };
  SlTheme {
    base: c(theme.base),
    crust: c(theme.crust),
    mantle: c(theme.mantle),
    primary: c(theme.primary),
    secondary: c(theme.secondary),
    green: c(theme.green),
    red: c(theme.red),
    blue: c(theme.blue),
    yellow: c(theme.yellow),
    orange: c(theme.orange),
    text: c(theme.text),
    subtext: c(theme.subtext),
    overlay: c(theme.overlay),
  }
}

#[derive(Default)]
struct CompositorStage {
  #[allow(dead_code)]
  strings: Vec<Box<[u8]>>,
  monitors: Vec<SlMonitor>,
  workspaces: Vec<SlWorkspace>,
  window: Option<SlWindow>,
  state: SlCompositorState,
}

fn stage_str(strings: &mut Vec<Box<[u8]>>, value: Option<&str>) -> SlStr {
  match value {
    Some(value) => {
      let bytes = value.as_bytes().to_vec().into_boxed_slice();
      let string = SlStr {
        ptr: bytes.as_ptr(),
        len: bytes.len(),
      };
      strings.push(bytes);
      string
    }
    None => SlStr::EMPTY,
  }
}

impl CompositorStage {
  fn new(state: &CompositorState) -> Self {
    let mut strings = Vec::new();

    let mut monitors = Vec::with_capacity(state.monitors.len());
    for monitor in &state.monitors {
      let name = stage_str(&mut strings, Some(&monitor.name));
      monitors.push(SlMonitor {
        name,
        width: monitor.width,
        height: monitor.height,
        scale: monitor.scale,
      });
    }

    let mut workspaces = Vec::with_capacity(state.workspaces.len());
    for workspace in &state.workspaces {
      let output = stage_str(&mut strings, workspace.output.as_deref());
      let name = stage_str(&mut strings, workspace.name.as_deref());
      workspaces.push(SlWorkspace {
        id: workspace.id,
        idx: workspace.idx,
        output,
        name,
        is_active: workspace.is_active,
        is_focused: workspace.is_focused,
        is_urgent: workspace.is_urgent,
      });
    }

    let window = state.active_window.as_ref().map(|window| SlWindow {
      title: stage_str(&mut strings, Some(&window.title)),
      wclass: stage_str(&mut strings, Some(&window.class)),
    });

    let mut staged = CompositorStage {
      strings,
      monitors,
      workspaces,
      window,
      state: SlCompositorState::default(),
    };
    staged.state = SlCompositorState {
      monitors: staged.monitors.as_ptr(),
      monitor_count: staged.monitors.len(),
      workspace_count: staged.workspaces.len(),
      workspaces: staged.workspaces.as_ptr(),
      active_window: staged
        .window
        .as_ref()
        .map_or(core::ptr::null(), |window| window as *const SlWindow),
      overview_active: state.overview_active,
    };
    staged
  }
}

fn record_register(register: Register) {
  with_state(|state| state.registers.push(register));
}

unsafe extern "C" fn mock_register_component(
  _host: *mut c_void,
  name: SlStr,
  vtable: *const SlComponentVtable,
  _userdata: *mut c_void,
) {
  let name = unsafe { name.as_str() }.unwrap_or_default().to_owned();
  record_register(Register::Component(name, vtable));
}

unsafe extern "C" fn mock_register_compositor(
  _host: *mut c_void,
  name: SlStr,
  vtable: *const SlCompositorVtable,
  _userdata: *mut c_void,
) {
  let name = unsafe { name.as_str() }.unwrap_or_default().to_owned();
  record_register(Register::Compositor(name, vtable));
}

unsafe extern "C" fn mock_register_payload(
  _host: *mut c_void,
  command: SlStr,
  vtable: *const SlPayloadVtable,
  _userdata: *mut c_void,
) {
  let command = unsafe { command.as_str() }.unwrap_or_default().to_owned();
  record_register(Register::Payload(command, vtable));
}

unsafe extern "C" fn mock_register_renderable(
  _host: *mut c_void,
  name: SlStr,
  vtable: *const SlRenderableVtable,
  _userdata: *mut c_void,
) {
  let name = unsafe { name.as_str() }.unwrap_or_default().to_owned();
  record_register(Register::Renderable(name, vtable));
}

unsafe extern "C" fn mock_register_desktop_item(
  _host: *mut c_void,
  name: SlStr,
  vtable: *const SlDesktopItemVtable,
  _userdata: *mut c_void,
) {
  let name = unsafe { name.as_str() }.unwrap_or_default().to_owned();
  record_register(Register::DesktopItem(name, vtable));
}

unsafe extern "C" fn mock_register_spotlight(
  _host: *mut c_void,
  name: SlStr,
  vtable: *const SlSpotlightVtable,
  _userdata: *mut c_void,
) {
  let name = unsafe { name.as_str() }.unwrap_or_default().to_owned();
  with_state(|state| state.spotlights.push((name, vtable)));
}

unsafe extern "C" fn mock_request_redraw(_ctx: *mut c_void, _window: u64) {
  with_state(|state| state.redraws += 1);
}

unsafe extern "C" fn mock_register_fd(_ctx: *mut c_void, fd: i32, action: u32) -> i32 {
  with_state(|state| state.fds.push((fd, action)));
  0
}

unsafe extern "C" fn mock_set_interval(
  _ctx: *mut c_void,
  millis: u32,
  repeating: bool,
  tag: u32,
) -> i32 {
  with_state(|state| state.timers.push((millis, repeating, tag)));
  0
}

unsafe extern "C" fn mock_log(_ctx: *mut c_void, level: i32, message: SlStr) {
  let message = unsafe { message.as_str() }.unwrap_or_default().to_owned();
  with_state(|state| state.logs.push((level, message)));
}

unsafe extern "C" fn mock_get_str(_bag: *mut c_void, key: SlStr, out: *mut SlStr) -> i32 {
  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };
  if out.is_null() {
    return -1;
  }
  let value = with_state(|state| {
    if key == "monitor" {
      state.monitor.clone()
    } else {
      state.options.get(key).cloned()
    }
  });
  match value {
    Some(value) => {
      unsafe { *out = stage_text(&value) };
      0
    }
    None => -1,
  }
}

unsafe extern "C" fn mock_get_f64(_bag: *mut c_void, key: SlStr, out: *mut f64) -> i32 {
  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };
  let value = with_state(|state| state.options.get(key).cloned());
  match value.and_then(|v| v.parse().ok()) {
    Some(value) if !out.is_null() => {
      unsafe { *out = value };
      0
    }
    _ => -1,
  }
}

unsafe extern "C" fn mock_get_i64(_bag: *mut c_void, key: SlStr, out: *mut i64) -> i32 {
  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };
  let value = with_state(|state| state.options.get(key).cloned());
  match value.and_then(|v| v.parse().ok()) {
    Some(value) if !out.is_null() => {
      unsafe { *out = value };
      0
    }
    _ => -1,
  }
}

unsafe extern "C" fn mock_get_bool(_bag: *mut c_void, key: SlStr, out: *mut bool) -> i32 {
  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };
  let value = with_state(|state| state.options.get(key).cloned());
  match value.and_then(|v| v.parse().ok()) {
    Some(value) if !out.is_null() => {
      unsafe { *out = value };
      0
    }
    _ => -1,
  }
}

unsafe extern "C" fn mock_set_compositor_state(
  _ctx: *mut c_void,
  _state: *const SlCompositorState,
) {
  with_state(|state| state.compositor_pushes += 1);
}

unsafe extern "C" fn mock_compositor_register_fd(_ctx: *mut c_void, fd: i32, flags: u32) -> i32 {
  if fd < 0 {
    return -1;
  }
  with_state(|state| state.compositor_fds.push((fd, flags)));
  0
}

fn notify_registry(key: usize, value: *mut c_void) {
  let subscribers: Vec<(usize, SlRegistryNotifyFn)> = with_state(|state| {
    state
      .registry_subs
      .iter()
      .filter(|(registered, _, _)| *registered == key)
      .map(|(_, ctx, callback)| (*ctx, *callback))
      .collect()
  });
  for (ctx, callback) in subscribers {
    unsafe { callback(ctx as *mut c_void, key, value) };
  }
}

unsafe extern "C" fn mock_registry_register(
  _ctx: *mut c_void,
  key: usize,
  value: *mut c_void,
) -> i32 {
  let taken = with_state(|state| match state.registries.entry(key) {
    std::collections::hash_map::Entry::Occupied(_) => true,
    std::collections::hash_map::Entry::Vacant(entry) => {
      entry.insert(value as usize);
      false
    }
  });
  if taken {
    return -1;
  }
  notify_registry(key, value);
  0
}

unsafe extern "C" fn mock_registry_get(_ctx: *mut c_void, key: usize) -> *mut c_void {
  with_state(|state| {
    state
      .registries
      .get(&key)
      .map(|value| *value as *mut c_void)
      .unwrap_or(core::ptr::null_mut())
  })
}

unsafe extern "C" fn mock_registry_remove(_ctx: *mut c_void, key: usize) -> *mut c_void {
  match with_state(|state| state.registries.remove(&key)) {
    Some(value) => {
      notify_registry(key, core::ptr::null_mut());
      value as *mut c_void
    }
    None => core::ptr::null_mut(),
  }
}

unsafe extern "C" fn mock_registry_subscribe(
  ctx: *mut c_void,
  key: usize,
  callback: SlRegistryNotifyFn,
) -> i32 {
  let existing = with_state(|state| {
    state
      .registries
      .get(&key)
      .map(|value| *value as *mut c_void)
  });
  with_state(|state| state.registry_subs.push((key, ctx as usize, callback)));
  if let Some(value) = existing {
    unsafe { callback(ctx, key, value) };
  }
  0
}

unsafe extern "C" fn mock_canvas(_ctx: *mut c_void, width: u32, height: u32) -> *mut u8 {
  if width == 0 || height == 0 {
    return core::ptr::null_mut();
  }
  let mut buffer = vec![0u8; width as usize * height as usize * 4];
  let ptr = buffer.as_mut_ptr();
  with_state(|state| state.canvases.push(buffer));
  ptr
}

unsafe extern "C" fn mock_config_get_str(_ctx: *mut c_void, key: SlStr, out: *mut SlStr) -> i32 {
  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };
  if out.is_null() {
    return -1;
  }
  match with_state(|state| state.config.get(key).cloned()) {
    Some(value) => {
      unsafe { *out = stage_text(&value) };
      0
    }
    None => -1,
  }
}

unsafe extern "C" fn mock_config_get_f64(_ctx: *mut c_void, key: SlStr, out: *mut f64) -> i32 {
  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };
  match with_state(|state| state.config.get(key).cloned()).and_then(|v| v.parse().ok()) {
    Some(value) if !out.is_null() => {
      unsafe { *out = value };
      0
    }
    _ => -1,
  }
}

unsafe extern "C" fn mock_config_get_i64(_ctx: *mut c_void, key: SlStr, out: *mut i64) -> i32 {
  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };
  match with_state(|state| state.config.get(key).cloned()).and_then(|v| v.parse().ok()) {
    Some(value) if !out.is_null() => {
      unsafe { *out = value };
      0
    }
    _ => -1,
  }
}

unsafe extern "C" fn mock_config_get_bool(_ctx: *mut c_void, key: SlStr, out: *mut bool) -> i32 {
  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };
  match with_state(|state| state.config.get(key).cloned()).and_then(|v| v.parse().ok()) {
    Some(value) if !out.is_null() => {
      unsafe { *out = value };
      0
    }
    _ => -1,
  }
}

unsafe extern "C" fn mock_style_get(_ctx: *mut c_void, name: SlStr, out: *mut SlStyleSheet) -> i32 {
  if out.is_null() {
    return -1;
  }
  let Some(name) = (unsafe { name.as_str() }) else {
    return -1;
  };
  let style = with_state(|state| state.styles.get(name).cloned());
  let Some(style) = style else {
    return -1;
  };

  STYLE_STAGE.with(|cell| {
    let mut staged = cell.borrow_mut();
    *staged = Some(style.to_sl_entries());
    let (_, entries) = staged.as_ref().expect("just stored");
    unsafe {
      *out = SlStyleSheet {
        entries: entries.as_ptr(),
        count: entries.len() as u32,
      }
    };
  });
  0
}

unsafe extern "C" fn mock_theme_get(_ctx: *mut c_void, out: *mut SlTheme) -> i32 {
  if out.is_null() {
    return -1;
  }
  let theme = with_state(|state| state.theme);
  unsafe { *out = theme_to_sl(&theme) };
  0
}

unsafe extern "C" fn mock_style_register(
  _ctx: *mut c_void,
  name: SlStr,
  sheet: *const SlStyleSheet,
) -> i32 {
  let Some(name) = (unsafe { name.as_str() }) else {
    return -1;
  };
  if sheet.is_null() {
    return -1;
  }
  let style = StyleSheet::from_sl(unsafe { &*sheet });
  with_state(|state| state.styles.insert(name.to_owned(), style));
  0
}

unsafe extern "C" fn mock_compositor_state_get(
  _ctx: *mut c_void,
  out: *mut SlCompositorState,
) -> i32 {
  if out.is_null() {
    return -1;
  }
  let state = with_state(|state| state.compositor.clone());
  let Some(state) = state else {
    return -1;
  };
  COMPOSITOR_STAGE.with(|cell| {
    let mut staged = cell.borrow_mut();
    *staged = Some(CompositorStage::new(&state));
    let staged = staged.as_ref().expect("just stored");
    unsafe { *out = staged.state };
  });
  0
}

unsafe extern "C" fn mock_config_parser_register(
  _host: *mut c_void,
  name: SlStr,
  callback: SlConfigParserFn,
) -> i32 {
  let Some(name) = (unsafe { name.as_str() }) else {
    return -1;
  };
  with_state(|state| state.parsers.push((name.to_owned(), callback)));
  0
}

unsafe extern "C" fn mock_plugin_error(_ctx: *mut c_void, code: u32, message: SlStr) {
  let message = unsafe { message.as_str() }.unwrap_or_default().to_owned();
  with_state(|state| state.errors.push((code, message)));
}

unsafe extern "C" fn mock_notify(_ctx: *mut c_void, notification: *const SlNotification) -> i32 {
  let Some(notification) = (unsafe { notification.as_ref() }) else {
    return -1;
  };

  let mut message = Notification::new(
    unsafe { notification.summary.as_str() }.unwrap_or_default(),
    unsafe { notification.body.as_str() }.unwrap_or_default(),
  );
  message.urgency = notification.urgency;
  message.timeout_ms = notification.timeout_ms;
  if let Some(app_name) = unsafe { notification.app_name.as_str() } {
    message = message.app_name(app_name);
  }
  if let Some(icon) = unsafe { notification.app_icon.as_str() } {
    message = message.icon(icon);
  }
  if !notification.actions.is_null() {
    for index in 0..notification.action_count {
      let action = unsafe { &*notification.actions.add(index) };
      let (Some(command), Some(label)) = (unsafe { action.command.as_str() }, unsafe {
        action.label.as_str()
      }) else {
        continue;
      };
      message = message.action(label, command);
    }
  }

  with_state(|state| {
    state.notifications.push(message);
    state.notifications.len() as i32
  })
}

thread_local! {
  static SERVICE_STAGE: RefCell<Option<ServiceStage>> = const { RefCell::new(None) };
}

#[allow(dead_code)]
enum ServiceStage {
  Audio {
    strings: Vec<Box<[u8]>>,
    sinks: Vec<SlAudioSink>,
    state: SlAudioState,
  },
  Bluetooth {
    strings: Vec<Box<[u8]>>,
    devices: Vec<SlBluetoothDevice>,
    state: SlBluetoothState,
  },
  Network {
    strings: Vec<Box<[u8]>>,
    networks: Vec<SlAccessPoint>,
    state: SlNetworkState,
  },
  System {
    strings: Vec<Box<[u8]>>,
    processes: Vec<SlProcessInfo>,
    state: SlSystemState,
  },
  Tray {
    strings: Vec<Box<[u8]>>,
    items: Vec<SlTrayItem>,
    state: SlTrayState,
  },
  Power {
    strings: Vec<Box<[u8]>>,
    profiles: Vec<SlStr>,
    state: SlPowerState,
  },
}

fn audio_stage(snapshot: &AudioState) -> ServiceStage {
  let mut strings = Vec::new();
  let default_sink = stage_str(&mut strings, snapshot.default_sink.as_deref());
  let mut sinks = Vec::with_capacity(snapshot.sinks.len());
  for sink in &snapshot.sinks {
    sinks.push(SlAudioSink {
      id: sink.id,
      name: stage_str(&mut strings, Some(&sink.name)),
      description: stage_str(&mut strings, Some(&sink.description)),
      volume: sink.volume,
      muted: sink.muted,
      is_default: sink.is_default,
    });
  }
  let player = snapshot.player.as_ref().map(|player| SlMprisPlayer {
    identity: stage_str(&mut strings, Some(&player.identity)),
    title: stage_str(&mut strings, Some(&player.title)),
    artist: stage_str(&mut strings, Some(&player.artist)),
    album: stage_str(&mut strings, Some(&player.album)),
    art_url: stage_str(&mut strings, player.art_url.as_deref()),
    playback_status: stage_str(&mut strings, Some(&player.playback_status)),
    can_play_pause: player.can_play_pause,
    can_go_next: player.can_go_next,
    can_go_previous: player.can_go_previous,
  });

  let mut stage = ServiceStage::Audio {
    strings,
    sinks,
    state: SlAudioState::default(),
  };
  if let ServiceStage::Audio { sinks, state, .. } = &mut stage {
    *state = SlAudioState {
      volume: snapshot.volume,
      muted: snapshot.muted,
      default_sink,
      sinks: sinks.as_ptr(),
      sink_count: sinks.len(),
      has_player: player.is_some(),
      player: player.unwrap_or_default(),
    };
  }
  stage
}

fn bluetooth_stage(snapshot: &BluetoothState) -> ServiceStage {
  let mut strings = Vec::new();
  let adapter_name = stage_str(&mut strings, snapshot.adapter_name.as_deref());
  let mut devices = Vec::with_capacity(snapshot.devices.len());
  for device in &snapshot.devices {
    devices.push(SlBluetoothDevice {
      address: stage_str(&mut strings, Some(&device.address)),
      name: stage_str(&mut strings, Some(&device.name)),
      icon: stage_str(&mut strings, device.icon.as_deref()),
      paired: device.paired,
      connected: device.connected,
      trusted: device.trusted,
      battery: device.battery.map(i32::from).unwrap_or(-1),
      rssi: device.rssi.map(i32::from).unwrap_or(i32::MIN),
    });
  }

  let mut stage = ServiceStage::Bluetooth {
    strings,
    devices,
    state: SlBluetoothState::default(),
  };
  if let ServiceStage::Bluetooth { devices, state, .. } = &mut stage {
    *state = SlBluetoothState {
      powered: snapshot.powered,
      discovering: snapshot.discovering,
      adapter_name,
      devices: devices.as_ptr(),
      device_count: devices.len(),
    };
  }
  stage
}

fn network_stage(snapshot: &NetworkState) -> ServiceStage {
  let mut strings = Vec::new();
  let access_point = |strings: &mut Vec<Box<[u8]>>, ap: &AccessPoint| SlAccessPoint {
    ssid: stage_str(strings, Some(&ap.ssid)),
    signal: ap.signal,
    secured: ap.secured,
    in_use: ap.in_use,
    saved: ap.saved,
  };
  let wifi_connected = snapshot
    .wifi_connected
    .as_ref()
    .map(|ap| access_point(&mut strings, ap));
  let ethernet = snapshot.ethernet.as_ref().map(|eth| SlEthernet {
    iface: stage_str(&mut strings, Some(&eth.iface)),
    speed: eth.speed,
    carrier: eth.carrier,
    connected: eth.connected,
  });
  let mut networks = Vec::with_capacity(snapshot.networks.len());
  for ap in &snapshot.networks {
    networks.push(access_point(&mut strings, ap));
  }
  let connected = snapshot.connected.as_ref().map(|info| SlConnectionInfo {
    label: stage_str(&mut strings, Some(&info.label)),
    mac: stage_str(&mut strings, Some(&info.mac)),
    is_wifi: info.is_wifi,
  });

  let mut stage = ServiceStage::Network {
    strings,
    networks,
    state: SlNetworkState::default(),
  };
  if let ServiceStage::Network {
    networks, state, ..
  } = &mut stage
  {
    *state = SlNetworkState {
      wifi_enabled: snapshot.wifi_enabled,
      wifi_hardware_enabled: snapshot.wifi_hardware_enabled,
      scanning: snapshot.scanning,
      busy: snapshot.busy,
      has_wifi_connected: wifi_connected.is_some(),
      wifi_connected: wifi_connected.unwrap_or_default(),
      has_ethernet: ethernet.is_some(),
      ethernet: ethernet.unwrap_or_default(),
      networks: networks.as_ptr(),
      network_count: networks.len(),
      has_connected: connected.is_some(),
      connected: connected.unwrap_or_default(),
    };
  }
  stage
}

fn system_stage(snapshot: &SystemState) -> ServiceStage {
  let mut strings = Vec::new();
  let mut processes = Vec::with_capacity(snapshot.processes.len());
  for process in &snapshot.processes {
    processes.push(SlProcessInfo {
      pid: process.pid,
      name: stage_str(&mut strings, Some(&process.name)),
      cpu_usage: process.cpu_usage,
      memory: process.memory,
    });
  }

  let mut stage = ServiceStage::System {
    strings,
    processes,
    state: SlSystemState::default(),
  };
  if let ServiceStage::System {
    processes, state, ..
  } = &mut stage
  {
    *state = SlSystemState {
      cpu_usage: snapshot.cpu_usage,
      mem_usage: snapshot.mem_usage,
      mem_total: snapshot.mem_total,
      mem_used: snapshot.mem_used,
      has_temperature: snapshot.temperature.is_some(),
      temperature: snapshot.temperature.unwrap_or(0.0),
      load: snapshot.load,
      network_state: snapshot.network_state,
      network_rx: snapshot.network_rx,
      network_tx: snapshot.network_tx,
      top_processes: processes.as_ptr(),
      process_count: processes.len(),
    };
  }
  stage
}

fn tray_stage(snapshot: &TrayState) -> ServiceStage {
  let mut strings = Vec::new();
  let mut items = Vec::with_capacity(snapshot.items.len());
  for item in &snapshot.items {
    let (has_pixmap, width, height, pixmap) = match &item.pixmap {
      Some((width, height, bytes)) => (
        true,
        *width,
        *height,
        SlStr {
          ptr: bytes.as_ptr(),
          len: bytes.len(),
        },
      ),
      None => (false, 0, 0, SlStr::EMPTY),
    };
    items.push(SlTrayItem {
      address: stage_str(&mut strings, Some(&item.address)),
      title: stage_str(&mut strings, Some(&item.title)),
      icon_name: stage_str(&mut strings, item.icon_name.as_deref()),
      menu_path: stage_str(&mut strings, item.menu_path.as_deref()),
      has_pixmap,
      pixmap_width: width,
      pixmap_height: height,
      pixmap,
    });
  }

  let mut stage = ServiceStage::Tray {
    strings,
    items,
    state: SlTrayState::default(),
  };
  if let ServiceStage::Tray { items, state, .. } = &mut stage {
    *state = SlTrayState {
      items: items.as_ptr(),
      item_count: items.len(),
    };
  }
  stage
}

fn power_stage(snapshot: &PowerState) -> ServiceStage {
  let mut strings = Vec::new();
  let status = stage_str(&mut strings, Some(&snapshot.status));
  let time_remaining = stage_str(&mut strings, snapshot.time_remaining.as_deref());
  let active_profile = stage_str(&mut strings, snapshot.active_profile.as_deref());
  let device_name = stage_str(&mut strings, Some(&snapshot.device_name));
  let mut profiles = Vec::with_capacity(snapshot.available_profiles.len());
  for profile in &snapshot.available_profiles {
    profiles.push(stage_str(&mut strings, Some(profile)));
  }

  let mut stage = ServiceStage::Power {
    strings,
    profiles,
    state: SlPowerState::default(),
  };
  if let ServiceStage::Power {
    profiles, state, ..
  } = &mut stage
  {
    *state = SlPowerState {
      has_percent: snapshot.percent.is_some(),
      percent: snapshot.percent.unwrap_or(0),
      charging: snapshot.charging,
      status,
      has_health: snapshot.health.is_some(),
      health: snapshot.health.unwrap_or(0),
      has_energy_now: snapshot.energy_now_wh.is_some(),
      energy_now_wh: snapshot.energy_now_wh.unwrap_or(0.0),
      has_energy_full: snapshot.energy_full_wh.is_some(),
      energy_full_wh: snapshot.energy_full_wh.unwrap_or(0.0),
      has_power: snapshot.power_w.is_some(),
      power_w: snapshot.power_w.unwrap_or(0.0),
      time_remaining,
      has_active_profile: snapshot.active_profile.is_some(),
      active_profile,
      available_profiles: profiles.as_ptr(),
      available_profile_count: profiles.len(),
      brightness_percent: snapshot.brightness_percent,
      brightness_max: snapshot.brightness_max,
      brightness_current: snapshot.brightness_current,
      device_name,
    };
  }
  stage
}

unsafe extern "C" fn mock_dispatch(_host: *mut c_void, command: SlStr) -> i32 {
  let command = unsafe { command.as_str() }.unwrap_or_default().to_owned();
  with_state(|state| state.dispatched.push(command));
  0
}

unsafe extern "C" fn mock_audio_state_get(_host: *mut c_void, out: *mut SlAudioState) -> i32 {
  if out.is_null() {
    return -1;
  }
  let Some(stage) = with_state(|state| state.audio.as_ref().map(audio_stage)) else {
    return -1;
  };
  SERVICE_STAGE.with(|cell| *cell.borrow_mut() = Some(stage));
  SERVICE_STAGE.with(|cell| {
    if let Some(ServiceStage::Audio { state, .. }) = cell.borrow().as_ref() {
      unsafe { *out = *state };
    }
  });
  0
}

unsafe extern "C" fn mock_bluetooth_state_get(
  _host: *mut c_void,
  out: *mut SlBluetoothState,
) -> i32 {
  if out.is_null() {
    return -1;
  }
  let Some(stage) = with_state(|state| state.bluetooth.as_ref().map(bluetooth_stage)) else {
    return -1;
  };
  SERVICE_STAGE.with(|cell| *cell.borrow_mut() = Some(stage));
  SERVICE_STAGE.with(|cell| {
    if let Some(ServiceStage::Bluetooth { state, .. }) = cell.borrow().as_ref() {
      unsafe { *out = *state };
    }
  });
  0
}

unsafe extern "C" fn mock_network_state_get(_host: *mut c_void, out: *mut SlNetworkState) -> i32 {
  if out.is_null() {
    return -1;
  }
  let Some(stage) = with_state(|state| state.network.as_ref().map(network_stage)) else {
    return -1;
  };
  SERVICE_STAGE.with(|cell| *cell.borrow_mut() = Some(stage));
  SERVICE_STAGE.with(|cell| {
    if let Some(ServiceStage::Network { state, .. }) = cell.borrow().as_ref() {
      unsafe { *out = *state };
    }
  });
  0
}

unsafe extern "C" fn mock_system_state_get(_host: *mut c_void, out: *mut SlSystemState) -> i32 {
  if out.is_null() {
    return -1;
  }
  let Some(stage) = with_state(|state| state.system.as_ref().map(system_stage)) else {
    return -1;
  };
  SERVICE_STAGE.with(|cell| *cell.borrow_mut() = Some(stage));
  SERVICE_STAGE.with(|cell| {
    if let Some(ServiceStage::System { state, .. }) = cell.borrow().as_ref() {
      unsafe { *out = *state };
    }
  });
  0
}

unsafe extern "C" fn mock_tray_state_get(_host: *mut c_void, out: *mut SlTrayState) -> i32 {
  if out.is_null() {
    return -1;
  }
  let Some(stage) = with_state(|state| state.tray.as_ref().map(tray_stage)) else {
    return -1;
  };
  SERVICE_STAGE.with(|cell| *cell.borrow_mut() = Some(stage));
  SERVICE_STAGE.with(|cell| {
    if let Some(ServiceStage::Tray { state, .. }) = cell.borrow().as_ref() {
      unsafe { *out = *state };
    }
  });
  0
}

unsafe extern "C" fn mock_power_state_get(_host: *mut c_void, out: *mut SlPowerState) -> i32 {
  if out.is_null() {
    return -1;
  }
  let Some(stage) = with_state(|state| state.power.as_ref().map(power_stage)) else {
    return -1;
  };
  SERVICE_STAGE.with(|cell| *cell.borrow_mut() = Some(stage));
  SERVICE_STAGE.with(|cell| {
    if let Some(ServiceStage::Power { state, .. }) = cell.borrow().as_ref() {
      unsafe { *out = *state };
    }
  });
  0
}

unsafe extern "C" fn mock_persistence_get(_host: *mut c_void, key: SlStr, out: *mut SlStr) -> i32 {
  if out.is_null() {
    return -1;
  }
  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };
  let Some(value) = with_state(|state| state.persistence.get(key).cloned()) else {
    return -1;
  };
  unsafe { *out = stage_text(&value) };
  0
}

unsafe extern "C" fn mock_persistence_set(_host: *mut c_void, key: SlStr, value: SlStr) -> i32 {
  let (Some(key), Some(value)) = (unsafe { key.as_str() }, unsafe { value.as_str() }) else {
    return -1;
  };
  with_state(|state| state.persistence.insert(key.to_owned(), value.to_owned()));
  0
}

unsafe extern "C" fn mock_persistence_remove(_host: *mut c_void, key: SlStr) -> i32 {
  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };
  with_state(|state| state.persistence.remove(key));
  0
}

static MOCK_API: SlHostApi = SlHostApi {
  size: core::mem::size_of::<SlHostApi>() as u32,
  register_component: Some(mock_register_component),
  register_compositor: Some(mock_register_compositor),
  request_redraw: Some(mock_request_redraw),
  register_fd: Some(mock_register_fd),
  log: Some(mock_log),
  get_str: Some(mock_get_str),
  get_f64: Some(mock_get_f64),
  get_i64: Some(mock_get_i64),
  get_bool: Some(mock_get_bool),
  set_compositor_state: Some(mock_set_compositor_state),
  set_interval: Some(mock_set_interval),
  register_payload: Some(mock_register_payload),
  register_renderable: Some(mock_register_renderable),
  register_desktop_item: Some(mock_register_desktop_item),
  canvas: Some(mock_canvas),
  config_get_str: Some(mock_config_get_str),
  config_get_f64: Some(mock_config_get_f64),
  config_get_i64: Some(mock_config_get_i64),
  config_get_bool: Some(mock_config_get_bool),
  style_get: Some(mock_style_get),
  theme_get: Some(mock_theme_get),
  style_register: Some(mock_style_register),
  compositor_state_get: Some(mock_compositor_state_get),
  config_parser_register: Some(mock_config_parser_register),
  plugin_error: Some(mock_plugin_error),
  notify: Some(mock_notify),
  compositor_register_fd: Some(mock_compositor_register_fd),
  register_spotlight: Some(mock_register_spotlight),
  registry_register: Some(mock_registry_register),
  registry_get: Some(mock_registry_get),
  registry_remove: Some(mock_registry_remove),
  registry_subscribe: Some(mock_registry_subscribe),
  dispatch: Some(mock_dispatch),
  audio_state_get: Some(mock_audio_state_get),
  bluetooth_state_get: Some(mock_bluetooth_state_get),
  network_state_get: Some(mock_network_state_get),
  system_state_get: Some(mock_system_state_get),
  tray_state_get: Some(mock_tray_state_get),
  power_state_get: Some(mock_power_state_get),
  persistence_get: Some(mock_persistence_get),
  persistence_set: Some(mock_persistence_set),
  persistence_remove: Some(mock_persistence_remove),
};

fn install() {
  static INSTALLED: AtomicPtr<SlHostApi> = AtomicPtr::new(core::ptr::null_mut());
  if INSTALLED.load(Ordering::Relaxed).is_null() {
    INSTALLED.store(&MOCK_API as *const _ as *mut SlHostApi, Ordering::Relaxed);
    set_api(&MOCK_API);
  }
}

pub struct Harness;

impl Default for Harness {
  fn default() -> Self {
    Self::new()
  }
}

impl Harness {
  pub fn new() -> Self {
    install();
    STATE.with(|state| *state.borrow_mut() = State::default());
    Harness
  }

  pub fn config(&self, key: &str, value: impl Into<String>) -> &Self {
    with_state(|state| state.config.insert(key.to_owned(), value.into()));
    self
  }

  pub fn option(&self, key: &str, value: impl Into<String>) -> &Self {
    with_state(|state| state.options.insert(key.to_owned(), value.into()));
    self
  }

  pub fn monitor(&self, name: &str) -> &Self {
    with_state(|state| state.monitor = Some(name.to_owned()));
    self
  }

  pub fn style(&self, name: &str, sheet: StyleSheet) -> &Self {
    with_state(|state| state.styles.insert(name.to_owned(), sheet));
    self
  }

  pub fn theme(&self, theme: Theme) -> &Self {
    with_state(|state| state.theme = theme);
    self
  }

  pub fn set_compositor(&self, state: CompositorState) -> &Self {
    with_state(|s| s.compositor = Some(state));
    self
  }

  pub fn register(&self, f: impl FnOnce(&mut Registrar)) {
    let mut registrar = Registrar::new(&MOCK_API, CTX);
    f(&mut registrar);
  }

  pub fn reload(&self, kdl: &str) -> Vec<(String, i32)> {
    let document: kdl::KdlDocument = kdl.parse().expect("test kdl");
    let parsers: Vec<(String, SlConfigParserFn)> = with_state(|state| state.parsers.clone());

    let mut results = Vec::new();
    for (name, callback) in parsers {
      let node = document
        .nodes()
        .iter()
        .find(|node| node.name().value() == name);
      let result = match node {
        Some(node) => {
          let text = node.to_string();
          unsafe { callback(CTX, SlStr::from_str(&name), SlStr::from_str(&text)) }
        }
        None => unsafe { callback(CTX, SlStr::from_str(&name), SlStr::EMPTY) },
      };
      results.push((name, result));
    }
    results
  }

  pub fn component(&self, name: &str) -> Option<ComponentHandle> {
    let vtable = with_state(|state| {
      state.registers.iter().find_map(|register| match register {
        Register::Component(registered, vtable) if registered == name => Some(*vtable),
        _ => None,
      })
    })?;
    ComponentHandle::new(vtable)
  }

  pub fn renderable(&self, name: &str) -> Option<RenderableHandle> {
    let vtable = with_state(|state| {
      state.registers.iter().find_map(|register| match register {
        Register::Renderable(registered, vtable) if registered == name => Some(*vtable),
        _ => None,
      })
    })?;
    RenderableHandle::new(vtable)
  }

  pub fn desktop_item(&self, name: &str) -> Option<DesktopItemHandle> {
    let vtable = with_state(|state| {
      state.registers.iter().find_map(|register| match register {
        Register::DesktopItem(registered, vtable) if registered == name => Some(*vtable),
        _ => None,
      })
    })?;
    DesktopItemHandle::new(vtable)
  }

  pub fn payload(&self, command: &str) -> Option<PayloadHandle> {
    let vtable = with_state(|state| {
      state.registers.iter().find_map(|register| match register {
        Register::Payload(registered, vtable) if registered == command => Some(*vtable),
        _ => None,
      })
    })?;
    PayloadHandle::new(vtable)
  }

  pub fn compositor(&self, name: &str) -> Option<CompositorHandle> {
    let vtable = with_state(|state| {
      state.registers.iter().find_map(|register| match register {
        Register::Compositor(registered, vtable) if registered == name => Some(*vtable),
        _ => None,
      })
    })?;
    CompositorHandle::new(vtable)
  }

  pub fn spotlight(&self, name: &str) -> Option<SpotlightHandle> {
    let vtable = with_state(|state| {
      state
        .spotlights
        .iter()
        .find(|(registered, _)| registered == name)
        .map(|(_, vtable)| *vtable)
    })?;
    SpotlightHandle::new(vtable)
  }

  pub fn registered_components(&self) -> Vec<String> {
    with_state(|state| {
      state
        .registers
        .iter()
        .filter_map(|register| match register {
          Register::Component(name, _) => Some(name.clone()),
          _ => None,
        })
        .collect()
    })
  }

  pub fn timers(&self) -> Vec<(u32, bool, u32)> {
    with_state(|state| state.timers.clone())
  }

  pub fn fds(&self) -> Vec<(i32, u32)> {
    with_state(|state| state.fds.clone())
  }

  pub fn redraws(&self) -> u32 {
    with_state(|state| state.redraws)
  }

  pub fn logs(&self) -> Vec<(i32, String)> {
    with_state(|state| state.logs.clone())
  }

  pub fn errors(&self) -> Vec<(u32, String)> {
    with_state(|state| state.errors.clone())
  }

  pub fn notifications(&self) -> Vec<Notification> {
    with_state(|state| state.notifications.clone())
  }

  pub fn compositor_pushes(&self) -> usize {
    with_state(|state| state.compositor_pushes)
  }

  pub fn compositor_fds(&self) -> Vec<(i32, u32)> {
    with_state(|state| state.compositor_fds.clone())
  }

  pub fn with_registry<T, R>(&self, key: usize, f: impl FnOnce(&mut T) -> R) -> Option<R> {
    let pointer = with_state(|state| {
      state
        .registries
        .get(&key)
        .map(|value| *value as *mut c_void)
        .unwrap_or(core::ptr::null_mut())
    });
    if pointer.is_null() {
      None
    } else {
      Some(f(unsafe { &mut *(pointer as *mut T) }))
    }
  }

  pub fn dispatched(&self) -> Vec<String> {
    with_state(|state| state.dispatched.clone())
  }

  pub fn audio(&self, state: AudioState) -> &Self {
    with_state(|mock| mock.audio = Some(state));
    self
  }

  pub fn bluetooth(&self, state: BluetoothState) -> &Self {
    with_state(|mock| mock.bluetooth = Some(state));
    self
  }

  pub fn network(&self, state: NetworkState) -> &Self {
    with_state(|mock| mock.network = Some(state));
    self
  }

  pub fn system(&self, state: SystemState) -> &Self {
    with_state(|mock| mock.system = Some(state));
    self
  }

  pub fn tray(&self, state: TrayState) -> &Self {
    with_state(|mock| mock.tray = Some(state));
    self
  }

  pub fn power(&self, state: PowerState) -> &Self {
    with_state(|mock| mock.power = Some(state));
    self
  }
}

fn effect_from_sl_effect(effect: SlEffect) -> ItemEffect {
  effect_from_sl(effect.code, effect.custom)
}

fn event_to_sl(event: &Event) -> SlEvent {
  match event {
    Event::Tick { tag, fd } => SlEvent {
      kind: sl_event_kind::TICK,
      action: *tag,
      fd: *fd,
      ..SlEvent::default()
    },
    Event::Fd { action, fd } => SlEvent {
      kind: sl_event_kind::FD,
      action: *action,
      fd: *fd,
      ..SlEvent::default()
    },
    Event::ConfigReload => SlEvent {
      kind: sl_event_kind::CONFIG_RELOAD,
      ..SlEvent::default()
    },
    Event::CompositorUpdate => SlEvent {
      kind: sl_event_kind::COMPOSITOR_UPDATE,
      ..SlEvent::default()
    },
    Event::Frame => SlEvent {
      kind: sl_event_kind::FRAME,
      ..SlEvent::default()
    },
    Event::Other(kind) => SlEvent {
      kind: *kind,
      ..SlEvent::default()
    },
  }
}

fn message_to_sl(message: &Message) -> (SlItemMessage, Option<String>) {
  let mut sl = SlItemMessage::default();
  let mut keep = None;
  match message {
    Message::Noop => sl.kind = sl_message_kind::NOOP,
    Message::Effect(effect) => {
      sl.kind = sl_message_kind::EFFECT;
      let effect = effect.to_sl();
      sl.effect = effect.code;
      sl.custom = effect.custom;
    }
    Message::Action(name) => {
      sl.kind = sl_message_kind::ACTION;
      sl.name = SlStr::from_str(name);
      keep = Some(name.clone());
    }
    Message::EffectAction(effect, name) => {
      sl.kind = sl_message_kind::EFFECT_ACTION;
      let effect = effect.to_sl();
      sl.effect = effect.code;
      sl.custom = effect.custom;
      sl.name = SlStr::from_str(name);
      keep = Some(name.clone());
    }
  }
  (sl, keep)
}

pub struct ComponentHandle {
  ctx: *mut c_void,
  vtable: *const SlComponentVtable,
  state: *mut c_void,
}

impl ComponentHandle {
  fn new(vtable: *const SlComponentVtable) -> Option<Self> {
    let create = unsafe { (*vtable).create }?;
    let state = unsafe { create(CTX) };
    Some(Self {
      ctx: CTX,
      vtable,
      state,
    })
  }

  pub fn events(&self) -> EventMask {
    match unsafe { (*self.vtable).events } {
      Some(events) => EventMask(unsafe { events(self.ctx, self.state) }),
      None => EventMask::NONE,
    }
  }

  pub fn watch(&self) {
    if let Some(watch) = unsafe { (*self.vtable).watch } {
      unsafe { watch(self.ctx, self.state, CTX) };
    }
  }

  pub fn update(&self, event: &Event) -> ItemEffect {
    let Some(update) = (unsafe { (*self.vtable).update }) else {
      return ItemEffect::None;
    };
    let event = event_to_sl(event);
    effect_from_sl_effect(unsafe { update(self.ctx, self.state, &event) })
  }

  pub fn check_view(&self) -> bool {
    match unsafe { (*self.vtable).check_view } {
      Some(check) => unsafe { check(self.ctx, self.state, CTX) },
      None => true,
    }
  }

  pub fn stop(&self) {
    if let Some(stop) = unsafe { (*self.vtable).stop } {
      unsafe { stop(self.ctx, self.state) };
    }
  }

  pub fn view(&self) -> TestNode {
    let mut list = SlNodeList::EMPTY;
    if let Some(view) = unsafe { (*self.vtable).view } {
      unsafe { view(self.ctx, self.state, CTX, &mut list) };
    }
    TestNode::from_list(&list)
  }
}

impl Drop for ComponentHandle {
  fn drop(&mut self) {
    if let Some(destroy) = unsafe { (*self.vtable).destroy } {
      unsafe { destroy(self.ctx, self.state) };
    }
  }
}

pub struct RenderableHandle {
  ctx: *mut c_void,
  vtable: *const SlRenderableVtable,
  state: *mut c_void,
}

impl RenderableHandle {
  fn new(vtable: *const SlRenderableVtable) -> Option<Self> {
    let create = unsafe { (*vtable).create }?;
    let state = unsafe { create(CTX) };
    Some(Self {
      ctx: CTX,
      vtable,
      state,
    })
  }

  pub fn settings(&self) -> SlRenderableSettings {
    let mut settings = SlRenderableSettings::default();
    if let Some(settings_fn) = unsafe { (*self.vtable).settings } {
      unsafe { settings_fn(self.ctx, self.state, &mut settings) };
    }
    settings
  }

  pub fn initialize(&self) {
    if let Some(initialize) = unsafe { (*self.vtable).initialize } {
      unsafe { initialize(self.ctx, self.state) };
    }
  }

  pub fn update(&self) -> ItemEffect {
    match unsafe { (*self.vtable).update } {
      Some(update) => effect_from_sl_effect(unsafe { update(self.ctx, self.state) }),
      None => ItemEffect::None,
    }
  }

  pub fn handle_message(&self, message: &Message) -> ItemEffect {
    let Some(handle) = (unsafe { (*self.vtable).handle_message }) else {
      return ItemEffect::None;
    };
    let (message, _keep) = message_to_sl(message);
    effect_from_sl_effect(unsafe { handle(self.ctx, self.state, &message) })
  }

  pub fn view(&self) -> TestNode {
    let mut list = SlNodeList::EMPTY;
    if let Some(view) = unsafe { (*self.vtable).view } {
      unsafe { view(self.ctx, self.state, CTX, &mut list) };
    }
    TestNode::from_list(&list)
  }
}

impl Drop for RenderableHandle {
  fn drop(&mut self) {
    if let Some(destroy) = unsafe { (*self.vtable).destroy } {
      unsafe { destroy(self.ctx, self.state) };
    }
  }
}

pub struct DesktopItemHandle {
  ctx: *mut c_void,
  vtable: *const SlDesktopItemVtable,
  state: *mut c_void,
}

impl DesktopItemHandle {
  fn new(vtable: *const SlDesktopItemVtable) -> Option<Self> {
    let create = unsafe { (*vtable).create }?;
    let state = unsafe { create(CTX) };
    Some(Self {
      ctx: CTX,
      vtable,
      state,
    })
  }

  pub fn settings(&self) -> SlDesktopSettings {
    let mut settings = SlDesktopSettings::default();
    if let Some(settings_fn) = unsafe { (*self.vtable).settings } {
      unsafe { settings_fn(self.ctx, self.state, &mut settings) };
    }
    settings
  }

  pub fn events(&self) -> EventMask {
    match unsafe { (*self.vtable).events } {
      Some(events) => EventMask(unsafe { events(self.ctx, self.state) }),
      None => EventMask::NONE,
    }
  }

  pub fn initialize(&self) {
    if let Some(initialize) = unsafe { (*self.vtable).initialize } {
      unsafe { initialize(self.ctx, self.state, CTX) };
    }
  }

  pub fn update(&self, event: &Event) -> ItemEffect {
    let Some(update) = (unsafe { (*self.vtable).update }) else {
      return ItemEffect::None;
    };
    let event = event_to_sl(event);
    effect_from_sl_effect(unsafe { update(self.ctx, self.state, &event) })
  }

  pub fn handle_message(&self, message: &Message) -> ItemEffect {
    let Some(handle) = (unsafe { (*self.vtable).handle_message }) else {
      return ItemEffect::None;
    };
    let (message, _keep) = message_to_sl(message);
    effect_from_sl_effect(unsafe { handle(self.ctx, self.state, &message) })
  }

  pub fn view(&self) -> TestNode {
    let mut list = SlNodeList::EMPTY;
    if let Some(view) = unsafe { (*self.vtable).view } {
      unsafe { view(self.ctx, self.state, CTX, &mut list) };
    }
    TestNode::from_list(&list)
  }
}

impl Drop for DesktopItemHandle {
  fn drop(&mut self) {
    if let Some(destroy) = unsafe { (*self.vtable).destroy } {
      unsafe { destroy(self.ctx, self.state) };
    }
  }
}

pub struct PayloadHandle {
  ctx: *mut c_void,
  vtable: *const SlPayloadVtable,
  state: *mut c_void,
}

impl PayloadHandle {
  fn new(vtable: *const SlPayloadVtable) -> Option<Self> {
    let create = unsafe { (*vtable).create }?;
    let state = unsafe { create(CTX) };
    Some(Self {
      ctx: CTX,
      vtable,
      state,
    })
  }

  pub fn invoke(&self, args: &[(&str, &str)]) {
    let Some(invoke) = (unsafe { (*self.vtable).invoke }) else {
      return;
    };
    let keys: Vec<SlStr> = args.iter().map(|(key, _)| SlStr::from_str(key)).collect();
    let values: Vec<SlStr> = args
      .iter()
      .map(|(_, value)| SlStr::from_str(value))
      .collect();
    let sl = SlPayloadArgs {
      count: args.len(),
      keys: if keys.is_empty() {
        core::ptr::null()
      } else {
        keys.as_ptr()
      },
      values: if values.is_empty() {
        core::ptr::null()
      } else {
        values.as_ptr()
      },
    };
    unsafe { invoke(self.ctx, self.state, &sl as *const _ as *mut c_void) };
  }
}

impl Drop for PayloadHandle {
  fn drop(&mut self) {
    if let Some(destroy) = unsafe { (*self.vtable).destroy } {
      unsafe { destroy(self.ctx, self.state) };
    }
  }
}

pub struct CompositorHandle {
  ctx: *mut c_void,
  vtable: *const SlCompositorVtable,
  state: *mut c_void,
}

impl CompositorHandle {
  fn new(vtable: *const SlCompositorVtable) -> Option<Self> {
    let create = unsafe { (*vtable).create }?;
    let state = unsafe { create(CTX) };
    Some(Self {
      ctx: CTX,
      vtable,
      state,
    })
  }

  pub fn is_active(&self) -> bool {
    match unsafe { (*self.vtable).is_active } {
      Some(is_active) => unsafe { is_active(self.ctx, self.state) },
      None => false,
    }
  }

  pub fn initialize(&self) {
    if let Some(initialize) = unsafe { (*self.vtable).initialize } {
      unsafe { initialize(self.ctx, self.state) };
    }
  }

  pub fn update_state(&self) {
    if let Some(update) = unsafe { (*self.vtable).update_state } {
      unsafe { update(self.ctx, self.state) };
    }
  }

  pub fn send_command(&self, command: u32, arg: i32) {
    if let Some(send) = unsafe { (*self.vtable).send_command } {
      unsafe { send(self.ctx, self.state, command, arg) };
    }
  }
}

impl Drop for CompositorHandle {
  fn drop(&mut self) {
    if let Some(destroy) = unsafe { (*self.vtable).destroy } {
      unsafe { destroy(self.ctx, self.state) };
    }
  }
}

pub struct SpotlightHandle {
  ctx: *mut c_void,
  vtable: *const SlSpotlightVtable,
  state: *mut c_void,
}

impl SpotlightHandle {
  fn new(vtable: *const SlSpotlightVtable) -> Option<Self> {
    let create = unsafe { (*vtable).create }?;
    let state = unsafe { create(CTX) };
    Some(Self {
      ctx: CTX,
      vtable,
      state,
    })
  }

  pub fn generate(&self, query: &str) -> Vec<TestSpotlightItem> {
    let Some(generate) = (unsafe { (*self.vtable).generate }) else {
      return Vec::new();
    };
    let mut list = SlSpotlightList::default();
    if unsafe { generate(self.ctx, self.state, SlStr::from_str(query), &mut list) } != 0
      || list.items.is_null()
      || list.count == 0
    {
      return Vec::new();
    }
    let items = unsafe { std::slice::from_raw_parts(list.items, list.count) };
    items
      .iter()
      .map(|item| TestSpotlightItem {
        icon: unsafe { item.icon.as_str() }
          .filter(|icon| !icon.is_empty())
          .map(str::to_owned),
        title: unsafe { item.title.as_str() }
          .unwrap_or_default()
          .to_owned(),
        subtitle: unsafe { item.subtitle.as_str() }
          .filter(|subtitle| !subtitle.is_empty())
          .map(str::to_owned),
        action_label: unsafe { item.action_label.as_str() }
          .filter(|label| !label.is_empty())
          .map(str::to_owned),
        image: unsafe { item.image.as_str() }
          .filter(|image| !image.is_empty())
          .map(str::to_owned),
        tags: if item.tags.is_null() || item.tag_count == 0 {
          Vec::new()
        } else {
          (0..item.tag_count)
            .filter_map(|index| unsafe { (*item.tags.add(index)).as_str() })
            .map(str::to_owned)
            .collect()
        },
      })
      .collect()
  }

  pub fn activate(&self, index: usize) {
    if let Some(activate) = unsafe { (*self.vtable).activate } {
      unsafe { activate(self.ctx, self.state, index) };
    }
  }
}

impl Drop for SpotlightHandle {
  fn drop(&mut self) {
    if let Some(destroy) = unsafe { (*self.vtable).destroy } {
      unsafe { destroy(self.ctx, self.state) };
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TestSpotlightItem {
  pub icon: Option<String>,
  pub title: String,
  pub subtitle: Option<String>,
  pub action_label: Option<String>,
  pub image: Option<String>,
  pub tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TestNode {
  pub kind: u32,
  pub text: Option<String>,
  pub size: f32,
  pub value: f32,
  pub action: Option<String>,
  pub effect: Option<[usize; 4]>,
  pub width: (u32, f32),
  pub height: (u32, f32),
  pub has_background: bool,
  pub has_border: bool,
  pub has_text_color: bool,
  pub children: Vec<TestNode>,
}

impl TestNode {
  fn from_list(list: &SlNodeList) -> TestNode {
    if list.nodes.is_null() || list.len == 0 {
      return TestNode::empty(sl_node_kind::ROW);
    }
    unsafe { TestNode::from_node(&*list.nodes) }
  }

  fn empty(kind: u32) -> Self {
    Self {
      kind,
      text: None,
      size: 0.0,
      value: 0.0,
      action: None,
      effect: None,
      width: (0, 0.0),
      height: (0, 0.0),
      has_background: false,
      has_border: false,
      has_text_color: false,
      children: Vec::new(),
    }
  }

  unsafe fn from_node(node: &SlNode) -> TestNode {
    let text = unsafe { node.text.as_str() }.map(str::to_owned);
    let action = unsafe { node.action.as_str() }
      .filter(|action| !action.is_empty())
      .map(str::to_owned);
    let effect = node.has_effect.then_some(node.effect);
    let children = if node.children.is_null() {
      Vec::new()
    } else {
      (0..node.child_count)
        .map(|index| unsafe { TestNode::from_node(&*node.children.add(index)) })
        .collect()
    };

    TestNode {
      kind: node.kind,
      text,
      size: node.size,
      value: node.value,
      action,
      effect,
      width: (node.width.unit, node.width.value),
      height: (node.height.unit, node.height.value),
      has_background: node.style.has_background,
      has_border: node.style.has_border,
      has_text_color: node.style.has_text_color,
      children,
    }
  }

  pub fn find_text(&self) -> Option<String> {
    if self.kind == sl_node_kind::TEXT
      && let Some(text) = &self.text
    {
      return Some(text.clone());
    }
    self.children.iter().find_map(TestNode::find_text)
  }

  pub fn count_kind(&self, kind: u32) -> usize {
    usize::from(self.kind == kind)
      + self
        .children
        .iter()
        .map(|c| c.count_kind(kind))
        .sum::<usize>()
  }

  pub fn walk(&self) -> Vec<&TestNode> {
    let mut nodes = vec![self];
    for child in &self.children {
      nodes.extend(child.walk());
    }
    nodes
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  struct Counter {
    ticks: u32,
  }

  impl Component for Counter {
    fn new(_ctx: &Context) -> Self {
      Counter { ticks: 0 }
    }

    fn events(&self) -> EventMask {
      EventMask::TICK
    }

    fn watch(&mut self, ctx: &Context) {
      ctx.set_interval(1000, true, 7);
    }

    fn update(&mut self, _ctx: &Context, event: &Event) -> ItemEffect {
      if matches!(event, Event::Tick { tag: 7, .. }) {
        self.ticks += 1;
      }
      ItemEffect::None
    }

    fn view(&self, ctx: &Context) -> Node {
      let label = ctx.options().str("label").unwrap_or_default();
      Node::text(format!("{label} {}", self.ticks))
    }
  }

  #[test]
  fn drives_a_component() {
    let harness = Harness::new();
    harness.option("label", "hi");
    harness.register(|reg| reg.component::<Counter>("test/counter"));

    let component = harness.component("test/counter").expect("registered");
    assert_eq!(component.events(), EventMask::TICK);

    component.watch();
    assert_eq!(harness.timers(), vec![(1000, true, 7)]);

    component.update(&Event::Tick { tag: 7, fd: -1 });
    assert_eq!(component.view().find_text().as_deref(), Some("hi 1"));
    assert_eq!(harness.redraws(), 0);
  }

  struct Menu {
    clicks: u32,
  }

  impl Renderable for Menu {
    fn new(_ctx: &Context) -> Self {
      Menu { clicks: 0 }
    }

    fn handle_message(&mut self, _ctx: &Context, message: &Message) -> ItemEffect {
      if matches!(message, Message::Effect(ItemEffect::Custom([40, 1, 0, 0]))) {
        self.clicks += 1;
        ItemEffect::Redraw
      } else {
        ItemEffect::None
      }
    }

    fn view(&self, _ctx: &Context) -> Node {
      Node::clickable(Node::text(format!("clicks {}", self.clicks))).effect([40, 1, 0, 0])
    }
  }

  #[test]
  fn round_trips_custom_effects() {
    let harness = Harness::new();
    harness.register(|reg| reg.renderable::<Menu>("test/menu"));

    let menu = harness.renderable("test/menu").expect("registered");
    let node = menu.view();
    assert_eq!(node.effect, Some([40, 1, 0, 0]));
    assert_eq!(node.kind, sl_node_kind::CONTAINER);

    let effect = menu.handle_message(&Message::Effect(ItemEffect::Custom([40, 1, 0, 0])));
    assert_eq!(effect, ItemEffect::Redraw);
  }

  struct Widget;

  impl DesktopItem for Widget {
    fn new(_ctx: &Context) -> Self {
      Widget
    }

    fn settings(&self) -> DesktopSettings {
      DesktopSettings {
        visibility: Visibility::Transient,
        update_when: UpdateWhen::EveryFrame,
        ..Default::default()
      }
    }

    fn events(&self) -> EventMask {
      EventMask::CONFIG_RELOAD
    }

    fn view(&self, ctx: &Context) -> Node {
      Node::text(ctx.options().str("monitor").unwrap_or_default())
    }
  }

  #[test]
  fn drives_a_desktop_item() {
    let harness = Harness::new();
    harness.monitor("DP-1");
    harness.register(|reg| reg.desktop_item::<Widget>("test/widget"));

    let item = harness.desktop_item("test/widget").expect("registered");
    assert_eq!(item.settings().visibility, sl_visibility::TRANSIENT);
    assert_eq!(item.settings().update_when, sl_update_when::EVERY_FRAME);
    assert_eq!(item.events(), EventMask::CONFIG_RELOAD);
    assert_eq!(item.view().find_text().as_deref(), Some("DP-1"));
  }

  struct Reset;

  impl Payload for Reset {
    fn new(_ctx: &Context) -> Self {
      Reset
    }

    fn invoke(&mut self, ctx: &Context, args: &Args) {
      ctx.error(1, "boom");
      ctx.notify(&Notification::new("done", "body").action("Again", "test.reset"));
      if args.contains("x") {
        ctx.log(2, "saw x");
      }
    }
  }

  #[test]
  fn records_payload_side_effects() {
    let harness = Harness::new();
    harness.register(|reg| reg.payload::<Reset>("test.reset"));

    let payload = harness.payload("test.reset").expect("registered");
    payload.invoke(&[("x", "1")]);

    assert_eq!(harness.errors(), vec![(1, "boom".to_owned())]);
    assert_eq!(harness.logs(), vec![(2, "saw x".to_owned())]);
    let notifications = harness.notifications();
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].summary, "done");
    assert_eq!(notifications[0].actions[0].command, "test.reset");
  }

  fn parse_label(node: Option<&kdl::KdlNode>) -> String {
    node
      .and_then(|node| node.get(0))
      .and_then(kdl::KdlValue::as_string)
      .unwrap_or_default()
      .to_owned()
  }

  #[test]
  fn reruns_config_parsers() {
    let harness = Harness::new();
    harness.register(|reg| reg.config_parser::<String>("test_entry", parse_label));

    let results = harness.reload("test_entry \"hello\"");
    assert_eq!(results, vec![("test_entry".to_owned(), 0)]);

    let ctx = Context::new(&MOCK_API, CTX);
    assert_eq!(ctx.config::<String>("test_entry"), Some("hello".to_owned()));
  }

  #[test]
  fn serves_styles_theme_and_compositor_state() {
    let harness = Harness::new();
    let mut sheet = StyleSheet::default();
    sheet.insert("background", "#112233");
    sheet.insert("radius", 4);
    harness.style("test/card", sheet);

    let mut monitors = CompositorState::default();
    monitors.monitors.push(Monitor {
      name: "DP-1".into(),
      width: 1920,
      height: 1080,
      scale: 1.0,
    });
    harness.set_compositor(monitors);

    let ctx = Context::new(&MOCK_API, CTX);
    let card = ctx.style("test/card");
    assert_eq!(card.number("radius"), Some(4.0));
    let state = ctx.compositor_state().expect("compositor state");
    assert_eq!(state.monitors[0].name, "DP-1");
    assert_eq!(state.monitors[0].width, 1920);
  }

  #[test]
  fn serves_service_snapshots_and_dispatch() {
    let harness = Harness::new();

    let audio = AudioState {
      volume: 0.5,
      default_sink: Some("speakers".into()),
      sinks: vec![AudioSink {
        id: 1,
        name: "sink0".into(),
        description: "Speakers".into(),
        volume: 0.5,
        muted: false,
        is_default: true,
      }],
      player: Some(MprisPlayer {
        identity: "mpv".into(),
        title: "Song".into(),
        artist: "Artist".into(),
        album: "Album".into(),
        art_url: Some("https://example.test/art".into()),
        playback_status: "Playing".into(),
        can_play_pause: true,
        can_go_next: true,
        can_go_previous: false,
      }),
      ..Default::default()
    };
    harness.audio(audio);

    let bluetooth = BluetoothState {
      powered: true,
      adapter_name: Some("hci0".into()),
      devices: vec![BluetoothDevice {
        address: "AA:BB".into(),
        name: "Headset".into(),
        icon: Some("audio-headset".into()),
        paired: true,
        connected: true,
        trusted: true,
        battery: Some(80),
        rssi: Some(-40),
      }],
      ..Default::default()
    };
    harness.bluetooth(bluetooth);

    let network = NetworkState {
      wifi_enabled: true,
      networks: vec![AccessPoint {
        ssid: "home".into(),
        signal: 80,
        secured: true,
        in_use: true,
        saved: true,
      }],
      connected: Some(ConnectionInfo {
        label: "home".into(),
        mac: "AA:BB:CC".into(),
        is_wifi: true,
      }),
      ..Default::default()
    };
    harness.network(network);

    let system = SystemState {
      cpu_usage: 12.5,
      temperature: Some(40.0),
      processes: vec![ProcessInfo {
        pid: 42,
        name: "firefox".into(),
        cpu_usage: 3.0,
        memory: 1024,
      }],
      ..Default::default()
    };
    harness.system(system);

    let tray = TrayState {
      items: vec![TrayItem {
        address: "org.test".into(),
        title: "Test".into(),
        icon_name: Some("test-symbolic".into()),
        menu_path: Some("/Menu".into()),
        pixmap: Some((1, 1, vec![255, 0, 0, 255])),
      }],
    };
    harness.tray(tray);

    let power = PowerState {
      percent: Some(63),
      charging: true,
      status: "Charging".into(),
      health: Some(95),
      energy_now_wh: Some(30.0),
      energy_full_wh: Some(50.0),
      power_w: Some(12.5),
      time_remaining: Some("1:20".into()),
      active_profile: Some("balanced".into()),
      available_profiles: vec!["power-saver".into(), "balanced".into()],
      brightness_percent: 70,
      brightness_max: 1000,
      brightness_current: 700,
      device_name: "BAT0".into(),
    };
    harness.power(power);

    let ctx = Context::new(&MOCK_API, CTX);

    let audio = ctx.audio_state().expect("audio state");
    assert_eq!(audio.volume, 0.5);
    assert_eq!(audio.default_sink.as_deref(), Some("speakers"));
    assert_eq!(audio.sinks[0].description, "Speakers");
    assert_eq!(audio.player.as_ref().unwrap().title, "Song");

    let bluetooth = ctx.bluetooth_state().expect("bluetooth state");
    assert!(bluetooth.powered);
    assert_eq!(bluetooth.devices[0].battery, Some(80));
    assert_eq!(bluetooth.devices[0].rssi, Some(-40));

    let network = ctx.network_state().expect("network state");
    assert_eq!(network.networks[0].ssid, "home");
    assert_eq!(network.connected.as_ref().unwrap().label, "home");

    let system = ctx.system_state().expect("system state");
    assert_eq!(system.temperature, Some(40.0));
    assert_eq!(system.processes[0].name, "firefox");

    let tray = ctx.tray_state().expect("tray state");
    assert_eq!(tray.items[0].title, "Test");
    assert_eq!(tray.items[0].pixmap, Some((1, 1, vec![255, 0, 0, 255])));

    let power = ctx.power_state().expect("power state");
    assert_eq!(power.percent, Some(63));
    assert!(power.charging);
    assert_eq!(power.active_profile.as_deref(), Some("balanced"));
    assert_eq!(
      power.available_profiles,
      vec!["power-saver".to_owned(), "balanced".to_owned()]
    );
    assert_eq!(power.brightness_current, 700);
    assert_eq!(power.device_name, "BAT0");

    assert!(ctx.dispatch("audio.toggle_mute"));
    assert_eq!(harness.dispatched(), vec!["audio.toggle_mute".to_owned()]);
  }

  struct Comp;

  impl Compositor for Comp {
    fn new(_ctx: &Context) -> Self {
      Comp
    }

    fn initialize(&mut self, ctx: &Context) {
      assert_eq!(ctx.register_compositor_fd(9, 0), 0);
      ctx.set_compositor_state(&CompositorState::default());
    }

    fn update_state(&mut self, _ctx: &Context) {}
  }

  #[test]
  fn compositor_registers_fds_and_pushes_state() {
    let harness = Harness::new();
    harness.register(|reg| reg.compositor::<Comp>("test/comp"));

    let comp = harness.compositor("test/comp").expect("registered");
    comp.initialize();
    comp.update_state();

    assert_eq!(harness.compositor_fds(), vec![(9, 0)]);
    assert_eq!(harness.compositor_pushes(), 1);
  }

  struct Provider;

  impl Spotlight for Provider {
    fn new(_ctx: &Context) -> Self {
      Provider
    }

    fn generate(&mut self, _ctx: &Context, query: &str) -> Vec<SpotlightResult> {
      if query.is_empty() {
        return Vec::new();
      }
      vec![SpotlightResult {
        icon: Some("edit-find-symbolic".into()),
        title: format!("{query}!"),
        subtitle: Some("sub".into()),
        action_label: Some("Open".into()),
        image: Some("/tmp/thumb.png".into()),
        tags: vec!["T".into()],
      }]
    }
  }

  #[test]
  fn serves_spotlight_modes() {
    let harness = Harness::new();
    harness.register(|reg| reg.spotlight::<Provider>("test/mode"));

    let mode = harness.spotlight("test/mode").expect("registered");
    assert!(mode.generate("").is_empty());

    let results = mode.generate("hi");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "hi!");
    assert_eq!(results[0].subtitle.as_deref(), Some("sub"));
    assert_eq!(results[0].action_label.as_deref(), Some("Open"));
    assert_eq!(results[0].image.as_deref(), Some("/tmp/thumb.png"));
    assert_eq!(results[0].tags, vec!["T".to_owned()]);

    mode.activate(0);
  }

  #[test]
  fn registries_round_trip_and_notify() {
    let ctx = Context::new(&MOCK_API, CTX);
    let key = 0x5245_4749_5354_5259;

    assert!(ctx.register_registry::<u32>(key, 7));
    assert!(!ctx.register_registry::<u32>(key, 8));
    assert_eq!(ctx.with_registry::<u32, _>(key, |value| *value), Some(7));

    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<Option<u32>>::new()));
    let sink = seen.clone();
    ctx.subscribe_registry(key, move |_ctx, value| {
      let value = if value.is_null() {
        None
      } else {
        Some(unsafe { *(value as *const u32) })
      };
      sink.lock().unwrap().push(value);
    });
    assert_eq!(*seen.lock().unwrap(), vec![Some(7)]);

    assert_eq!(ctx.remove_registry::<u32>(key), Some(7));
    assert_eq!(*seen.lock().unwrap(), vec![Some(7), None]);
    assert!(ctx.with_registry::<u32, _>(key, |value| *value).is_none());

    let other = Context::new(&MOCK_API, CTX);
    assert!(other.register_registry::<u32>(key, 9));
    assert_eq!(*seen.lock().unwrap(), vec![Some(7), None, Some(9)]);
  }

  #[test]
  fn persistence_round_trips_typed_values() {
    let ctx = Context::new(&MOCK_API, CTX);

    assert_eq!(ctx.persistence_get::<u32>("count"), None);
    assert!(!ctx.persistence_has("count"));

    assert_eq!(ctx.persistence_open_or_get("count", 3u32), 3);
    assert_eq!(ctx.persistence_get::<u32>("count"), Some(3));
    assert!(ctx.persistence_has("count"));

    assert!(ctx.persistence_set("name", &"dock".to_string()));
    assert_eq!(
      ctx.persistence_get::<String>("name").as_deref(),
      Some("dock")
    );

    assert!(ctx.persistence_remove("name"));
    assert_eq!(ctx.persistence_get::<String>("name"), None);
    assert!(!ctx.persistence_has("name"));
  }
}
