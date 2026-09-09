use std::{collections::HashMap, os::fd::OwnedFd, sync::atomic::Ordering, time::Duration};

use futures_util::StreamExt;
use slowshell_commons::bluetooth::{BluetoothCmd, BluetoothDevice, SharedBluetoothState};
use zbus::zvariant::{ObjectPath, Value};

use crate::util;

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
) -> anyhow::Result<()> {
  let conn = zbus::connection::Builder::system()?.build().await?;

  let mut adapter_path: Option<String> = None;

  refresh_all(&conn, shared, &mut adapter_path).await?;
  bump(shared, notify);

  let root = zbus::Proxy::new_owned(conn.clone(), BLUEZ_DEST, "/", OBJ_MGR_IFACE).await?;
  let mut added_stream = root.receive_signal("InterfacesAdded").await?;
  let mut removed_stream = root.receive_signal("InterfacesRemoved").await?;

  let mut adapter_props_stream = if let Some(ref path) = adapter_path {
    let p = zbus::Proxy::new_owned(conn.clone(), BLUEZ_DEST, path.clone(), PROPS_IFACE).await?;
    Some(p.receive_signal("PropertiesChanged").await?)
  } else {
    None
  };

  loop {
    tokio::select! {
      Some(cmd) = cmd_rx.recv() => {
        handle_cmd(&conn, shared, &adapter_path, cmd).await;
        bump(shared, notify);
      }

      Some(_) = added_stream.next() => {
        refresh_all(&conn, shared, &mut adapter_path).await.ok();
        bump(shared, notify);
      }

      Some(_) = removed_stream.next() => {
        refresh_all(&conn, shared, &mut adapter_path).await.ok();
        bump(shared, notify);
      }

      Some(_) = async {
        if let Some(stream) = adapter_props_stream.as_mut() {
          stream.next().await
        } else {
          futures_util::future::pending().await
        }
      } => {
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
) -> anyhow::Result<()> {
  let reply = conn
    .call_method(
      Some(BLUEZ_DEST),
      "/",
      Some(OBJ_MGR_IFACE),
      "GetManagedObjects",
      &(),
    )
    .await?;

  let body = reply.body();
  let objects: HashMap<ObjectPath<'_>, HashMap<String, HashMap<String, Value<'_>>>> =
    body.deserialize()?;

  let mut found_adapter = None;
  let mut adapter_powered = false;
  let mut adapter_discovering = false;
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

      let battery = ifaces
        .get("org.bluez.Battery1")
        .and_then(|b_props| b_props.get("Percentage"))
        .and_then(|v| match v {
          Value::U8(b) => Some(*b),
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
        battery,
        rssi,
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

  let mut state = shared.state.lock().unwrap();
  state.adapter_name = found_adapter;
  state.devices = devices;

  Ok(())
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
    BluetoothCmd::Connect(addr) => {
      let dev_path = addr_to_path(adapter, &addr);
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
    BluetoothCmd::Disconnect(addr) => {
      let dev_path = addr_to_path(adapter, &addr);
      let _ = conn
        .call_method(
          Some(BLUEZ_DEST),
          dev_path.as_str(),
          Some(DEVICE_IFACE),
          "Disconnect",
          &(),
        )
        .await;
    }
    BluetoothCmd::Pair(addr) => {
      let dev_path = addr_to_path(adapter, &addr);
      let _ = conn
        .call_method(
          Some(BLUEZ_DEST),
          dev_path.as_str(),
          Some(DEVICE_IFACE),
          "Pair",
          &(),
        )
        .await;
    }
    BluetoothCmd::Trust(addr, trust) => {
      let dev_path = addr_to_path(adapter, &addr);
      let target = Value::from(trust);
      let _ = conn
        .call_method(
          Some(BLUEZ_DEST),
          dev_path.as_str(),
          Some(PROPS_IFACE),
          "Set",
          &(DEVICE_IFACE, "Trusted", target),
        )
        .await;
    }
    BluetoothCmd::Remove(addr) => {
      let dev_path = addr_to_path(adapter, &addr);
      if let Ok(obj_path) = ObjectPath::try_from(dev_path.as_str()) {
        let _ = conn
          .call_method(
            Some(BLUEZ_DEST),
            adapter,
            Some(ADAPTER_IFACE),
            "RemoveDevice",
            &(obj_path,),
          )
          .await;
      }
    }
  }
}

fn addr_to_path(adapter: &str, addr: &str) -> String {
  let formatted = addr.replace(':', "_");
  format!("{adapter}/dev_{formatted}")
}

fn bump(shared: &SharedBluetoothState, notify: &Option<OwnedFd>) {
  shared.revision.fetch_add(1, Ordering::SeqCst);
  if let Some(fd) = notify {
    util::notify_signal(fd);
  }
}
