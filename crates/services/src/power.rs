use std::{fs, os::fd::OwnedFd, time::Duration};

use futures_util::StreamExt;
use slowshell_commons::power::{PowerCmd, PowerData, PowerProfile, SharedPowerState};
use zbus::{Connection, Proxy};

use crate::util;

pub fn run(
  shared: SharedPowerState,
  mut cmd_rx: tokio::sync::mpsc::UnboundedReceiver<PowerCmd>,
  notify: Option<OwnedFd>,
  duration: u64,
) {
  crate::shared_runtime().spawn(async move {
    loop {
      match power_loop(&shared, &mut cmd_rx, &notify, duration).await {
        Ok(()) => break,
        Err(e) => {
          eprintln!("[power] service loop error: {e}");
          tokio::time::sleep(Duration::from_millis(2000)).await;
        }
      }
    }
  });
}

async fn power_loop(
  shared: &SharedPowerState,
  cmd_rx: &mut tokio::sync::mpsc::UnboundedReceiver<PowerCmd>,
  notify: &Option<OwnedFd>,
  poll_seconds: u64,
) -> anyhow::Result<()> {
  let system_conn = Connection::system().await.ok();

  let upower_proxy = if let Some(ref conn) = system_conn {
    Proxy::new(
      conn,
      "org.freedesktop.UPower",
      "/org/freedesktop/UPower",
      "org.freedesktop.UPower",
    )
    .await
    .ok()
  } else {
    None
  };

  let battery_path = if let Some(ref proxy) = upower_proxy {
    find_battery_device_path(proxy).await
  } else {
    None
  };

  let battery_proxy =
    if let (Some(conn), Some(path)) = (system_conn.as_ref(), battery_path.as_ref()) {
      Proxy::new(
        conn,
        "org.freedesktop.UPower",
        path.as_str(),
        "org.freedesktop.UPower.Device",
      )
      .await
      .ok()
    } else {
      None
    };

  let power_profiles_proxy = if let Some(ref conn) = system_conn {
    Proxy::new(
      conn,
      "net.hadess.PowerProfiles",
      "/net/hadess/PowerProfiles",
      "net.hadess.PowerProfiles",
    )
    .await
    .ok()
  } else {
    None
  };

  let mut battery_stream = if let Some(ref bp) = battery_proxy {
    bp.receive_all_signals().await.ok()
  } else {
    None
  };

  let mut profiles_stream = if let Some(ref pp) = power_profiles_proxy {
    pp.receive_all_signals().await.ok()
  } else {
    None
  };

  refresh_all(
    shared,
    battery_proxy.as_ref(),
    power_profiles_proxy.as_ref(),
  )
  .await;
  bump(shared, notify);

  let mut interval = tokio::time::interval(Duration::from_secs(poll_seconds.max(3)));

  loop {
    tokio::select! {
      _ = interval.tick() => {
        if refresh_all(shared, battery_proxy.as_ref(), power_profiles_proxy.as_ref()).await {
          bump(shared, notify);
        }
      }

      Some(_) = async {
        if let Some(ref mut s) = battery_stream {
          s.next().await
        } else {
          futures_util::future::pending().await
        }
      } => {
        if refresh_all(shared, battery_proxy.as_ref(), power_profiles_proxy.as_ref()).await {
          bump(shared, notify);
        }
      }

      Some(_) = async {
        if let Some(ref mut s) = profiles_stream {
          s.next().await
        } else {
          futures_util::future::pending().await
        }
      } => {
        if refresh_all(shared, battery_proxy.as_ref(), power_profiles_proxy.as_ref()).await {
          bump(shared, notify);
        }
      }

      cmd = cmd_rx.recv() => {
        let Some(cmd) = cmd else { break; };
        handle_cmd(cmd, shared, power_profiles_proxy.as_ref()).await;
        refresh_all(shared, battery_proxy.as_ref(), power_profiles_proxy.as_ref()).await;
        bump(shared, notify);
      }
    }
  }

  Ok(())
}

fn bump(shared: &SharedPowerState, notify: &Option<OwnedFd>) {
  shared.bump();
  if let Some(fd) = notify {
    util::notify_signal(fd);
  }
}

async fn find_battery_device_path(upower: &Proxy<'_>) -> Option<String> {
  if let Ok(reply) = upower.call_method("EnumerateDevices", &()).await {
    if let Ok(devices) = reply
      .body()
      .deserialize::<Vec<zbus::zvariant::OwnedObjectPath>>()
    {
      for dev in devices {
        let path_str = dev.as_str();
        if path_str.contains("battery") {
          return Some(path_str.to_string());
        }
      }
    }
  }

  if let Ok(reply) = upower.call_method("GetDisplayDevice", &()).await {
    if let Ok(display_dev) = reply
      .body()
      .deserialize::<zbus::zvariant::OwnedObjectPath>()
    {
      return Some(display_dev.as_str().to_string());
    }
  }

  Some("/org/freedesktop/UPower/devices/DisplayDevice".to_string())
}

