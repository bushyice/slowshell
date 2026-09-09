use std::sync::{
  Arc, Mutex,
  atomic::{AtomicU64, Ordering},
};

pub const POWER_CHANGED: &str = "component/power.changed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PowerProfile {
  PowerSaver,
  Balanced,
  Performance,
  Unknown(String),
}

impl PowerProfile {
  pub fn as_str(&self) -> &str {
    match self {
      PowerProfile::PowerSaver => "power-saver",
      PowerProfile::Balanced => "balanced",
      PowerProfile::Performance => "performance",
      PowerProfile::Unknown(s) => s.as_str(),
    }
  }

  pub fn label(&self) -> &str {
    match self {
      PowerProfile::PowerSaver => "Power Saver",
      PowerProfile::Balanced => "Balanced",
      PowerProfile::Performance => "Performance",
      PowerProfile::Unknown(s) => s.as_str(),
    }
  }

  pub fn icon(&self) -> &'static str {
    match self {
      PowerProfile::PowerSaver => "power-profile-power-saver-symbolic",
      PowerProfile::Balanced => "power-profile-balanced-symbolic",
      PowerProfile::Performance => "power-profile-performance-symbolic",
      PowerProfile::Unknown(_) => "power-profile-balanced-symbolic",
    }
  }

  pub fn from_str(s: &str) -> Self {
    match s.trim() {
      "power-saver" => PowerProfile::PowerSaver,
      "balanced" => PowerProfile::Balanced,
      "performance" => PowerProfile::Performance,
      other => PowerProfile::Unknown(other.to_string()),
    }
  }
}

#[derive(Debug, Clone, Default)]
pub struct PowerData {
  pub percent: Option<u8>,
  pub charging: bool,
  pub status: String,
  pub health: Option<u8>,
  pub energy_now_wh: Option<f32>,
  pub energy_full_wh: Option<f32>,
  pub power_w: Option<f32>,
  pub time_remaining: Option<String>,
  pub active_profile: Option<PowerProfile>,
  pub available_profiles: Vec<PowerProfile>,
  pub brightness_percent: u8,
  pub brightness_max: u32,
  pub brightness_current: u32,
  pub device_name: String,
}

#[derive(Debug, Clone)]
pub enum PowerCmd {
  SetProfile(PowerProfile),
  SetBrightness(u8),
  StepBrightness(i32),
  Refresh,
}

pub struct PowerState {
  pub data: Mutex<PowerData>,
  pub revision: AtomicU64,
  pub cmd_tx: tokio::sync::mpsc::UnboundedSender<PowerCmd>,
}

impl PowerState {
  pub fn bump(&self) {
    self.revision.fetch_add(1, Ordering::SeqCst);
  }
}

pub type SharedPowerState = Arc<PowerState>;

pub fn brightness_icon(percent: u8) -> &'static str {
  match percent {
    0..=25 => "display-brightness-low-symbolic",
    26..=65 => "display-brightness-medium-symbolic",
    _ => "display-brightness-high-symbolic",
  }
}

pub fn battery_icon(charging: bool, percent: Option<u8>) -> &'static str {
  match (charging, percent) {
    (true, Some(p)) => match p {
      80..=100 => "battery-full-charging-symbolic",
      40..=79 => "battery-good-charging-symbolic",
      20..=39 => "battery-low-charging-symbolic",
      _ => "battery-caution-charging-symbolic",
    },
    (true, None) => "battery-caution-charging-symbolic",
    (false, Some(p)) => match p {
      80..=100 => "battery-full-symbolic",
      40..=79 => "battery-good-symbolic",
      20..=39 => "battery-low-symbolic",
      _ => "battery-caution-symbolic",
    },
    (false, None) => "battery-missing-symbolic",
  }
}
