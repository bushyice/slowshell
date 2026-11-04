#[cfg(feature = "audio")]
pub mod audio;
#[cfg(feature = "bluetooth")]
pub mod bluetooth;
pub mod foreground;
#[allow(dead_code)]
pub(crate) mod lifecycle;
#[cfg(feature = "network")]
pub mod network;
#[cfg(feature = "notifications")]
pub mod notifications;
#[cfg(feature = "power")]
pub mod power;
pub mod system;
#[cfg(feature = "tray")]
pub mod tray;
pub mod util;

pub use foreground::{
  ForegroundService, ForegroundServiceFactory, ForegroundServices, ForegroundServicesHandle,
  start_foreground,
};

pub fn start_background(store: &mut slowshell_core::Store, name: &str) -> bool {
  match name {
    "system" => {
      system::ensure(store, std::time::Duration::from_secs(5));
      true
    }
    #[cfg(feature = "audio")]
    "audio" | "sound" => {
      audio::ensure(store);
      true
    }
    #[cfg(feature = "bluetooth")]
    "bluetooth" => {
      bluetooth::ensure(store);
      true
    }
    #[cfg(feature = "network")]
    "network" => {
      network::ensure(store);
      true
    }
    #[cfg(feature = "power")]
    "power" | "brightness" => {
      power::ensure(store);
      true
    }
    #[cfg(feature = "tray")]
    "tray" => {
      tray::ensure(store);
      true
    }
    #[cfg(feature = "notifications")]
    "notifications" => {
      notifications::ensure(store);
      true
    }
    // TODO: Plugin services
    _ => false,
  }
}

use std::sync::OnceLock;

use tokio::runtime::Runtime;

pub fn shared_runtime() -> &'static Runtime {
  static RT: OnceLock<Runtime> = OnceLock::new();
  RT.get_or_init(|| {
    tokio::runtime::Builder::new_multi_thread()
      .worker_threads(1)
      .thread_stack_size(256 * 1024)
      .thread_name("slowshell-svc")
      .enable_all()
      .build()
      .expect("shared tokio runtime")
  })
}