async fn handle_cmd(cmd: PowerCmd, shared: &SharedPowerState, profiles_proxy: Option<&Proxy<'_>>) {
  // TODO: don't do commands
  match cmd {
    PowerCmd::SetProfile(profile) => {
      let profile_str = profile.as_str();
      let mut done = false;
      if let Some(pp) = profiles_proxy {
        if pp.set_property("ActiveProfile", profile_str).await.is_ok() {
          done = true;
        }
      }
      if !done {
        let _ = std::process::Command::new("powerprofilesctl")
          .args(["set", profile_str])
          .status();
      }
    }
    PowerCmd::SetBrightness(pct) => {
      let pct_clamped = pct.clamp(1, 100);
      let _ = std::process::Command::new("brightnessctl")
        .args(["set", &format!("{pct_clamped}%")])
        .status();
    }
    PowerCmd::StepBrightness(delta) => {
      let current = {
        let d = shared.data.lock().unwrap();
        d.brightness_percent
      };
      let new_pct = (current as i32 + delta).clamp(1, 100) as u8;
      let _ = std::process::Command::new("brightnessctl")
        .args(["set", &format!("{new_pct}%")])
        .status();
    }
    PowerCmd::Refresh => {}
  }
}

async fn refresh_all(
  shared: &SharedPowerState,
  battery_proxy: Option<&Proxy<'_>>,
  profiles_proxy: Option<&Proxy<'_>>,
) -> bool {
  let mut new_data = PowerData::default();

  let mut upower_ok = false;
  if let Some(bp) = battery_proxy {
    if let Ok(pct) = bp.get_property::<f64>("Percentage").await {
      new_data.percent = Some(pct.round().clamp(0.0, 100.0) as u8);
      upower_ok = true;
    }

    if let Ok(state_code) = bp.get_property::<u32>("State").await {
      new_data.charging = state_code == 1;
      new_data.status = match state_code {
        1 => "Charging".to_string(),
        2 => "Discharging".to_string(),
        3 => "Empty".to_string(),
        4 => "Fully charged".to_string(),
        5 => "Pending charge".to_string(),
        6 => "Pending discharge".to_string(),
        _ => "Unknown".to_string(),
      };
    }

    if let Ok(energy) = bp.get_property::<f64>("Energy").await {
      new_data.energy_now_wh = Some(energy as f32);
    }
    let energy_full = bp.get_property::<f64>("EnergyFull").await.ok();
    if let Some(ef) = energy_full {
      new_data.energy_full_wh = Some(ef as f32);
    }
    if let (Some(energy_full), Ok(energy_full_design)) = (
      energy_full,
      bp.get_property::<f64>("EnergyFullDesign").await,
    ) {
      if energy_full_design > 0.0 {
        new_data.health = Some(
          ((energy_full / energy_full_design) * 100.0)
            .round()
            .clamp(0.0, 100.0) as u8,
        );
      }
    }
    if let Ok(rate) = bp.get_property::<f64>("EnergyRate").await {
      new_data.power_w = Some(rate as f32);
    }

    let time_to_empty = bp.get_property::<i64>("TimeToEmpty").await.unwrap_or(0);
    let time_to_full = bp.get_property::<i64>("TimeToFull").await.unwrap_or(0);

    if new_data.charging && time_to_full > 0 {
      let mins = (time_to_full / 60) as u32;
      let h = mins / 60;
      let m = mins % 60;
      new_data.time_remaining = Some(format!("{h}h {m}m until full"));
    } else if !new_data.charging && time_to_empty > 0 && new_data.status == "Discharging" {
      let mins = (time_to_empty / 60) as u32;
      let h = mins / 60;
      let m = mins % 60;
      new_data.time_remaining = Some(format!("{h}h {m}m remaining"));
    }
  }

  if !upower_ok {
    refresh_battery_sysfs(&mut new_data);
  }

  refresh_brightness(&mut new_data);

  let mut profiles_ok = false;
  if let Some(pp) = profiles_proxy {
    if let Ok(active_str) = pp.get_property::<String>("ActiveProfile").await {
      new_data.active_profile = Some(PowerProfile::from_str(&active_str));
      profiles_ok = true;
    }

    if let Ok(profiles_val) = pp
      .get_property::<Vec<std::collections::HashMap<String, zbus::zvariant::OwnedValue>>>(
        "Profiles",
      )
      .await
    {
      let mut profiles = Vec::new();
      for map in profiles_val {
        if let Some(p_val) = map.get("Profile") {
          if let Ok(name) = <&str>::try_from(p_val) {
            let prof = PowerProfile::from_str(name);
            if !profiles.contains(&prof) {
              profiles.push(prof);
            }
          }
        }
      }
      if !profiles.is_empty() {
        new_data.available_profiles = profiles;
        profiles_ok = true;
      }
    }
  }

  if !profiles_ok {
    refresh_profiles_cli(&mut new_data);
  }

  let mut lock = shared.data.lock().unwrap();
  let changed = lock.percent != new_data.percent
    || lock.charging != new_data.charging
    || lock.status != new_data.status
    || lock.brightness_percent != new_data.brightness_percent
    || lock.active_profile != new_data.active_profile
    || lock.time_remaining != new_data.time_remaining
    || lock.power_w != new_data.power_w;

  *lock = new_data;
  changed
}

