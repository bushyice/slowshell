use std::{
  collections::HashMap,
  sync::{Arc, Mutex, OnceLock},
};

pub const NOTCH_STATUS_CHANGED: &str = "notch/status.changed";

#[derive(Debug, Clone, PartialEq)]
pub enum NotchStatus {
  Icon(String),
  Text(String),
  IconText(String, String),
}

#[derive(Debug, Default, Clone)]
pub struct NotchStatusStore {
  pub items: Arc<Mutex<HashMap<String, NotchStatus>>>,
}

impl NotchStatusStore {
  pub fn new() -> Self {
    Self {
      items: Arc::new(Mutex::new(HashMap::new())),
    }
  }

  pub fn update_status(&self, name: impl Into<String>, status: NotchStatus) {
    if let Ok(mut items) = self.items.lock() {
      items.insert(name.into(), status);
    }
  }

  pub fn remove_status(&self, name: &str) {
    if let Ok(mut items) = self.items.lock() {
      items.remove(name);
    }
  }

  pub fn get(&self, name: &str) -> Option<NotchStatus> {
    self.items.lock().ok()?.get(name).cloned()
  }

  pub fn all(&self) -> HashMap<String, NotchStatus> {
    self.items.lock().map(|m| m.clone()).unwrap_or_default()
  }
}

pub type SharedNotchStatus = Arc<NotchStatusStore>;

pub fn shared() -> SharedNotchStatus {
  static SHARED: OnceLock<SharedNotchStatus> = OnceLock::new();
  SHARED
    .get_or_init(|| Arc::new(NotchStatusStore::new()))
    .clone()
}
