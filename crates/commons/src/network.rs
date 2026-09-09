use std::collections::HashSet;
use std::sync::{Arc, Mutex, atomic::AtomicBool, atomic::AtomicU64};

pub const NET_CHANGED: &str = "component/network.changed";

#[derive(Debug, Clone, Default)]
pub struct AccessPoint {
  pub ssid: String,
  pub signal: u8,
  pub secured: bool,
  pub in_use: bool,
  pub saved: bool,
}

#[derive(Default, Clone)]
pub struct NetworkPayload {
  pub ssid: String,
  pub password: Option<String>,
  pub value: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EthernetState {
  pub iface: String,
  pub speed: u32,
  pub carrier: bool,
  pub connected: bool,
}

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
  pub label: String,
  pub is_wifi: bool,
  pub mac: String,
  pub ip4: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct NetworkInner {
  pub wifi_connected: Option<AccessPoint>,
  pub ethernet: Option<EthernetState>,
  pub networks: Vec<AccessPoint>,
  pub connected: Option<ConnectionInfo>,
}

#[derive(Debug, Clone)]
pub enum NetCmd {
  Connect {
    ssid: String,
    password: Option<String>,
  },
  DisconnectWifi,
  WiredConnect,
  ToggleWifi,
  Scan,
}

#[derive(Debug, Default)]
pub struct MenuUi {
  pub selected_ssid: Option<String>,
  pub password: String,
  pub info_ssid: Option<String>,
}

pub struct NetworkState {
  pub wifi_enabled: AtomicBool,
  pub wifi_hardware_enabled: AtomicBool,
  pub scanning: AtomicBool,
  pub busy: AtomicBool,
  pub state: Mutex<NetworkInner>,
  pub menu: Mutex<MenuUi>,
  pub saved_ssids: Mutex<Option<HashSet<String>>>,
  pub revision: AtomicU64,
  pub cmd_tx: tokio::sync::mpsc::UnboundedSender<NetCmd>,
}

pub type SharedNetworkState = Arc<NetworkState>;

pub fn signal_icon(strength: u8) -> &'static str {
  match strength {
    80..=100 => "network-wireless-signal-excellent-symbolic",
    60..=79 => "network-wireless-signal-good-symbolic",
    40..=59 => "network-wireless-signal-ok-symbolic",
    20..=39 => "network-wireless-signal-weak-symbolic",
    _ => "network-wireless-signal-none-symbolic",
  }
}
