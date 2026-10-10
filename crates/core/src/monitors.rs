use std::{
  collections::HashMap,
  sync::{OnceLock, RwLock},
};

use crate::types::{ToUstr, Ustr};

// TODO: Move into store
static MONITORS: OnceLock<RwLock<HashMap<Ustr, (u32, u32)>>> = OnceLock::new();

fn cache() -> &'static RwLock<HashMap<Ustr, (u32, u32)>> {
  MONITORS.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn set(monitors: impl IntoIterator<Item = (Ustr, u32, u32)>) {
  let Ok(mut guard) = cache().write() else {
    return;
  };

  for (name, width, height) in monitors {
    if width > 0 && height > 0 {
      guard.insert(name, (width, height));
    }
  }
}

pub fn size(name: &str) -> Option<(u32, u32)> {
  cache().read().ok()?.get(&name.to_ustr()).copied()
}