fn refresh_battery_sysfs(data: &mut PowerData) {
  let Some(base) = fs::read_dir("/sys/class/power_supply")
    .ok()
    .and_then(|mut entries| {
      entries.find_map(|entry| {
        let name = entry.ok()?.file_name().to_string_lossy().into_owned();
        if name.starts_with("BAT") {
          Some(name)
        } else {
          None
        }
      })
    })
  else {
    return;
  };

  let dir = format!("/sys/class/power_supply/{base}");
  let read_trimmed = |file: &str| {
    fs::read_to_string(format!("{dir}/{file}"))
      .ok()
      .map(|s| s.trim().to_string())
  };
  let read_u32 = |file: &str| read_trimmed(file).and_then(|s| s.parse::<u32>().ok());

  data.percent = read_u32("capacity").map(|v| v as u8);
  let status_str = read_trimmed("status").unwrap_or_else(|| "Unknown".into());
  data.charging = status_str == "Charging";
  data.status = status_str;

  let energy_now = read_u32("energy_now").or_else(|| read_u32("charge_now"));
  let energy_full = read_u32("energy_full").or_else(|| read_u32("charge_full"));
  let energy_full_design =
    read_u32("energy_full_design").or_else(|| read_u32("charge_full_design"));
  let power_now = read_u32("power_now").or_else(|| read_u32("current_now"));

  if let Some(en) = energy_now {
    data.energy_now_wh = Some((en as f32) / 1_000_000.0);
  }
  if let Some(ef) = energy_full {
    data.energy_full_wh = Some((ef as f32) / 1_000_000.0);
  }
  if let (Some(ef), Some(efd)) = (energy_full, energy_full_design) {
    if efd > 0 {
      data.health = Some(((ef as f32 / efd as f32) * 100.0).round() as u8);
    }
  }
  if let Some(pw) = power_now {
    let watts = (pw as f32) / 1_000_000.0;
    data.power_w = Some(watts);
  }
}

fn refresh_brightness(data: &mut PowerData) {
  let bl_opt = fs::read_dir("/sys/class/backlight")
    .ok()
    .and_then(|mut entries| {
      entries.find_map(|e| {
        e.ok()
          .map(|entry| entry.file_name().to_string_lossy().into_owned())
      })
    });

  if let Some(bl) = bl_opt {
    let dir = format!("/sys/class/backlight/{bl}");
    data.device_name = bl;
    let cur = fs::read_to_string(format!("{dir}/brightness"))
      .ok()
      .and_then(|s| s.trim().parse::<u32>().ok())
      .unwrap_or(0);
    let max = fs::read_to_string(format!("{dir}/max_brightness"))
      .ok()
      .and_then(|s| s.trim().parse::<u32>().ok())
      .unwrap_or(1);

    data.brightness_current = cur;
    data.brightness_max = max;
    if max > 0 {
      data.brightness_percent = ((cur as f32 / max as f32) * 100.0)
        .round()
        .clamp(0.0, 100.0) as u8;
    }
  } else if let Ok(output) = std::process::Command::new("brightnessctl")
    .args(["-m", "info"])
    .output()
  {
    if let Ok(stdout) = String::from_utf8(output.stdout) {
      let parts: Vec<&str> = stdout.trim().split(',').collect();
      if parts.len() >= 5 {
        data.device_name = parts[0].to_string();
        data.brightness_current = parts[2].parse().unwrap_or(0);
        data.brightness_percent = parts[3].trim_end_matches('%').parse().unwrap_or(0);
        data.brightness_max = parts[4].parse().unwrap_or(1);
      }
    }
  }
}

fn refresh_profiles_cli(data: &mut PowerData) {
  if let Ok(output) = std::process::Command::new("powerprofilesctl")
    .arg("list")
    .output()
  {
    if let Ok(stdout) = String::from_utf8(output.stdout) {
      let mut profiles = Vec::new();
      let mut active = None;

      for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
          continue;
        }

        let is_active = line.starts_with('*');
        let profile_candidate = if let Some(idx) = trimmed.find(':') {
          let name = trimmed[..idx].trim_start_matches('*').trim();
          Some(PowerProfile::from_str(name))
        } else {
          None
        };

        if let Some(p) = profile_candidate {
          if is_active {
            active = Some(p.clone());
          }
          if !profiles.contains(&p) {
            profiles.push(p);
          }
        }
      }

      data.available_profiles = profiles;
      data.active_profile = active;
    }
  }
}
