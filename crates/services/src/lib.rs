pub mod audio;
pub mod bluetooth;
pub mod network;
pub mod notifications;
pub mod power;
pub mod system;
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
