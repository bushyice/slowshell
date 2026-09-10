use std::{
  collections::{HashMap, HashSet},
  os::fd::OwnedFd,
  pin::Pin,
  sync::atomic::Ordering,
  time::{Duration, Instant},
};

use futures_util::StreamExt;
use slowshell_commons::network::{
  AccessPoint, ConnectionInfo, EthernetState, NetCmd, SharedNetworkState,
};
use zbus::zvariant::{OwnedObjectPath, Value};

use crate::util;

const DEST: &str = "org.freedesktop.NetworkManager";
const NM_PATH: &str = "/org/freedesktop/NetworkManager";
const NM_IFACE: &str = "org.freedesktop.NetworkManager";
const DEV_IFACE: &str = "org.freedesktop.NetworkManager.Device";
const WIFI_IFACE: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const WIRED_IFACE: &str = "org.freedesktop.NetworkManager.Device.Wired";
const AP_IFACE: &str = "org.freedesktop.NetworkManager.AccessPoint";
const PROPS_IFACE: &str = "org.freedesktop.DBus.Properties";

const DEV_ETHERNET: u32 = 1;
const DEV_WIRELESS: u32 = 2;
const NM_ACTIVATED: u32 = 100;

type SourceStream = Pin<Box<dyn futures_util::Stream<Item = Source> + Send>>;
type Sources = futures_util::stream::SelectAll<SourceStream>;

#[derive(Debug)]
enum Source {
  RootSignal(&'static str, zbus::Message),
  RootProps,
  Dev(String),
}

struct SourceRegistry {
  all: Sources,
  devs: HashSet<String>,
}

impl SourceRegistry {
  async fn new(conn: &zbus::Connection) -> zbus::Result<Self> {
    let mut all = Sources::new();

    let root = zbus::Proxy::new_owned(conn.clone(), DEST, NM_PATH, NM_IFACE).await?;
    for name in [
      "DeviceAdded",
      "DeviceRemoved",
      "AccessPointAdded",
      "AccessPointRemoved",
    ] {
      let stream = root.receive_signal(name).await?;
      all.push(Box::pin(stream.map(move |m| Source::RootSignal(name, m))));
    }

    let root_props = zbus::Proxy::new_owned(conn.clone(), DEST, NM_PATH, PROPS_IFACE).await?;
    let props = root_props.receive_signal("PropertiesChanged").await?;
    all.push(Box::pin(props.map(|_| Source::RootProps)));

    Ok(Self {
      all,
      devs: HashSet::new(),
    })
  }

