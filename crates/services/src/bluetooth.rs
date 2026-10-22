use std::{
  collections::HashMap,
  os::fd::OwnedFd,
  sync::{Arc, atomic::Ordering},
  time::Duration,
};

use futures_util::StreamExt;
use slowshell_commons::bluetooth::{BluetoothCmd, BluetoothDevice, SharedBluetoothState};
use zbus::zvariant::{ObjectPath, Value};

use crate::lifecycle::ServiceSlot;

pub(crate) struct GlobalBluetooth {
  shared: SharedBluetoothState,
  slot: ServiceSlot<BluetoothCmd>,
}

static GLOBAL: std::sync::OnceLock<GlobalBluetooth> = std::sync::OnceLock::new();

fn global() -> &'static GlobalBluetooth {
  use slowshell_commons::bluetooth::BluetoothState;
  use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64},
  };

  GLOBAL.get_or_init(|| {
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();

    GlobalBluetooth {
      shared: Arc::new(BluetoothState {
        powered: AtomicBool::new(false),
        discovering: AtomicBool::new(false),
        state: Mutex::new(Default::default()),
        menu: Mutex::new(Default::default()),
        revision: AtomicU64::new(0),
        cmd_tx,
      }),
      slot: ServiceSlot::new(cmd_rx),
    }
  })
}

pub fn shared() -> SharedBluetoothState {
  global().shared.clone()
}

pub fn ensure(store: &mut slowshell_core::Store) -> SharedBluetoothState {
  let global = global();
  store.insert(global.shared.clone());

  if let Some(cmd_rx) = global.slot.take() {
    let notify = crate::lifecycle::watch_notify(store, slowshell_commons::bluetooth::BT_CHANGED);
    run(global.shared.clone(), cmd_rx, notify);
  }

  global.shared.clone()
}

pub fn running() -> bool {
  global().slot.running()
}

const BLUEZ_DEST: &str = "org.bluez";
const ADAPTER_IFACE: &str = "org.bluez.Adapter1";
const DEVICE_IFACE: &str = "org.bluez.Device1";
const PROPS_IFACE: &str = "org.freedesktop.DBus.Properties";
const OBJ_MGR_IFACE: &str = "org.freedesktop.DBus.ObjectManager";

pub fn run(
  shared: SharedBluetoothState,
  mut cmd_rx: tokio::sync::mpsc::UnboundedReceiver<BluetoothCmd>,
  notify: Option<OwnedFd>,
) {
  crate::shared_runtime().spawn(async move {
    loop {
      match bt_loop(&shared, &mut cmd_rx, &notify).await {
        Ok(()) => break,
        Err(e) => {
          eprintln!("[bluetooth] dbus loop error: {e}");
          tokio::time::sleep(Duration::from_millis(2000)).await;
        }
      }
    }
  });
}

async fn bt_loop(
  shared: &SharedBluetoothState,
  cmd_rx: &mut tokio::sync::mpsc::UnboundedReceiver<BluetoothCmd>,
  notify: &Option<OwnedFd>,
) -> miette::Result<()> {
  use miette::IntoDiagnostic;

  let conn = zbus::connection::Builder::system()
    .into_diagnostic()?
    .build()
    .await
    .into_diagnostic()?;

  let mut adapter_path: Option<String> = None;

  refresh_all(&conn, shared, &mut adapter_path).await?;
  bump(shared, notify);

  let root = zbus::Proxy::new_owned(conn.clone(), BLUEZ_DEST, "/", OBJ_MGR_IFACE)
    .await
    .into_diagnostic()?;
  let mut added_stream = root
    .receive_signal("InterfacesAdded")
    .await
    .into_diagnostic()?;
  let mut removed_stream = root
    .receive_signal("InterfacesRemoved")
    .await
    .into_diagnostic()?;

  let props_rule = zbus::MatchRule::builder()
    .msg_type(zbus::message::Type::Signal)
    .interface(PROPS_IFACE)
    .into_diagnostic()?
    .member("PropertiesChanged")
    .into_diagnostic()?
    .path_namespace("/org/bluez")
    .into_diagnostic()?
    .build();

  let mut props_stream = zbus::MessageStream::for_match_rule(props_rule, &conn, Some(64))
    .await
    .into_diagnostic()?;

  let mut ticker = tokio::time::interval(Duration::from_secs(5));

  loop {
    tokio::select! {
      Some(cmd) = cmd_rx.recv() => {
        use std::os::fd::AsRawFd;
        let conn = conn.clone();
        let shared = shared.clone();
        let adapter_path = adapter_path.clone();
        let raw_notify = notify.as_ref().map(|f| f.as_raw_fd());
        crate::shared_runtime().spawn(async move {
          handle_cmd(&conn, &shared, &adapter_path, cmd).await;
          let mut ap = adapter_path.clone();
          refresh_all(&conn, &shared, &mut ap).await.ok();
          bump_raw(&shared, raw_notify);
        });
      }

      Some(_) = added_stream.next() => {
        refresh_all(&conn, shared, &mut adapter_path).await.ok();
        bump(shared, notify);
      }

      Some(_) = removed_stream.next() => {
        refresh_all(&conn, shared, &mut adapter_path).await.ok();
        bump(shared, notify);
      }

      Some(_) = props_stream.next() => {
        refresh_all(&conn, shared, &mut adapter_path).await.ok();
        bump(shared, notify);
      }

      _ = ticker.tick() => {
        refresh_all(&conn, shared, &mut adapter_path).await.ok();
        bump(shared, notify);
      }
    }
  }
}

