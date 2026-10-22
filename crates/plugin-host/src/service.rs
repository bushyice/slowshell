use std::{
  cell::RefCell,
  collections::HashMap,
  os::fd::{AsRawFd, OwnedFd},
  sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
  },
  time::{Duration, Instant},
};

use slowshell_config::Config;
use slowshell_core::{
  ActionDispatcher, Store,
  listeners::{FdHandle, ListenerAction},
  types::{PayloadBuilderRegistry, Ustr},
};
use slowshell_plugin_abi::{Context, ServiceContext, ServiceFactoryBox, ServiceObjectBox};

use crate::state::{PluginScope, ScopedConfig, record_plugin_error, store_scope_mut};

pub(crate) const BUILTINS: &[&str] = &[
  "network",
  "audio",
  "bluetooth",
  "power",
  "system",
  "tray",
  "notifications",
];

pub(crate) fn event_name(name: &str) -> Ustr {
  format!("service/{name}").into()
}

struct Entry {
  plugin_id: String,
  factory: Arc<ServiceFactoryBox>,
  autostart: bool,
}

fn entries() -> &'static Mutex<HashMap<String, Entry>> {
  static ENTRIES: OnceLock<Mutex<HashMap<String, Entry>>> = OnceLock::new();
  ENTRIES.get_or_init(|| Mutex::new(HashMap::new()))
}

struct Running {
  stop: Arc<AtomicBool>,
  tx: std::sync::mpsc::Sender<String>,
}

fn running() -> &'static Mutex<HashMap<String, Running>> {
  static RUNNING: OnceLock<Mutex<HashMap<String, Running>>> = OnceLock::new();
  RUNNING.get_or_init(|| Mutex::new(HashMap::new()))
}

struct ThreadState {
  stop: Arc<AtomicBool>,
  wake: Option<OwnedFd>,
  rx: std::sync::mpsc::Receiver<String>,
  dispatcher: Option<ActionDispatcher>,
  builders: Option<Arc<PayloadBuilderRegistry>>,
}

thread_local! {
  static CURRENT: RefCell<Option<ThreadState>> = const { RefCell::new(None) };
}

pub(crate) fn register(plugin_id: &str, name: String, factory: ServiceFactoryBox, autostart: bool) {
  if name.is_empty() {
    record_plugin_error(plugin_id, 2, "service name must not be empty");
    return;
  }
  if BUILTINS.contains(&name.as_str()) {
    record_plugin_error(
      plugin_id,
      2,
      &format!("service name `{name}` is reserved by a built-in service"),
    );
    return;
  }

  let mut map = entries().lock().unwrap();
  if map.contains_key(&name) {
    record_plugin_error(
      plugin_id,
      2,
      &format!("service `{name}` is already registered"),
    );
    return;
  }

  map.insert(
    name.clone(),
    Entry {
      plugin_id: plugin_id.to_owned(),
      factory: Arc::new(factory),
      autostart,
    },
  );
  drop(map);

  crate::state::contribute(plugin_id, |contributions| {
    contributions.services.push(name);
  });
}

