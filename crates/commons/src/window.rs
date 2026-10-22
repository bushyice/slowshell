use slowshell_core::Window;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug, Clone, Default)]
pub struct LastActiveWindowStore {
  pub window: Arc<Mutex<Option<Window>>>,
}

impl LastActiveWindowStore {
  pub fn set(&self, window: Window) {
    if let Ok(mut lock) = self.window.lock() {
      *lock = Some(window);
    }
  }

  pub fn get(&self) -> Option<Window> {
    self.window.lock().ok()?.clone()
  }

  pub fn clear(&self) {
    if let Ok(mut lock) = self.window.lock() {
      *lock = None;
    }
  }
}

pub fn last_active_window() -> &'static LastActiveWindowStore {
  static STORE: OnceLock<LastActiveWindowStore> = OnceLock::new();
  STORE.get_or_init(LastActiveWindowStore::default)
}