async fn refresh_all(
  conn: &zbus::Connection,
  shared: &SharedBluetoothState,
  adapter_path: &mut Option<String>,
) -> miette::Result<()> {
  use miette::IntoDiagnostic;

  let reply = conn
    .call_method(
      Some(BLUEZ_DEST),
      "/",
      Some(OBJ_MGR_IFACE),
      "GetManagedObjects",
      &(),
    )
    .await
    .into_diagnostic()?;

  let body = reply.body();
  let objects: HashMap<ObjectPath<'_>, HashMap<String, HashMap<String, Value<'_>>>> =
    body.deserialize().into_diagnostic()?;

  let mut found_adapter = None;
  let mut adapter_alias = None;
  let mut adapter_powered = false;
  let mut adapter_discovering = false;
  let mut adapter_discoverable = false;
  let mut devices = Vec::new();

  for (path, ifaces) in &objects {
    let path_str = path.as_str();

    if let Some(props) = ifaces.get(ADAPTER_IFACE) {
      if found_adapter.is_none() || path_str.ends_with("hci0") {
        found_adapter = Some(path_str.to_string());
        if let Some(Value::Bool(p)) = props.get("Powered") {
          adapter_powered = *p;
        }
        if let Some(Value::Bool(d)) = props.get("Discovering") {
          adapter_discovering = *d;
        }
        if let Some(Value::Bool(disc)) = props.get("Discoverable") {
          adapter_discoverable = *disc;
        }
        if let Some(Value::Str(alias)) = props.get("Alias").or_else(|| props.get("Name")) {
          adapter_alias = Some(alias.to_string());
        }
      }
    }

    if let Some(props) = ifaces.get(DEVICE_IFACE) {
      let address = match props.get("Address") {
        Some(Value::Str(s)) => s.to_string(),
        _ => continue,
      };

      let name = props
        .get("Alias")
        .or_else(|| props.get("Name"))
        .and_then(|v| match v {
          Value::Str(s) => Some(s.to_string()),
          _ => None,
        })
        .unwrap_or_else(|| address.clone());

      let icon = props.get("Icon").and_then(|v| match v {
        Value::Str(s) => Some(s.to_string()),
        _ => None,
      });

      let paired = matches!(props.get("Paired"), Some(Value::Bool(true)));
      let connected = matches!(props.get("Connected"), Some(Value::Bool(true)));
      let trusted = matches!(props.get("Trusted"), Some(Value::Bool(true)));
      let blocked = matches!(props.get("Blocked"), Some(Value::Bool(true)));

      let battery = ifaces
        .get("org.bluez.Battery1")
        .and_then(|b_props| b_props.get("Percentage"))
        .and_then(|v| match v {
          Value::U8(b) => Some(*b),
          Value::U16(b) => Some(*b as u8),
          Value::U32(b) => Some(*b as u8),
          Value::I32(b) => Some(*b as u8),
          _ => None,
        });

      let rssi = props.get("RSSI").and_then(|v| match v {
        Value::I16(r) => Some(*r),
        _ => None,
      });

      devices.push(BluetoothDevice {
        address,
        name,
        icon,
        paired,
        connected,
        trusted,
        blocked,
        battery,
        rssi,
        path: Some(path_str.to_string()),
      });
    }
  }

  devices.sort_by(|a, b| {
    b.connected
      .cmp(&a.connected)
      .then_with(|| b.paired.cmp(&a.paired))
      .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
  });

  if found_adapter.is_some() {
    *adapter_path = found_adapter.clone();
  }

  shared.powered.store(adapter_powered, Ordering::Relaxed);
  shared
    .discovering
    .store(adapter_discovering, Ordering::Relaxed);

  let mut guard = shared.state.lock().unwrap();
  let state = Arc::make_mut(&mut guard);
  state.adapter_name = found_adapter;
  state.adapter_alias = adapter_alias;
  state.discoverable = adapter_discoverable;
  state.devices = devices;

  Ok(())
}