pub(crate) fn start(name: &str) -> bool {
  if start_builtin(name) {
    return true;
  }

  if is_running(name) {
    return true;
  }

  let (plugin_id, factory) = {
    let map = entries().lock().unwrap();
    match map.get(name) {
      Some(entry) => (entry.plugin_id.clone(), entry.factory.clone()),
      None => return false,
    }
  };

  let stop = Arc::new(AtomicBool::new(false));
  let (tx, rx) = std::sync::mpsc::channel::<String>();

  let (wake, dispatcher, builders) = {
    let store = crate::state::current_store();
    let dispatcher = store
      .and_then(|store| store.borrow::<ActionDispatcher>())
      .map(|dispatcher| ActionDispatcher(dispatcher.0.clone()));
    let builders = store
      .and_then(|store| store.borrow::<Arc<PayloadBuilderRegistry>>())
      .cloned();
    (watch_wake(name, store), dispatcher, builders)
  };

  let mut inner: ServiceObjectBox = {
    let _scope = PluginScope::enter(&plugin_id);
    let ctx = Context::new(crate::host::host());
    factory.create(&ctx)
  };

  running().lock().unwrap().insert(
    name.to_owned(),
    Running {
      stop: stop.clone(),
      tx,
    },
  );

  let thread_name = name.to_owned();
  let thread_stop = stop.clone();
  let spawn = std::thread::Builder::new()
    .name(format!("slowshell-service-{name}"))
    .spawn(move || {
      let _scope = PluginScope::enter(&plugin_id);
      CURRENT.with(|cell| {
        *cell.borrow_mut() = Some(ThreadState {
          stop: thread_stop.clone(),
          wake,
          rx,
          dispatcher,
          builders,
        })
      });

      let ctx = ServiceContext::new(crate::host::host());
      inner.run(&ctx);

      CURRENT.with(|cell| {
        *cell.borrow_mut() = None;
      });

      let mut map = running().lock().unwrap();
      if map
        .get(&thread_name)
        .is_some_and(|entry| Arc::ptr_eq(&entry.stop, &thread_stop))
      {
        map.remove(&thread_name);
      }
    });

  if spawn.is_err() {
    let mut map = running().lock().unwrap();
    if map
      .get(name)
      .is_some_and(|entry| Arc::ptr_eq(&entry.stop, &stop))
    {
      map.remove(name);
    }
    return false;
  }

  true
}

fn watch_wake(name: &str, store: Option<&Store>) -> Option<OwnedFd> {
  use nix::{
    fcntl::{FcntlArg, OFlag, fcntl},
    sys::epoll::EpollFlags,
  };

  let handle = store?.borrow::<FdHandle>()?;
  let (read_fd, write_fd) = nix::unistd::pipe().ok()?;
  fcntl(&read_fd, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).ok()?;

  let raw = read_fd.as_raw_fd();
  handle.watch_with_flags(
    read_fd,
    EpollFlags::EPOLLIN | EpollFlags::EPOLLET,
    ListenerAction::Signal {
      name: event_name(name),
      fd: raw,
    },
  );

  Some(write_fd)
}

pub(crate) fn is_running(name: &str) -> bool {
  if running().lock().unwrap().contains_key(name) {
    return true;
  }
  builtin_running(name)
}

pub(crate) fn stop(name: &str) -> bool {
  let map = running().lock().unwrap();
  match map.get(name) {
    Some(entry) => {
      entry.stop.store(true, Ordering::SeqCst);
      true
    }
    None => false,
  }
}

pub(crate) fn send(name: &str, message: &str) -> bool {
  let map = running().lock().unwrap();
  match map.get(name) {
    Some(entry) => entry.tx.send(message.to_owned()).is_ok(),
    None => false,
  }
}

pub(crate) fn notify_current() -> bool {
  CURRENT.with(|cell| {
    let cell = cell.borrow();
    match cell.as_ref().and_then(|state| state.wake.as_ref()) {
      Some(wake) => {
        slowshell_services::util::notify_signal(wake);
        true
      }
      None => false,
    }
  })
}

pub(crate) fn sleep(millis: u64) -> bool {
  CURRENT.with(|cell| {
    let cell = cell.borrow();
    let Some(state) = cell.as_ref() else {
      return false;
    };

    let deadline = Instant::now() + Duration::from_millis(millis);
    loop {
      if state.stop.load(Ordering::Relaxed) {
        return false;
      }
      let now = Instant::now();
      if now >= deadline {
        return true;
      }
      std::thread::sleep((deadline - now).min(Duration::from_millis(50)));
    }
  })
}

pub(crate) fn stopping() -> bool {
  CURRENT.with(|cell| {
    cell
      .borrow()
      .as_ref()
      .map(|state| state.stop.load(Ordering::Relaxed))
      .unwrap_or(true)
  })
}

pub(crate) fn recv(timeout_millis: u32) -> Option<String> {
  CURRENT.with(|cell| {
    let mut cell = cell.borrow_mut();
    let state = cell.as_mut()?;
    state
      .rx
      .recv_timeout(Duration::from_millis(timeout_millis as u64))
      .ok()
  })
}

pub(crate) fn current_dispatcher() -> Option<ActionDispatcher> {
  CURRENT.with(|cell| {
    cell
      .borrow()
      .as_ref()
      .and_then(|state| state.dispatcher.clone())
  })
}