  async fn ensure_dev(&mut self, conn: &zbus::Connection, path: &str) -> zbus::Result<()> {
    if self.devs.insert(path.to_string()) {
      let props = zbus::Proxy::new_owned(conn.clone(), DEST, path.to_string(), PROPS_IFACE).await?;
      let stream = props.receive_signal("PropertiesChanged").await?;
      let p = path.to_string();
      self
        .all
        .push(Box::pin(stream.map(move |_| Source::Dev(p.clone()))));
    }
    Ok(())
  }
}

async fn sync_sources(
  conn: &zbus::Connection,
  registry: &mut SourceRegistry,
  devices: &HashMap<String, DeviceInfo>,
) -> zbus::Result<()> {
  for path in devices.keys() {
    registry.ensure_dev(conn, path).await?;
  }
  Ok(())
}

#[derive(Debug, Clone)]
struct DeviceInfo {
  path: String,
  iface: String,
  dev_type: u32,
  state: u32,
  active_conn: String,
  active_ap: String,
  carrier: bool,
  speed: u32,
  mac: String,
  ip4: Vec<String>,
}

pub fn run(
  shared: SharedNetworkState,
  mut cmd_rx: tokio::sync::mpsc::UnboundedReceiver<NetCmd>,
  notify: Option<OwnedFd>,
) {
  crate::shared_runtime().spawn(async move {
    loop {
      match net_loop(&shared, &mut cmd_rx, &notify).await {
        Ok(()) => break,
        Err(e) => {
          eprintln!("[network] dbus loop ended: {e}");
          tokio::time::sleep(Duration::from_millis(2000)).await;
        }
      }
    }
  });
}

async fn net_loop(
  shared: &SharedNetworkState,
  cmd_rx: &mut tokio::sync::mpsc::UnboundedReceiver<NetCmd>,
  notify: &Option<OwnedFd>,
) -> zbus::Result<()> {
  let conn = zbus::Connection::system().await?;

  let mut devices: HashMap<String, DeviceInfo> = HashMap::new();
  let mut aps: HashMap<String, AccessPoint> = HashMap::new();

  refresh_radio(&conn, shared, notify).await?;
  refresh_devices(&conn, &mut devices, &mut aps, shared, notify).await?;

  let mut registry = SourceRegistry::new(&conn).await?;
  sync_sources(&conn, &mut registry, &devices).await?;
  let mut scan_until: Option<Instant> = None;

  loop {
    let mut scan_sleep = scan_until.map(|until| Box::pin(tokio::time::sleep_until(until.into())));

    tokio::select! {
      cmd = cmd_rx.recv() => {
        let Some(cmd) = cmd else { return Ok(()) };
        match handle_cmd(
          &conn,
          &mut devices,
          &mut aps,
          shared,
          notify,
          cmd,
        )
        .await
        {
          Ok((scanning, _rebuild)) => {
            if scanning {
              scan_until = Some(Instant::now() + Duration::from_secs(3));
            }
          }
          Err(e) => {
            eprintln!("[network] command failed: {e}");
            continue;
          }
        }
        sync_sources(&conn, &mut registry, &devices).await?;
      }
      _ = async {
        if let Some(sleep) = scan_sleep.as_mut() {
          sleep.as_mut().await;
        }
      }, if scan_until.is_some() => {
        scan_until = None;
        shared.scanning.store(false, Ordering::Relaxed);
        bump_notify(shared, notify);
      }
      Some(source) = registry.all.next() => {
        if let Err(e) = handle_event(&conn, &mut devices, &mut aps, shared, notify, &source).await {
          eprintln!("[network] event handling failed: {e}");
        }
        sync_sources(&conn, &mut registry, &devices).await?;
      }
    }
  }
}

async fn handle_event(
  conn: &zbus::Connection,
  devices: &mut HashMap<String, DeviceInfo>,
  aps: &mut HashMap<String, AccessPoint>,
  shared: &SharedNetworkState,
  notify: &Option<OwnedFd>,
  source: &Source,
) -> zbus::Result<()> {
  match source {
    Source::RootSignal("DeviceAdded", _) | Source::RootSignal("DeviceRemoved", _) => {
      refresh_devices(conn, devices, aps, shared, notify).await?;
    }
    Source::RootSignal("AccessPointAdded", msg) => {
      if let Ok((path,)) = msg.body().deserialize::<(OwnedObjectPath,)>() {
        refresh_ap(conn, &path.to_string(), devices, aps, shared, notify).await?;
      }
    }
    Source::RootSignal("AccessPointRemoved", msg) => {
      if let Ok((path,)) = msg.body().deserialize::<(OwnedObjectPath,)>() {
        aps.remove(&path.to_string());
        publish(shared, devices, aps, notify);
      }
    }
    Source::RootSignal(_name, _msg) => {}
    Source::RootProps => {
      refresh_radio(conn, shared, notify).await?;
    }
    Source::Dev(path) => {
      if devices.contains_key(path) {
        refresh_device(conn, path, devices, aps, shared, notify).await?;
      }
    }
  }
  Ok(())
}

async fn handle_cmd(
  conn: &zbus::Connection,
  devices: &mut HashMap<String, DeviceInfo>,
  aps: &mut HashMap<String, AccessPoint>,
  shared: &SharedNetworkState,
  notify: &Option<OwnedFd>,
  cmd: NetCmd,
) -> zbus::Result<(bool, bool)> {
  match cmd {
    NetCmd::Connect { ssid, password } => {
      set_busy(shared, notify, true);
      let res = connect_to_network(conn, shared, &ssid, password).await;
      set_busy(shared, notify, false);
      match res {
        Ok(()) => {}
        Err(e) => eprintln!("[network] connect failed: {e}"),
      }
      *shared.saved_ssids.lock().unwrap() = None;
      refresh_devices(conn, devices, aps, shared, notify).await?;
      Ok((false, false))
    }
    NetCmd::DisconnectWifi => {
      set_busy(shared, notify, true);
      let res = disconnect_active_wifi(conn, devices).await;
      set_busy(shared, notify, false);
      if let Err(e) = res {
        eprintln!("[network] wifi disconnect failed: {e}");
      }
      *shared.saved_ssids.lock().unwrap() = None;
      refresh_devices(conn, devices, aps, shared, notify).await?;
      Ok((false, false))
    }
    NetCmd::WiredConnect => {
      set_busy(shared, notify, true);
      let res = toggle_wired(conn, devices).await;
      set_busy(shared, notify, false);
      if let Err(e) = res {
        eprintln!("[network] wired toggle failed: {e}");
      }
      refresh_devices(conn, devices, aps, shared, notify).await?;
      Ok((false, false))
    }
    NetCmd::ToggleWifi => {
      set_busy(shared, notify, true);
      let res = toggle_wifi_radio(conn).await;
      set_busy(shared, notify, false);
      if let Err(e) = res {
        eprintln!("[network] wifi radio toggle failed: {e}");
      }
      refresh_radio(conn, shared, notify).await?;
      Ok((false, false))
    }
    NetCmd::Scan => {
      shared.scanning.store(true, Ordering::Relaxed);
      let res = request_scan(conn, devices).await;
      if let Err(e) = res {
        eprintln!("[network] scan failed: {e}");
        shared.scanning.store(false, Ordering::Relaxed);
      }
      bump_notify(shared, notify);
      Ok((true, false))
    }
  }
}

async fn connect_to_network(
  conn: &zbus::Connection,
  shared: &SharedNetworkState,
  ssid: &str,
  password: Option<String>,
) -> zbus::Result<()> {
  let root = zbus::Proxy::new_owned(conn.clone(), DEST, NM_PATH, NM_IFACE).await?;
  let saved = load_saved_ssids(conn, shared).await;
  let devices: Vec<OwnedObjectPath> = root.get_property::<Vec<OwnedObjectPath>>("Devices").await?;

  for dev in devices {
    let base = zbus::Proxy::new_owned(conn.clone(), DEST, dev.clone(), DEV_IFACE).await?;
    if base.get_property::<u32>("DeviceType").await.unwrap_or(0) != DEV_WIRELESS {
      continue;
    }
    let wifi = zbus::Proxy::new_owned(conn.clone(), DEST, dev.clone(), WIFI_IFACE).await?;
    let aps: Vec<OwnedObjectPath> = wifi
      .get_property::<Vec<OwnedObjectPath>>("AccessPoints")
      .await
      .unwrap_or_default();
    for ap_path in aps {
      let ap = zbus::Proxy::new_owned(conn.clone(), DEST, ap_path.clone(), AP_IFACE).await?;
      let bytes: Vec<u8> = ap.get_property("Ssid").await.unwrap_or_default();
      if String::from_utf8_lossy(&bytes).trim_end_matches('\0') != ssid {
        continue;
      }
      if saved.contains(ssid) {
        root
          .call_method("ActivateConnection", &(obj_path("/"), dev, ap_path))
          .await?;
      } else {
        let settings = build_settings(ssid, password.as_deref());
        let options: HashMap<String, Value> = HashMap::new();
        let reply = root
          .call_method(
            "AddAndActivateConnection2",
            &(settings, dev, ap_path, options, 0_i32),
          )
          .await;
        match reply {
          Ok(_) => {}
          Err(e) => return Err(zbus::Error::Failure(format!("AddAndActivateConnection2 failed: {e}"))),
        }
      }
      return Ok(());
    }
  }
  Err(zbus::Error::Failure(format!("no access point named '{ssid}' found")))
}

async fn disconnect_active_wifi(
  conn: &zbus::Connection,
  devices: &HashMap<String, DeviceInfo>,
) -> zbus::Result<()> {
  let root = zbus::Proxy::new_owned(conn.clone(), DEST, NM_PATH, NM_IFACE).await?;
  for info in devices.values() {
    if info.dev_type == DEV_WIRELESS
      && info.state == NM_ACTIVATED
      && !info.active_conn.is_empty()
      && info.active_conn != "/"
    {
      let path = OwnedObjectPath::try_from(info.active_conn.as_str())?;
      root.call_method("DeactivateConnection", &(path,)).await?;
      return Ok(());
    }
  }
  Err(zbus::Error::Failure("no active wifi connection to disconnect".into()))
}

async fn toggle_wired(
  conn: &zbus::Connection,
  devices: &HashMap<String, DeviceInfo>,
) -> zbus::Result<()> {
  let root = zbus::Proxy::new_owned(conn.clone(), DEST, NM_PATH, NM_IFACE).await?;
  let mut target: Option<String> = None;
  let mut active_conn: Option<String> = None;

  for info in devices.values() {
    if info.dev_type == DEV_ETHERNET {
      target = Some(info.path.clone());
      if info.state == NM_ACTIVATED && !info.active_conn.is_empty() && info.active_conn != "/" {
        active_conn = Some(info.active_conn.clone());
      }
      break;
    }
  }

  let Some(dev) = target else {
    return Err(zbus::Error::Failure("no wired device found".into()));
  };

  if let Some(conn_path) = active_conn {
    let path = OwnedObjectPath::try_from(conn_path.as_str())?;
    root.call_method("DeactivateConnection", &(path,)).await?;
    Ok(())
  } else {
    let settings: HashMap<String, HashMap<String, Value>> = HashMap::new();
    let dev_path = OwnedObjectPath::try_from(dev.as_str())?;
    let specific = OwnedObjectPath::try_from("/")?;
    root
      .call_method("AddAndActivateConnection", &(settings, dev_path, specific))
      .await?;
    Ok(())
  }
}

async fn toggle_wifi_radio(conn: &zbus::Connection) -> zbus::Result<()> {
  let root = zbus::Proxy::new_owned(conn.clone(), DEST, NM_PATH, NM_IFACE).await?;
  let current = root.get_property::<bool>("WirelessEnabled").await?;
  root.set_property("WirelessEnabled", !current).await?;
  Ok(())
}

async fn request_scan(
  conn: &zbus::Connection,
  devices: &HashMap<String, DeviceInfo>,
) -> zbus::Result<()> {
  for info in devices.values() {
    if info.dev_type == DEV_WIRELESS {
      let wifi = zbus::Proxy::new_owned(conn.clone(), DEST, info.path.clone(), WIFI_IFACE).await?;
      let options: HashMap<String, Value> = HashMap::new();
      if let Err(e) = wifi.call_method("RequestScan", &(options,)).await {
        eprintln!("[network] RequestScan on {} failed: {e}", info.iface);
      }
    }
  }
  Ok(())
}

fn build_settings(
  ssid: &str,
  password: Option<&str>,
) -> HashMap<String, HashMap<String, Value<'static>>> {
  let mut wireless: HashMap<String, Value<'static>> = HashMap::new();
  wireless.insert("ssid".to_string(), Value::from(ssid.as_bytes().to_vec()));
  wireless.insert(
    "mode".to_string(),
    Value::from("infrastructure".to_string()),
  );

