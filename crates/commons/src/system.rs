use std::sync::{Arc, Mutex, OnceLock, atomic::AtomicBool};

#[derive(Debug, Clone, Default)]
pub struct ProcessInfo {
  pub pid: u32,
  pub name: String,
  pub name_display: String,
  pub cpu_usage: f32,
  pub cpu_display: String,
  pub memory: u64, // bytes
  pub memory_display: String,
}

#[derive(Debug, Clone, Default)]
pub struct SystemSnapshot {
  pub cpu_usage: f32,
  pub mem_usage: f32,
  pub mem_total: u64,
  pub mem_used: u64,
  pub temperature: Option<f32>,
  pub load: [f32; 3],
  pub network_state: u16,
  pub network_rx: u64,
  pub network_tx: u64,
  pub top_processes: Vec<ProcessInfo>,
}

pub struct SystemState {
  pub snapshot: Mutex<Arc<SystemSnapshot>>,
  pub sample_processes: AtomicBool,
}

impl Default for SystemState {
  fn default() -> Self {
    Self {
      snapshot: Mutex::new(Arc::new(SystemSnapshot::default())),
      sample_processes: AtomicBool::new(false),
    }
  }
}

pub type SharedSystemState = Arc<SystemState>;

pub fn shared() -> SharedSystemState {
  static SHARED: OnceLock<SharedSystemState> = OnceLock::new();
  SHARED
    .get_or_init(|| Arc::new(SystemState::default()))
    .clone()
}
