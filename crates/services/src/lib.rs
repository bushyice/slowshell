#[cfg(feature = "audio")]
pub mod audio;
#[cfg(feature = "bluetooth")]
pub mod bluetooth;
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