  let mut settings: HashMap<String, HashMap<String, Value<'static>>> = HashMap::new();
  settings.insert("802-11-wireless".to_string(), wireless);

  if let Some(pw) = password
    && !pw.is_empty()
  {
    let mut security: HashMap<String, Value<'static>> = HashMap::new();
    security.insert("key-mgmt".to_string(), Value::from("wpa-psk".to_string()));
    security.insert("psk".to_string(), Value::from(pw.to_string()));
    settings.insert("802-11-wireless-security".to_string(), security);
  }
  settings
}

async fn refresh_radio(
  conn: &zbus::Connection,
  shared: &SharedNetworkState,
  notify: &Option<OwnedFd>,
) -> zbus::Result<()> {
  let root = zbus::Proxy::new_owned(conn.clone(), DEST, NM_PATH, NM_IFACE).await?;
  let enabled = root
    .get_property::<bool>("WirelessEnabled")
    .await
    .unwrap_or(false);
  let hw = root
    .get_property::<bool>("WirelessHardwareEnabled")
    .await
    .unwrap_or(false);

  let changed = enabled != shared.wifi_enabled.load(Ordering::Relaxed)
    || hw != shared.wifi_hardware_enabled.load(Ordering::Relaxed);
  shared.wifi_enabled.store(enabled, Ordering::Relaxed);
  shared.wifi_hardware_enabled.store(hw, Ordering::Relaxed);
  if changed {
    bump_notify(shared, notify);
  }
  Ok(())
}

