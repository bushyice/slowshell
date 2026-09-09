use std::sync::{Arc, Mutex, atomic::AtomicBool};

#[derive(Debug, Clone, Default)]
pub struct ProcessInfo {
  pub pid: u32,
  pub name: String,
  pub cpu_usage: f32,
  pub memory: u64, // bytes
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
  pub snapshot: Mutex<SystemSnapshot>,
  pub sample_processes: AtomicBool,
}

impl Default for SystemState {
  fn default() -> Self {
    Self {
      snapshot: Mutex::new(SystemSnapshot::default()),
      sample_processes: AtomicBool::new(false),
    }
  }
}

pub type SharedSystemState = Arc<SystemState>;
