use std::{collections::HashMap, sync::atomic::Ordering};

use slowshell_commons::{
  audio::SharedAudioState,
  bluetooth::SharedBluetoothState,
  network::{AccessPoint as NetworkAccessPoint, SharedNetworkState},
  power::SharedPowerState,
  system::SharedSystemState,
  tray::SharedTrayState,
};
use slowshell_core::{
  Monitor as CoreMonitor, Window as CoreWindow, Workspace as CoreWorkspace, types::Ustr,
};
use slowshell_plugin_abi::{
  AccessPoint, AudioSink, AudioState, BluetoothDevice, BluetoothState, Color, CompositorState,
  ConnectionInfo, Ethernet, Monitor, MprisPlayer, NetworkState, PowerState, ProcessInfo, RString,
  RVec, SystemState, Theme, TrayItem, TrayPixmap, TrayState, Window, Workspace,
};

use crate::state::{current_compositor, current_config, current_store};

pub(crate) fn audio_state() -> Option<AudioState> {
  let store = current_store()?;
  let shared = store.borrow::<SharedAudioState>()?;

  let inner = shared.state.lock().unwrap();
  let player = shared.player.lock().unwrap().clone();

  let sinks: RVec<AudioSink> = inner
    .sinks
    .iter()
    .map(|sink| AudioSink {
      id: sink.id,
      name: sink.name.clone().into(),
      description: sink.description.clone().into(),
      volume: sink.volume,
      muted: sink.muted,
      is_default: sink.is_default,
    })
    .collect();

  let player = player.as_ref().map(|player| MprisPlayer {
    identity: player.identity.clone().into(),
    title: player.title.clone().into(),
    artist: player.artist.clone().into(),
    album: player.album.clone().into(),
    art_url: player.art_url.clone().map(RString::from).into(),
    playback_status: player.playback_status.clone().into(),
    can_play_pause: player.can_play_pause,
    can_go_next: player.can_go_next,
    can_go_previous: player.can_go_previous,
  });

  Some(AudioState {
    volume: inner.volume,
    muted: inner.muted,
    default_sink: inner.default_sink_name.clone().map(RString::from).into(),
    sinks,
    player: player.into(),
  })
}

pub(crate) fn bluetooth_state() -> Option<BluetoothState> {
  let store = current_store()?;
  let shared = store.borrow::<SharedBluetoothState>()?;

  let inner = shared.state.lock().unwrap();

  let devices: RVec<BluetoothDevice> = inner
    .devices
    .iter()
    .map(|device| BluetoothDevice {
      address: device.address.clone().into(),
      name: device.name.clone().into(),
      icon: device.icon.clone().map(RString::from).into(),
      paired: device.paired,
      connected: device.connected,
      trusted: device.trusted,
      battery: device.battery.into(),
      rssi: device.rssi.into(),
    })
    .collect();

  Some(BluetoothState {
    powered: shared.powered.load(Ordering::Relaxed),
    discovering: shared.discovering.load(Ordering::Relaxed),
    adapter_name: inner.adapter_name.clone().map(RString::from).into(),
    devices,
  })
}

pub(crate) fn network_state() -> Option<NetworkState> {
  let store = current_store()?;
  let shared = store.borrow::<SharedNetworkState>()?;

  let inner = shared.state.lock().unwrap();

  let access_point = |ap: &NetworkAccessPoint| AccessPoint {
    ssid: ap.ssid.clone().into(),
    signal: ap.signal,
    secured: ap.secured,
    in_use: ap.in_use,
    saved: ap.saved,
  };

  let wifi_connected = inner.wifi_connected.as_ref().map(access_point);
  let ethernet = inner.ethernet.as_ref().map(|eth| Ethernet {
    iface: eth.iface.clone().into(),
    speed: eth.speed,
    carrier: eth.carrier,
    connected: eth.connected,
  });
  let networks: RVec<AccessPoint> = inner.networks.iter().map(access_point).collect();
  let connected = inner.connected.as_ref().map(|info| ConnectionInfo {
    label: info.label.clone().into(),
    mac: info.mac.clone().into(),
    is_wifi: info.is_wifi,
  });

  Some(NetworkState {
    wifi_enabled: shared.wifi_enabled.load(Ordering::Relaxed),
    wifi_hardware_enabled: shared.wifi_hardware_enabled.load(Ordering::Relaxed),
    scanning: shared.scanning.load(Ordering::Relaxed),
    busy: shared.busy.load(Ordering::Relaxed),
    wifi_connected: wifi_connected.into(),
    ethernet: ethernet.into(),
    networks,
    connected: connected.into(),
  })
}