async fn refresh_devices(
  conn: &zbus::Connection,
  devices: &mut HashMap<String, DeviceInfo>,
  aps: &mut HashMap<String, AccessPoint>,
  shared: &SharedNetworkState,
  notify: &Option<OwnedFd>,
) -> zbus::Result<()> {
  let root = zbus::Proxy::new_owned(conn.clone(), DEST, NM_PATH, NM_IFACE).await?;
  let paths: Vec<OwnedObjectPath> = root
    .get_property::<Vec<OwnedObjectPath>>("Devices")
    .await
    .unwrap_or_default();

  let mut seen: HashSet<String> = HashSet::new();
  for path in paths {
    let key = path.to_string();
    seen.insert(key.clone());
    let Ok(info) = fetch_device(conn, &key).await else {
      continue;
    };
    if info.dev_type == DEV_WIRELESS && !aps_alive_for(&key, aps) {
      let _ = refresh_aps_for(conn, &key, aps).await;
    }
    devices.insert(key, info);
  }
  devices.retain(|key, _| seen.contains(key));

  let saved = load_saved_ssids(conn, shared).await;
  mark_saved(aps, &saved);

  publish(shared, devices, aps, notify);
  Ok(())
}

async fn refresh_device(
  conn: &zbus::Connection,
  path: &str,
  devices: &mut HashMap<String, DeviceInfo>,
  aps: &mut HashMap<String, AccessPoint>,
  shared: &SharedNetworkState,
  notify: &Option<OwnedFd>,
) -> zbus::Result<()> {
  let info = fetch_device(conn, path).await?;
  if info.dev_type == DEV_WIRELESS {
    refresh_aps_for(conn, path, aps).await?;
  }
  devices.insert(path.to_string(), info);
  let saved = load_saved_ssids(conn, shared).await;
  mark_saved(aps, &saved);
  publish(shared, devices, aps, notify);
  Ok(())
}

