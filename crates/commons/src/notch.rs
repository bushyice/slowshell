use std::{
  collections::HashMap,
  sync::{Arc, Mutex, OnceLock},
};

use slowshell_core::Store;

pub const NOTCH_STATUS_CHANGED: &str = "notch/status.changed";

#[derive(Debug, Clone, PartialEq)]
pub enum NotchStatus {
  Icon(String),
  Text(String),
  IconText(String, String),
}

pub trait NotchServiceStarter: Send {
  fn start(&mut self, store: &mut Store);
  fn stop(&self, store: &Store) {
    let _ = store;
  }
}

#[derive(Default, Clone)]
pub struct NotchStatusStore {
  pub items: Arc<Mutex<HashMap<String, NotchStatus>>>,
  pub services: Arc<Mutex<HashMap<String, Box<dyn NotchServiceStarter>>>>,
}

impl NotchStatusStore {
  pub fn new() -> Self {
    Self {
      items: Arc::new(Mutex::new(HashMap::new())),
      services: Arc::new(Mutex::new(HashMap::new())),
    }
  }

  pub fn service<T: NotchServiceStarter + 'static>(&self, name: &str, starter: T) {
    if let Ok(mut services) = self.services.lock() {
      services.insert(name.into(), Box::new(starter));
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

  pub fn with_service<R>(
    &self,
    name: &str,
    start: impl FnOnce(&mut (dyn NotchServiceStarter + 'static)) -> R,
  ) -> Option<R> {
    let mut guard = self.services.lock().ok()?;
    let mut service = guard.remove(name)?;
    drop(guard);
    let result = start(&mut *service);

    if let Ok(mut services) = self.services.lock() {
      services.entry(name.to_owned()).or_insert(service);
    }
    Some(result)
  }

  pub fn with_services(
    &self,
    mut start: impl FnMut(&str, &mut (dyn NotchServiceStarter + 'static)),
  ) {
    let Ok(mut services) = self.services.lock() else {
      return;
    };
    let mut taken: Vec<(String, Box<dyn NotchServiceStarter + 'static>)> =
      services.drain().collect();
    drop(services);

    for (name, service) in taken.iter_mut() {
      start(name, &mut **service);
    }

    if let Ok(mut services) = self.services.lock() {
      services.extend(taken);
    }
  }
}

pub type SharedNotchStatus = Arc<NotchStatusStore>;

pub fn shared() -> SharedNotchStatus {
  static SHARED: OnceLock<SharedNotchStatus> = OnceLock::new();
  SHARED
    .get_or_init(|| Arc::new(NotchStatusStore::new()))
    .clone()
}
