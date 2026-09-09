use std::sync::{Arc, Mutex, atomic::AtomicBool, atomic::AtomicU64};

pub const BT_CHANGED: &str = "component/bluetooth.changed";

#[derive(Debug, Clone, Default)]
pub struct BluetoothDevice {
  pub address: String,
  pub name: String,
  pub icon: Option<String>,
  pub paired: bool,
  pub connected: bool,
  pub trusted: bool,
  pub battery: Option<u8>,
  pub rssi: Option<i16>,
}

#[derive(Debug, Clone, Default)]
pub struct BluetoothInner {
  pub adapter_name: Option<String>,
  pub devices: Vec<BluetoothDevice>,
}

#[derive(Debug, Clone)]
pub enum BluetoothCmd {
  TogglePower,
  SetPower(bool),
  StartDiscovery,
  StopDiscovery,
  Connect(String),
  Disconnect(String),
  Pair(String),
  Remove(String),
  Trust(String, bool),
}

#[derive(Debug, Default)]
pub struct BluetoothMenuUi {
  pub selected_device: Option<String>,
  pub confirming_remove: Option<String>,
}

pub struct BluetoothState {
  pub powered: AtomicBool,
  pub discovering: AtomicBool,
  pub state: Mutex<BluetoothInner>,
  pub menu: Mutex<BluetoothMenuUi>,
  pub revision: AtomicU64,
  pub cmd_tx: tokio::sync::mpsc::UnboundedSender<BluetoothCmd>,
}

pub type SharedBluetoothState = Arc<BluetoothState>;

pub fn bluetooth_icon(powered: bool, connected_count: usize) -> &'static str {
  if !powered {
    "bluetooth-disabled-symbolic"
  } else if connected_count > 0 {
    "bluetooth-active-symbolic"
  } else {
    "bluetooth-symbolic"
  }
}