async fn fetch_device(conn: &zbus::Connection, path: &str) -> zbus::Result<DeviceInfo> {
  let base = zbus::Proxy::new_owned(conn.clone(), DEST, path.to_string(), DEV_IFACE).await?;
  let dev_type = base.get_property::<u32>("DeviceType").await.unwrap_or(0);
  let state = base.get_property::<u32>("State").await.unwrap_or(0);
  let iface = base
    .get_property::<String>("Interface")
    .await
    .unwrap_or_default();
  let active_conn = base
    .get_property::<OwnedObjectPath>("ActiveConnection")
    .await
    .unwrap_or_else(|_| obj_path("/"));
  let mac = base
    .get_property::<String>("HwAddress")
    .await
    .unwrap_or_default();
  let ip4_path = base
    .get_property::<OwnedObjectPath>("Ip4Config")
    .await
    .unwrap_or_else(|_| obj_path("/"));
  let mut info = DeviceInfo {
    path: path.to_string(),
    iface,
    dev_type,
    state,
    active_conn: active_conn.to_string(),
    active_ap: String::new(),
    carrier: false,
    speed: 0,
    mac,
    ip4: Vec::new(),
  };
  if ip4_path.as_str() != "/" {
    info.ip4 = fetch_ip4(conn, ip4_path.as_str()).await;
  }
  if dev_type == DEV_WIRELESS {
    if let Ok(wifi) = zbus::Proxy::new_owned(conn.clone(), DEST, path.to_string(), WIFI_IFACE).await
    {
      info.active_ap = wifi
        .get_property::<OwnedObjectPath>("ActiveAccessPoint")
        .await
        .unwrap_or_else(|_| obj_path("/"))
        .to_string();
    }
  } else if dev_type == DEV_ETHERNET {
    if let Ok(wired) =
      zbus::Proxy::new_owned(conn.clone(), DEST, path.to_string(), WIRED_IFACE).await
    {
      info.carrier = wired.get_property::<bool>("Carrier").await.unwrap_or(false);
      info.speed = wired.get_property::<u32>("Speed").await.unwrap_or(0);
    }
  }
  Ok(info)
}

async fn fetch_ip4(conn: &zbus::Connection, path: &str) -> Vec<String> {
  let Ok(props) = zbus::Proxy::new_owned(
    conn.clone(),
    DEST,
    path.to_string(),
    "org.freedesktop.NetworkManager.IP4Config",
  )
  .await
  else {
    return Vec::new();
  };
  let Ok(addrs) = props.get_property::<Vec<Vec<u32>>>("Addresses").await else {
    return Vec::new();
  };
  addrs
    .into_iter()
    .filter_map(|mut a| {
      if a.is_empty() {
        None
      } else {
        Some(format_ipv4(a.remove(0)))
      }
    })
    .collect()
}