pub(crate) fn system_state() -> Option<SystemState> {
  let store = current_store()?;
  let shared = store.borrow::<SharedSystemState>()?;

  let snapshot = shared.snapshot.lock().unwrap();

  let processes: RVec<ProcessInfo> = snapshot
    .top_processes
    .iter()
    .map(|process| ProcessInfo {
      pid: process.pid,
      name: process.name.clone().into(),
      cpu_usage: process.cpu_usage,
      memory: process.memory,
    })
    .collect();

  Some(SystemState {
    cpu_usage: snapshot.cpu_usage,
    mem_usage: snapshot.mem_usage,
    mem_total: snapshot.mem_total,
    mem_used: snapshot.mem_used,
    temperature: snapshot.temperature.into(),
    load: snapshot.load,
    network_state: snapshot.network_state,
    network_rx: snapshot.network_rx,
    network_tx: snapshot.network_tx,
    processes,
  })
}

pub(crate) fn tray_state() -> Option<TrayState> {
  let store = current_store()?;
  let shared = store.borrow::<SharedTrayState>()?;

  let inner = shared.state.lock().unwrap();

  let items: RVec<TrayItem> = inner
    .items
    .iter()
    .map(|item| TrayItem {
      address: item.address.clone().into(),
      title: item.title.clone().into(),
      icon_name: item.icon_name.clone().map(RString::from).into(),
      menu_path: item.menu_path.clone().map(RString::from).into(),
      pixmap: item
        .pixmap
        .as_ref()
        .map(|(width, height, pixels)| TrayPixmap {
          width: *width,
          height: *height,
          data: pixels.clone().into(),
        })
        .into(),
    })
    .collect();

  Some(TrayState { items })
}

pub(crate) fn power_state() -> Option<PowerState> {
  let store = current_store()?;
  let shared = store.borrow::<SharedPowerState>()?;

  let data = shared.data.lock().unwrap();

  let available_profiles: RVec<RString> = data
    .available_profiles
    .iter()
    .map(|profile| RString::from(profile.as_str()))
    .collect();

  Some(PowerState {
    percent: data.percent.into(),
    charging: data.charging,
    status: data.status.clone().into(),
    health: data.health.into(),
    energy_now_wh: data.energy_now_wh.into(),
    energy_full_wh: data.energy_full_wh.into(),
    power_w: data.power_w.into(),
    time_remaining: data.time_remaining.clone().map(RString::from).into(),
    active_profile: data
      .active_profile
      .as_ref()
      .map(|profile| RString::from(profile.as_str()))
      .into(),
    available_profiles,
    brightness_percent: data.brightness_percent,
    brightness_max: data.brightness_max,
    brightness_current: data.brightness_current,
    device_name: data.device_name.clone().into(),
  })
}

