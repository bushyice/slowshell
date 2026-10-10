use std::sync::Mutex;

use slowshell_plugin_abi::{Context, StoreCallbackBox, TypeKey};

use crate::state::PluginScope;

struct Subscriber {
  key: TypeKey,
  plugin_id: String,
  callback: StoreCallbackBox,
}

fn subscribers() -> &'static Mutex<Vec<Subscriber>> {
  static SUBSCRIBERS: std::sync::OnceLock<Mutex<Vec<Subscriber>>> = std::sync::OnceLock::new();
  SUBSCRIBERS.get_or_init(|| Mutex::new(Vec::new()))
}

pub(crate) fn subscribe(key: TypeKey, plugin_id: String, callback: StoreCallbackBox) {
  subscribers().lock().unwrap().push(Subscriber {
    key,
    plugin_id,
    callback,
  });
}

pub(crate) fn notify(key: TypeKey) -> bool {
  let current = {
    let mut guard = subscribers().lock().unwrap();
    std::mem::take(&mut *guard)
  };

  let ctx = Context::new(crate::host::host());
  let mut notified = false;
  for subscriber in &current {
    if subscriber.key == key {
      let _scope = PluginScope::enter(&subscriber.plugin_id);
      subscriber.callback.notify(&ctx);
      notified = true;
    }
  }

  let mut guard = subscribers().lock().unwrap();
  let newly_added = std::mem::take(&mut *guard);
  guard.extend(current);
  guard.extend(newly_added);
  notified
}