fn format_ipv4(v: u32) -> String {
  format!(
    "{}.{}.{}.{}",
    (v >> 24) & 0xff,
    (v >> 16) & 0xff,
    (v >> 8) & 0xff,
    v & 0xff
  )
}

async fn load_saved_ssids(conn: &zbus::Connection, shared: &SharedNetworkState) -> HashSet<String> {
  {
    let cache = shared.saved_ssids.lock().unwrap();
    if let Some(ref cached) = *cache {
      return cached.clone();
    }
  }
  let out = load_saved_ssids_uncached(conn).await;
  *shared.saved_ssids.lock().unwrap() = Some(out.clone());
  out
}

async fn load_saved_ssids_uncached(conn: &zbus::Connection) -> HashSet<String> {
  let Ok(settings) = zbus::Proxy::new_owned(
    conn.clone(),
    DEST,
    "/org/freedesktop/NetworkManager/Settings",
    "org.freedesktop.NetworkManager.Settings",
  )
  .await
  else {
    return HashSet::new();
  };
  let paths: Vec<OwnedObjectPath> = settings
    .get_property("Connections")
    .await
    .unwrap_or_default();
  let mut out = HashSet::new();
  for path in paths {
    let Ok(profile) = zbus::Proxy::new_owned(
      conn.clone(),
      DEST,
      path,
      "org.freedesktop.NetworkManager.Settings.Connection",
    )
    .await
    else {
      continue;
    };
    let Ok(map) = profile.call_method("GetSettings", &()).await.and_then(|m| {
      m.body()
        .deserialize::<HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>>>()
    }) else {
      continue;
    };
    let Some(wireless) = map.get("802-11-wireless") else {
      continue;
    };
    let Some(ssid_value) = wireless.get("ssid") else {
      continue;
    };
    let bytes: Vec<u8> = match &**ssid_value {
      zbus::zvariant::Value::Array(arr) => arr
        .iter()
        .filter_map(|v| v.downcast_ref::<u8>().ok())
        .collect(),
      _ => Vec::new(),
    };
    let ssid = String::from_utf8_lossy(&bytes)
      .trim_end_matches('\u{0}')
      .to_string();
    if !ssid.is_empty() {
      out.insert(ssid);
    }
  }
  out
}

fn mark_saved(aps: &mut HashMap<String, AccessPoint>, saved: &HashSet<String>) {
  for ap in aps.values_mut() {
    ap.saved = saved.contains(&ap.ssid);
  }
}

async fn refresh_ap(
  conn: &zbus::Connection,
  path: &str,
  devices: &mut HashMap<String, DeviceInfo>,
  aps: &mut HashMap<String, AccessPoint>,
  shared: &SharedNetworkState,
  notify: &Option<OwnedFd>,
) -> zbus::Result<()> {
  let ap = fetch_ap(conn, path).await?;
  aps.insert(path.to_string(), ap);
  let saved = load_saved_ssids(conn, shared).await;
  mark_saved(aps, &saved);
  publish(shared, devices, aps, notify);
  Ok(())
}

fn aps_alive_for(_dev_path: &str, _aps: &HashMap<String, AccessPoint>) -> bool {
  !_aps.is_empty()
}

async fn refresh_aps_for(
  conn: &zbus::Connection,
  dev_path: &str,
  aps: &mut HashMap<String, AccessPoint>,
) -> zbus::Result<()> {
  let wifi = zbus::Proxy::new_owned(conn.clone(), DEST, dev_path.to_string(), WIFI_IFACE).await?;
  let paths: Vec<OwnedObjectPath> = wifi
    .get_property::<Vec<OwnedObjectPath>>("AccessPoints")
    .await
    .unwrap_or_default();
  for p in paths {
    let key = p.to_string();
    if !aps.contains_key(&key) {
      aps.insert(key.clone(), fetch_ap(conn, &key).await?);
    }
  }
  Ok(())
}

async fn fetch_ap(conn: &zbus::Connection, path: &str) -> zbus::Result<AccessPoint> {
  let ap = zbus::Proxy::new_owned(conn.clone(), DEST, path.to_string(), AP_IFACE).await?;
  let ssid = ap.get_property::<Vec<u8>>("Ssid").await.unwrap_or_default();
  let signal = ap.get_property::<u8>("Strength").await.unwrap_or(0);
  let wpa = ap.get_property::<u32>("WpaFlags").await.unwrap_or(0);
  let rsn = ap.get_property::<u32>("RsnFlags").await.unwrap_or(0);
  Ok(AccessPoint {
    ssid: String::from_utf8_lossy(&ssid)
      .trim_end_matches('\u{0}')
      .to_string(),
    signal,
    secured: wpa != 0 || rsn != 0,
    in_use: false,
    saved: false,
  })
}