fn resolve_dev_path(shared: &SharedBluetoothState, adapter: &str, addr: &str) -> String {
  if addr.starts_with('/') {
    return addr.to_string();
  }
  if let Ok(guard) = shared.state.lock() {
    for dev in &guard.devices {
      if dev.address.eq_ignore_ascii_case(addr) {
        if let Some(ref path) = dev.path {
          return path.clone();
        }
      }
    }
  }
  let formatted = addr.to_uppercase().replace(':', "_");
  format!("{adapter}/dev_{formatted}")
}

async fn handle_cmd(
  conn: &zbus::Connection,
  shared: &SharedBluetoothState,
  adapter_path: &Option<String>,
  cmd: BluetoothCmd,
) {
  let Some(adapter) = adapter_path.as_deref() else {
    return;
  };

  match cmd {
    BluetoothCmd::TogglePower => {
      let current = shared.powered.load(Ordering::Relaxed);
      let target = Value::from(!current);
      let _ = conn
        .call_method(
          Some(BLUEZ_DEST),
          adapter,
          Some(PROPS_IFACE),
          "Set",
          &(ADAPTER_IFACE, "Powered", target),
        )
        .await;
    }
    BluetoothCmd::SetPower(on) => {
      let target = Value::from(on);
      let _ = conn
        .call_method(
          Some(BLUEZ_DEST),
          adapter,
          Some(PROPS_IFACE),
          "Set",
          &(ADAPTER_IFACE, "Powered", target),
        )
        .await;
    }
    BluetoothCmd::StartDiscovery => {
      let _ = conn
        .call_method(
          Some(BLUEZ_DEST),
          adapter,
          Some(ADAPTER_IFACE),
          "StartDiscovery",
          &(),
        )
        .await;
    }
    BluetoothCmd::StopDiscovery => {
      let _ = conn
        .call_method(
          Some(BLUEZ_DEST),
          adapter,
          Some(ADAPTER_IFACE),
          "StopDiscovery",
          &(),
        )
        .await;
    }
    BluetoothCmd::ToggleDiscoverable => {
      let current = shared.state.lock().map(|s| s.discoverable).unwrap_or(false);
      let target = Value::from(!current);
      let _ = conn
        .call_method(
          Some(BLUEZ_DEST),
          adapter,
          Some(PROPS_IFACE),
          "Set",
          &(ADAPTER_IFACE, "Discoverable", target),
        )
        .await;
    }
    BluetoothCmd::SetDiscoverable(val) => {
      let target = Value::from(val);
      let _ = conn
        .call_method(
          Some(BLUEZ_DEST),
          adapter,
          Some(PROPS_IFACE),
          "Set",
          &(ADAPTER_IFACE, "Discoverable", target),
        )
        .await;
    }
    BluetoothCmd::Connect(addr) => {
      let dev_path = resolve_dev_path(shared, adapter, &addr);
      {
        let mut menu = shared.menu.lock().unwrap();
        menu.connecting_device = Some(addr.clone());
      }
      eprintln!("[bluetooth] Connecting to {addr} at {dev_path}");
      let res = conn
        .call_method(
          Some(BLUEZ_DEST),
          dev_path.as_str(),
          Some(DEVICE_IFACE),
          "Connect",
          &(),
        )
        .await;
      if let Err(e) = res {
        eprintln!("[bluetooth] Connect returned: {e}");
        let is_unpaired = shared
          .state
          .lock()
          .map(|s| {
            s.devices
              .iter()
              .any(|d| d.address.eq_ignore_ascii_case(&addr) && !d.paired)
          })
          .unwrap_or(false);
        if is_unpaired {
          let _ = conn
            .call_method(
              Some(BLUEZ_DEST),
              dev_path.as_str(),
              Some(DEVICE_IFACE),
              "Pair",
              &(),
            )
            .await;
          let _ = conn
            .call_method(
              Some(BLUEZ_DEST),
              dev_path.as_str(),
              Some(PROPS_IFACE),
              "Set",
              &(DEVICE_IFACE, "Trusted", Value::from(true)),
            )
            .await;
          let _ = conn
            .call_method(
              Some(BLUEZ_DEST),
              dev_path.as_str(),
              Some(DEVICE_IFACE),
              "Connect",
              &(),
            )
            .await;
        }
      }
      {
        let mut menu = shared.menu.lock().unwrap();
        menu.connecting_device = None;
      }
    }
    BluetoothCmd::Disconnect(addr) => {
      let dev_path = resolve_dev_path(shared, adapter, &addr);
      eprintln!("[bluetooth] Disconnecting from {addr} at {dev_path}");
      let res = conn
        .call_method(
          Some(BLUEZ_DEST),
          dev_path.as_str(),
          Some(DEVICE_IFACE),
          "Disconnect",
          &(),
        )
        .await;
      if let Err(e) = res {
        eprintln!("[bluetooth] Disconnect returned: {e}");
      }
    }
    BluetoothCmd::Pair(addr) => {
      let dev_path = resolve_dev_path(shared, adapter, &addr);
      eprintln!("[bluetooth] Pairing with {addr} at {dev_path}");
      let res = conn
        .call_method(
          Some(BLUEZ_DEST),
          dev_path.as_str(),
          Some(DEVICE_IFACE),
          "Pair",
          &(),
        )
        .await;
      if let Err(e) = res {
        eprintln!("[bluetooth] Pair returned: {e}");
      } else {
        let _ = conn
          .call_method(
            Some(BLUEZ_DEST),
            dev_path.as_str(),
            Some(PROPS_IFACE),
            "Set",
            &(DEVICE_IFACE, "Trusted", Value::from(true)),
          )
          .await;
      }
    }
    BluetoothCmd::Trust(addr, trust) => {
      let dev_path = resolve_dev_path(shared, adapter, &addr);
      let target = Value::from(trust);
      let res = conn
        .call_method(
          Some(BLUEZ_DEST),
          dev_path.as_str(),
          Some(PROPS_IFACE),
          "Set",
          &(DEVICE_IFACE, "Trusted", target),
        )
        .await;
      if let Err(e) = res {
        eprintln!("[bluetooth] Set Trusted returned: {e}");
      }
    }
    BluetoothCmd::Remove(addr) => {
      let dev_path = resolve_dev_path(shared, adapter, &addr);
      if let Ok(obj_path) = ObjectPath::try_from(dev_path.as_str()) {
        let res = conn
          .call_method(
            Some(BLUEZ_DEST),
            adapter,
            Some(ADAPTER_IFACE),
            "RemoveDevice",
            &(obj_path,),
          )
          .await;
        if let Err(e) = res {
          eprintln!("[bluetooth] RemoveDevice returned: {e}");
        }
      }
    }
  }
}

fn bump_raw(shared: &SharedBluetoothState, notify_fd: Option<i32>) {
  shared.revision.fetch_add(1, Ordering::SeqCst);
  if let Some(fd) = notify_fd {
    let buf = 1u64.to_ne_bytes();
    unsafe {
      libc::write(fd, buf.as_ptr() as *const libc::c_void, 8);
    }
  }
}

fn bump(shared: &SharedBluetoothState, notify: &Option<OwnedFd>) {
  use std::os::fd::AsRawFd;
  bump_raw(shared, notify.as_ref().map(|f| f.as_raw_fd()));
}