pub(crate) fn compositor_state() -> Option<CompositorState> {
  let state = current_compositor()?;

  let monitors: RVec<Monitor> = state
    .monitors
    .values()
    .map(|monitor| Monitor {
      name: RString::from(&*monitor.name),
      width: monitor.width,
      height: monitor.height,
      scale: monitor.scale as f32,
    })
    .collect();

  let workspaces: RVec<Workspace> = state
    .workspaces
    .iter()
    .map(|workspace| Workspace {
      id: workspace.id,
      idx: workspace.idx,
      name: workspace.name.clone().map(RString::from).into(),
      output: workspace.output.clone().map(RString::from).into(),
      is_active: workspace.is_active,
      is_focused: workspace.is_focused,
      is_urgent: workspace.is_urgent,
    })
    .collect();

  let active_window = state.active_window.as_ref().map(|window| Window {
    id: window.id,
    title: window.title.clone().into(),
    class: window.class.clone().into(),
  });

  Some(CompositorState {
    monitors,
    workspaces,
    active_window: active_window.into(),
    overview_active: state.overview_active,
  })
}

pub(crate) fn convert_compositor_state(
  state: &CompositorState,
) -> slowshell_compositor::CompositorState {
  let mut monitors = HashMap::new();
  for monitor in &state.monitors {
    let name = Ustr::from(monitor.name.as_str());
    monitors.insert(
      name.clone(),
      CoreMonitor {
        name,
        active_workspace: 0,
        width: monitor.width,
        height: monitor.height,
        scale: monitor.scale as f64,
      },
    );
  }

  let workspaces = state
    .workspaces
    .iter()
    .map(|workspace| CoreWorkspace {
      id: workspace.id,
      idx: workspace.idx,
      name: workspace
        .name
        .as_ref()
        .into_option()
        .map(|name| name.as_str().to_owned()),
      output: workspace
        .output
        .as_ref()
        .into_option()
        .map(|output| output.as_str().to_owned()),
      is_active: workspace.is_active,
      is_focused: workspace.is_focused,
      is_urgent: workspace.is_urgent,
    })
    .collect();

  let active_window = state
    .active_window
    .as_ref()
    .into_option()
    .map(|window| CoreWindow {
      id: window.id,
      title: window.title.as_str().to_owned(),
      class: window.class.as_str().to_owned(),
      is_active: true,
      metadata: HashMap::new(),
    });

  slowshell_compositor::CompositorState {
    monitors,
    active_window,
    active_windows: Vec::new(),
    workspaces,
    overview_active: state.overview_active,
  }
}

pub(crate) fn theme() -> Theme {
  let Some(config) = current_config() else {
    return Theme::default();
  };

  theme_from_palette(config.palette())
}

fn theme_from_palette(palette: &slowshell_config::style::Theme) -> Theme {
  let color = |color: iced::Color| Color {
    r: color.r,
    g: color.g,
    b: color.b,
    a: color.a,
  };

  Theme {
    base: color(palette.base),
    crust: color(palette.crust),
    mantle: color(palette.mantle),
    primary: color(palette.primary),
    secondary: color(palette.secondary),
    green: color(palette.green),
    red: color(palette.red),
    blue: color(palette.blue),
    yellow: color(palette.yellow),
    orange: color(palette.orange),
    text: color(palette.text),
    subtext: color(palette.subtext),
    overlay: color(palette.overlay),
  }
}

pub(crate) fn style_number(classes: &str, property: &str) -> Option<f32> {
  let config = current_config()?;

  let class_list: Vec<&str> = classes.split_whitespace().collect();
  config.computed(&class_list).length(property)
}

pub(crate) fn style_bool(classes: &str, property: &str) -> Option<bool> {
  let config = current_config()?;

  let class_list: Vec<&str> = classes.split_whitespace().collect();
  let computed = config.computed(&class_list);
  computed
    .keyword(property)
    .map(|value| value == "true")
    .or_else(|| computed.number(property).map(|value| value != 0.0))
}

pub(crate) fn style_color(classes: &str, property: &str) -> Option<Color> {
  let config = current_config()?;

  let class_list: Vec<&str> = classes.split_whitespace().collect();
  let color = config.computed(&class_list).color(property)?.to_iced();

  Some(Color {
    r: color.r,
    g: color.g,
    b: color.b,
    a: color.a,
  })
}