fn publish(
  shared: &SharedNetworkState,
  devices: &HashMap<String, DeviceInfo>,
  aps: &HashMap<String, AccessPoint>,
  notify: &Option<OwnedFd>,
) {
  let active_aps: HashSet<&str> = devices
    .values()
    .filter(|d| d.dev_type == DEV_WIRELESS)
    .filter(|d| !d.active_ap.is_empty() && d.active_ap != "/")
    .map(|d| d.active_ap.as_str())
    .collect();

  let mut by_ssid: HashMap<String, AccessPoint> = HashMap::new();
  for (path, ap) in aps {
    let mut ap = ap.clone();
    ap.in_use = active_aps.contains(path.as_str());
    if ap.ssid.is_empty() {
      continue;
    }
    match by_ssid.get(&ap.ssid) {
      Some(existing) => {
        let replace = (ap.in_use && !existing.in_use)
          || (ap.in_use == existing.in_use && ap.signal > existing.signal);
        if replace {
          by_ssid.insert(ap.ssid.clone(), ap);
        }
      }
      None => {
        by_ssid.insert(ap.ssid.clone(), ap);
      }
    }
  }
  let mut networks: Vec<AccessPoint> = by_ssid.into_values().collect();
  networks.sort_by(|a, b| {
    b.in_use
      .cmp(&a.in_use)
      .then(b.signal.cmp(&a.signal))
      .then(a.ssid.cmp(&b.ssid))
  });

  let mut wifi_connected = None;
  let mut ethernet = None;
  let mut connected = None;
  for info in devices.values() {
    if info.dev_type == DEV_WIRELESS && info.state == NM_ACTIVATED {
      let label = aps
        .get(&info.active_ap)
        .filter(|_| info.active_ap != "/")
        .map(|ap| {
          if ap.ssid.is_empty() {
            "Network".to_string()
          } else {
            ap.ssid.clone()
          }
        })
        .unwrap_or_else(|| "Network".to_string());
      connected = Some(ConnectionInfo {
        label,
        is_wifi: true,
        mac: info.mac.clone(),
        ip4: info.ip4.clone(),
      });
      if let Some(ap) = aps.get(&info.active_ap).filter(|_| info.active_ap != "/") {
        wifi_connected = Some(ap.clone());
      }
    } else if info.dev_type == DEV_ETHERNET {
      let candidate = EthernetState {
        iface: info.iface.clone(),
        speed: info.speed,
        carrier: info.carrier,
        connected: info.state == NM_ACTIVATED,
      };
      let better = match &ethernet {
        None => true,
        Some(current) => {
          let weight = |e: &EthernetState| (e.connected as u8, e.carrier as u8);
          weight(&candidate) > weight(current)
        }
      };
      if better {
        ethernet = Some(candidate);
      }
      if info.state == NM_ACTIVATED && connected.is_none() {
        connected = Some(ConnectionInfo {
          label: "Ethernet".to_string(),
          is_wifi: false,
          mac: info.mac.clone(),
          ip4: info.ip4.clone(),
        });
      }
    }
  }

  let mut guard = shared.state.lock().unwrap();
  guard.wifi_connected = wifi_connected;
  guard.ethernet = ethernet;
  guard.networks = networks;
  guard.connected = connected;
  drop(guard);

  bump_notify(shared, notify);
}

fn set_busy(shared: &SharedNetworkState, notify: &Option<OwnedFd>, busy: bool) {
  shared.busy.store(busy, Ordering::Relaxed);
  bump_notify(shared, notify);
}

fn bump_notify(shared: &SharedNetworkState, notify: &Option<OwnedFd>) {
  shared.revision.fetch_add(1, Ordering::Relaxed);
  if let Some(fd) = notify {
    util::notify_signal(fd);
  }
}

fn obj_path(path: &str) -> OwnedObjectPath {
  OwnedObjectPath::try_from(path)
    .unwrap_or_else(|_| OwnedObjectPath::try_from("/").expect("valid object path"))
}