pub(crate) fn current_builders() -> Option<Arc<PayloadBuilderRegistry>> {
  CURRENT.with(|cell| {
    cell
      .borrow()
      .as_ref()
      .and_then(|state| state.builders.clone())
  })
}

pub(crate) fn start_autostart(config: &Config, store: &mut Store) {
  let _config_scope = ScopedConfig::current(config);
  let _store_scope = store_scope_mut(store);

  let names: Vec<String> = entries()
    .lock()
    .unwrap()
    .iter()
    .filter(|(_, entry)| entry.autostart)
    .map(|(name, _)| name.clone())
    .collect();

  for name in names {
    start(&name);
  }
}

fn start_builtin(name: &str) -> bool {
  let Some(store) = crate::state::current_store_mut() else {
    return false;
  };
  let store = unsafe { &mut *store };

  match name {
    "system" => {
      slowshell_services::system::ensure(store, Duration::from_secs(5));
    }
    #[cfg(feature = "service-audio")]
    "audio" => {
      slowshell_services::audio::ensure(store);
    }
    #[cfg(feature = "service-bluetooth")]
    "bluetooth" => {
      slowshell_services::bluetooth::ensure(store);
    }
    #[cfg(feature = "service-network")]
    "network" => {
      slowshell_services::network::ensure(store);
    }
    #[cfg(feature = "service-power")]
    "power" => {
      slowshell_services::power::ensure(store);
    }
    #[cfg(feature = "service-tray")]
    "tray" => {
      slowshell_services::tray::ensure(store);
    }
    #[cfg(feature = "service-notifications")]
    "notifications" => {
      slowshell_services::notifications::ensure(store);
    }
    _ => return false,
  }

  builtin_running(name)
}

fn builtin_running(name: &str) -> bool {
  match name {
    "system" => slowshell_services::system::running(),
    #[cfg(feature = "service-audio")]
    "audio" => slowshell_services::audio::running(),
    #[cfg(feature = "service-bluetooth")]
    "bluetooth" => slowshell_services::bluetooth::running(),
    #[cfg(feature = "service-network")]
    "network" => slowshell_services::network::running(),
    #[cfg(feature = "service-power")]
    "power" => slowshell_services::power::running(),
    #[cfg(feature = "service-tray")]
    "tray" => slowshell_services::tray::running(),
    #[cfg(feature = "service-notifications")]
    "notifications" => slowshell_services::notifications::running(),
    _ => false,
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use slowshell_plugin_abi::{Service, service_factory};

  struct Svc;

  impl Service for Svc {
    fn new(_ctx: &Context<'_>) -> Self {
      Svc
    }

    fn run(&mut self, ctx: &ServiceContext<'_>) {
      while ctx.sleep(1) {}
    }
  }

  #[test]
  fn rejects_reserved_names() {
    register(
      "test-plugin",
      "network".to_owned(),
      service_factory::<Svc>(),
      false,
    );
    assert!(!entries().lock().unwrap().contains_key("network"));
  }

  #[test]
  fn rejects_duplicate_names() {
    register(
      "test-plugin",
      "test/dup".to_owned(),
      service_factory::<Svc>(),
      true,
    );
    register(
      "other-plugin",
      "test/dup".to_owned(),
      service_factory::<Svc>(),
      false,
    );

    let map = entries().lock().unwrap();
    assert!(map.contains_key("test/dup"));
    assert!(map.get("test/dup").unwrap().autostart);
  }

  #[test]
  fn starts_messages_and_stops() {
    register(
      "test-plugin",
      "test/lifecycle".to_owned(),
      service_factory::<Svc>(),
      false,
    );

    assert!(start("test/lifecycle"));
    assert!(is_running("test/lifecycle"));
    assert!(send("test/lifecycle", "hello"));
    assert!(stop("test/lifecycle"));

    let deadline = Instant::now() + Duration::from_secs(5);
    while is_running("test/lifecycle") && Instant::now() < deadline {
      std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!is_running("test/lifecycle"));
  }

  #[test]
  fn unknown_names_do_not_start() {
    assert!(!start("test/missing"));
    assert!(!is_running("test/missing"));
  }
}
