use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);

pub fn enabled() -> bool {
  ENABLED.load(Ordering::Relaxed)
}

pub fn set_enabled(enabled: bool) {
  ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn configure(configured: Option<bool>, feature_enabled: bool) -> bool {
  let enabled = feature_enabled && configured.unwrap_or(true);
  set_enabled(enabled);
  enabled
}
