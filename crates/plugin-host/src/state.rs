use std::{
  cell::RefCell,
  collections::HashMap,
  sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
  },
};

use slowshell_commons::notifications::SharedNotificationState;
use slowshell_compositor::{CompositorState, CompositorStore};
use slowshell_config::Config;
use slowshell_core::Store;
use slowshell_core::listeners::Listeners;
use slowshell_plugin_abi::{
  ComponentFactoryBox, CompositorFactoryBox, ConfigParserBox, DesktopItemFactoryBox,
  PayloadFactoryBox, PayloadParserBox, QuickSettingsFactoryBox, RenderableFactoryBox,
  SpotlightFactoryBox,
};

#[cfg(feature = "panels")]
use slowshell_components::ComponentOptions;

pub(crate) fn plugin_ids() -> &'static Mutex<Vec<String>> {
  static IDS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
  IDS.get_or_init(|| Mutex::new(Vec::new()))
}

#[derive(Clone)]
pub struct PluginInfo {
  pub id: String,
  pub version: String,
  pub requires: Vec<String>,
  pub path: std::path::PathBuf,
}

pub(crate) fn plugin_infos() -> &'static Mutex<Vec<PluginInfo>> {
  static INFOS: OnceLock<Mutex<Vec<PluginInfo>>> = OnceLock::new();
  INFOS.get_or_init(|| Mutex::new(Vec::new()))
}

#[derive(Default, Clone)]
pub struct PluginContributions {
  pub components: Vec<String>,
  pub compositors: Vec<String>,
  pub payloads: Vec<String>,
  pub payload_parsers: Vec<String>,
  pub renderables: Vec<String>,
  pub desktop_items: Vec<String>,
  pub config_parsers: Vec<String>,
  pub spotlights: Vec<String>,
  pub commands: Vec<String>,
  pub styles: Vec<String>,
}

pub(crate) fn plugin_contributions() -> &'static Mutex<HashMap<String, PluginContributions>> {
  static CONTRIBUTIONS: OnceLock<Mutex<HashMap<String, PluginContributions>>> = OnceLock::new();
  CONTRIBUTIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn contribute(plugin_id: &str, update: impl FnOnce(&mut PluginContributions)) {
  let mut contributions = plugin_contributions().lock().unwrap();
  update(contributions.entry(plugin_id.to_owned()).or_default());
}

pub(crate) fn plugin_configs() -> &'static Mutex<HashMap<String, HashMap<String, String>>> {
  static CONFIGS: OnceLock<Mutex<HashMap<String, HashMap<String, String>>>> = OnceLock::new();
  CONFIGS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn plugin_config_parsers() -> &'static Mutex<Vec<(String, String, ConfigParserBox)>> {
  static PARSERS: OnceLock<Mutex<Vec<(String, String, ConfigParserBox)>>> = OnceLock::new();
  PARSERS.get_or_init(|| Mutex::new(Vec::new()))
}

pub(crate) fn plugin_errors() -> &'static Mutex<HashMap<String, (u32, String)>> {
  static ERRORS: OnceLock<Mutex<HashMap<String, (u32, String)>>> = OnceLock::new();
  ERRORS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn record_plugin_error(plugin_id: &str, code: u32, message: &str) {
  plugin_errors()
    .lock()
    .unwrap()
    .insert(plugin_id.to_owned(), (code, message.to_owned()));
  eprintln!("[plugin:{plugin_id}] error {code}: {message}");
}

static SILENT: AtomicBool = AtomicBool::new(false);

pub fn set_silent(silent: bool) {
  SILENT.store(silent, Ordering::Relaxed);
}

pub(crate) fn plugin_log(message: std::fmt::Arguments) {
  if !SILENT.load(Ordering::Relaxed) {
    eprintln!("{message}");
  }
}

pub(crate) fn plugin_persistence() -> &'static Mutex<HashMap<String, HashMap<String, String>>> {
  static STORE: OnceLock<Mutex<HashMap<String, HashMap<String, String>>>> = OnceLock::new();
  STORE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn persistence_file(plugin_id: &str) -> String {
  let sanitized: String = plugin_id
    .chars()
    .map(|c| {
      if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
        c
      } else {
        '_'
      }
    })
    .collect();

  format!("plugin-{sanitized}")
}

pub(crate) fn with_plugin_persistence<R>(
  plugin_id: &str,
  f: impl FnOnce(&mut HashMap<String, String>) -> R,
) -> R {
  let mut store = plugin_persistence().lock().unwrap();
  let map = store.entry(plugin_id.to_owned()).or_insert_with(|| {
    slowshell_core::persistence::load(persistence_file(plugin_id)).unwrap_or_default()
  });

  f(map)
}

pub(crate) fn flush_plugin_persistence(plugin_id: &str, map: &HashMap<String, String>) {
  if let Err(e) = slowshell_core::persistence::save(persistence_file(plugin_id), map) {
    eprintln!("[plugin:{plugin_id}] failed to persist state: {e}");
  }
}

thread_local! {
  static CURRENT_PLUGIN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

pub(crate) struct PluginScope {
  plugin_id: String,
}

impl PluginScope {
  pub(crate) fn enter(plugin_id: &str) -> Self {
    CURRENT_PLUGIN.with(|cell| cell.borrow_mut().push(plugin_id.to_owned()));
    Self {
      plugin_id: plugin_id.to_owned(),
    }
  }
}

impl Drop for PluginScope {
  fn drop(&mut self) {
    CURRENT_PLUGIN.with(|cell| {
      let mut stack = cell.borrow_mut();
      if stack.last().is_some_and(|id| id == &self.plugin_id) {
        stack.pop();
      }
    });
  }
}

pub(crate) fn current_plugin() -> Option<String> {
  CURRENT_PLUGIN.with(|cell| cell.borrow().last().cloned())
}

pub(crate) enum Pending {
  Component {
    name: String,
    factory: ComponentFactoryBox,
  },
  Compositor {
    name: String,
    factory: CompositorFactoryBox,
  },
  Payload {
    command: String,
    factory: PayloadFactoryBox,
  },
  Renderable {
    name: String,
    factory: RenderableFactoryBox,
  },
  DesktopItem {
    name: String,
    factory: DesktopItemFactoryBox,
  },
  ConfigParser {
    name: String,
    parser: ConfigParserBox,
  },
  PayloadParser {
    commands: Vec<String>,
    parser: PayloadParserBox,
  },
  Spotlight {
    name: String,
    factory: SpotlightFactoryBox,
  },
  QuickSettings {
    name: String,
    factory: QuickSettingsFactoryBox,
  },
  Command {
    name: String,
    title: Option<String>,
    description: Option<String>,
  },
}

thread_local! {
  pub(crate) static PENDING: RefCell<Vec<Pending>> = const { RefCell::new(Vec::new()) };
}

#[derive(Default)]
pub(crate) struct PluginCalls {
  pub timers: Vec<(u32, bool, u32)>,
  pub fds: Vec<(i32, u32)>,
  pub redraw: bool,
}

thread_local! {
  static CURRENT_CALLS: RefCell<Option<PluginCalls>> = const { RefCell::new(None) };
}

pub(crate) fn with_plugin_calls<R>(f: impl FnOnce() -> R) -> (R, PluginCalls) {
  CURRENT_CALLS.with(|calls| *calls.borrow_mut() = Some(PluginCalls::default()));
  let result = f();
  let calls = CURRENT_CALLS
    .with(|calls| calls.borrow_mut().take())
    .unwrap_or_default();
  (result, calls)
}

pub(crate) fn record_timer(millis: u32, repeating: bool, tag: u32) {
  CURRENT_CALLS.with(|calls| {
    if let Some(calls) = calls.borrow_mut().as_mut() {
      calls.timers.push((millis, repeating, tag));
    }
  });
}

pub(crate) fn record_fd(fd: i32, action: u32) {
  CURRENT_CALLS.with(|calls| {
    if let Some(calls) = calls.borrow_mut().as_mut() {
      calls.fds.push((fd, action));
    }
  });
}

pub(crate) fn record_redraw() {
  CURRENT_CALLS.with(|calls| {
    if let Some(calls) = calls.borrow_mut().as_mut() {
      calls.redraw = true;
    }
  });
}

thread_local! {
  static CURRENT_CONFIG: RefCell<Option<*const Config>> = const { RefCell::new(None) };
  static CURRENT_COMPOSITOR: RefCell<Option<*const CompositorState>> = const { RefCell::new(None) };
  static CURRENT_COMPOSITOR_STATE: RefCell<Option<*mut CompositorState>> = const { RefCell::new(None) };
  static CURRENT_NOTIFICATIONS: RefCell<Option<SharedNotificationState>> = const { RefCell::new(None) };
  static CURRENT_LISTENERS: RefCell<Option<*mut Listeners>> = const { RefCell::new(None) };
  static CURRENT_STORE: RefCell<Option<*mut Store>> = const { RefCell::new(None) };
  #[cfg(feature = "panels")]
  static CURRENT_OPTIONS: RefCell<Option<*const ComponentOptions>> = const { RefCell::new(None) };
}

pub(crate) struct ScopedConfig {
  previous: Option<*const Config>,
}

impl ScopedConfig {
  pub(crate) fn current(config: &Config) -> Self {
    let previous = CURRENT_CONFIG.with(|cell| cell.borrow_mut().replace(config as *const Config));
    Self { previous }
  }
}

impl Drop for ScopedConfig {
  fn drop(&mut self) {
    CURRENT_CONFIG.with(|cell| *cell.borrow_mut() = self.previous);
  }
}

pub(crate) fn current_config() -> Option<&'static Config> {
  CURRENT_CONFIG
    .with(|cell| *cell.borrow())
    .map(|ptr| unsafe { &*ptr })
}

pub(crate) struct ScopedCompositor {
  previous: Option<*const CompositorState>,
}

impl ScopedCompositor {
  pub(crate) fn current(state: Option<&CompositorState>) -> Self {
    let pointer = state.map(|state| state as *const CompositorState);
    let previous =
      CURRENT_COMPOSITOR.with(|cell| std::mem::replace(&mut *cell.borrow_mut(), pointer));
    Self { previous }
  }
}

impl Drop for ScopedCompositor {
  fn drop(&mut self) {
    CURRENT_COMPOSITOR.with(|cell| *cell.borrow_mut() = self.previous);
  }
}

pub(crate) fn compositor_scope(store: &Store) -> ScopedCompositor {
  let state = store
    .borrow::<CompositorStore>()
    .and_then(|store| store.state().ok());
  ScopedCompositor::current(state)
}

pub(crate) fn current_compositor() -> Option<&'static CompositorState> {
  CURRENT_COMPOSITOR
    .with(|cell| *cell.borrow())
    .map(|ptr| unsafe { &*ptr })
}

pub(crate) struct ScopedCompositorState {
  previous: Option<*mut CompositorState>,
}

impl ScopedCompositorState {
  pub(crate) fn current(state: &mut CompositorState) -> Self {
    let previous = CURRENT_COMPOSITOR_STATE
      .with(|cell| cell.borrow_mut().replace(state as *mut CompositorState));
    Self { previous }
  }
}

impl Drop for ScopedCompositorState {
  fn drop(&mut self) {
    CURRENT_COMPOSITOR_STATE.with(|cell| *cell.borrow_mut() = self.previous);
  }
}

pub(crate) fn set_current_compositor_state(state: CompositorState) {
  CURRENT_COMPOSITOR_STATE.with(|cell| {
    if let Some(ptr) = *cell.borrow() {
      unsafe { *ptr = state };
    }
  });
}

pub(crate) struct ScopedNotifications {
  previous: Option<SharedNotificationState>,
}

impl ScopedNotifications {
  pub(crate) fn current(shared: Option<SharedNotificationState>) -> Self {
    let previous =
      CURRENT_NOTIFICATIONS.with(|cell| std::mem::replace(&mut *cell.borrow_mut(), shared));
    Self { previous }
  }
}

impl Drop for ScopedNotifications {
  fn drop(&mut self) {
    CURRENT_NOTIFICATIONS.with(|cell| *cell.borrow_mut() = self.previous.take());
  }
}

pub(crate) fn notifications_scope(store: &Store) -> ScopedNotifications {
  ScopedNotifications::current(store.borrow::<SharedNotificationState>().cloned())
}

pub(crate) fn current_notifications() -> Option<SharedNotificationState> {
  CURRENT_NOTIFICATIONS.with(|cell| cell.borrow().clone())
}

pub(crate) struct ScopedListeners {
  previous: Option<*mut Listeners>,
}

impl ScopedListeners {
  pub(crate) fn current(listeners: &mut Listeners) -> Self {
    let previous = CURRENT_LISTENERS
      .with(|cell| std::mem::replace(&mut *cell.borrow_mut(), Some(listeners as *mut Listeners)));
    Self { previous }
  }
}

impl Drop for ScopedListeners {
  fn drop(&mut self) {
    CURRENT_LISTENERS.with(|cell| *cell.borrow_mut() = self.previous);
  }
}

pub(crate) fn current_listeners() -> Option<*mut Listeners> {
  CURRENT_LISTENERS.with(|cell| *cell.borrow())
}

pub(crate) struct ScopedStore {
  previous: Option<*mut Store>,
}

impl ScopedStore {
  pub(crate) fn current(store: *mut Store) -> Self {
    let previous =
      CURRENT_STORE.with(|cell| std::mem::replace(&mut *cell.borrow_mut(), Some(store)));
    Self { previous }
  }
}

impl Drop for ScopedStore {
  fn drop(&mut self) {
    CURRENT_STORE.with(|cell| *cell.borrow_mut() = self.previous);
  }
}

pub(crate) fn store_scope(store: &Store) -> ScopedStore {
  ScopedStore::current(store as *const Store as *mut Store)
}

pub(crate) fn store_scope_mut(store: &mut Store) -> ScopedStore {
  ScopedStore::current(store as *mut Store)
}

pub(crate) fn current_store() -> Option<&'static Store> {
  CURRENT_STORE
    .with(|cell| *cell.borrow())
    .map(|ptr| unsafe { &*ptr })
}

pub(crate) fn current_store_mut() -> Option<*mut Store> {
  CURRENT_STORE.with(|cell| *cell.borrow())
}

#[cfg(feature = "panels")]
pub(crate) struct ScopedOptions {
  previous: Option<*const ComponentOptions>,
}

#[cfg(feature = "panels")]
impl ScopedOptions {
  pub(crate) fn current(options: Option<&ComponentOptions>) -> Self {
    let pointer = options.map(|options| options as *const ComponentOptions);
    let previous = CURRENT_OPTIONS.with(|cell| std::mem::replace(&mut *cell.borrow_mut(), pointer));
    Self { previous }
  }
}

#[cfg(feature = "panels")]
impl Drop for ScopedOptions {
  fn drop(&mut self) {
    CURRENT_OPTIONS.with(|cell| *cell.borrow_mut() = self.previous);
  }
}

#[cfg(feature = "panels")]
pub(crate) fn current_options() -> Option<&'static ComponentOptions> {
  CURRENT_OPTIONS
    .with(|cell| *cell.borrow())
    .map(|ptr| unsafe { &*ptr })
}

pub(crate) struct CanvasPool {
  buffers: Vec<(Vec<u8>, usize)>,
  in_use: usize,
}

thread_local! {
  static CANVAS_POOL: RefCell<CanvasPool> = const {
    RefCell::new(CanvasPool {
      buffers: Vec::new(),
      in_use: 0,
    })
  };
}

pub(crate) fn reset_canvas_pool() {
  CANVAS_POOL.with(|pool| {
    pool.borrow_mut().in_use = 0;
  });
}

pub(crate) fn canvas_alloc(width: u32, height: u32) -> Option<(*mut u8, usize, u64)> {
  let len = width as usize * height as usize * 4;
  if len == 0 {
    return None;
  }

  CANVAS_POOL.with(|pool| {
    let mut pool = pool.borrow_mut();
    let index = pool.in_use;
    pool.in_use += 1;

    if index < pool.buffers.len() {
      let (buffer, buffer_len) = &mut pool.buffers[index];
      if buffer.len() < len {
        buffer.resize(len, 0);
      }
      *buffer_len = len;
      let ptr = buffer.as_mut_ptr();
      Some((ptr, len, index as u64 + 1))
    } else {
      let mut buffer = vec![0u8; len];
      let ptr = buffer.as_mut_ptr();
      pool.buffers.push((buffer, len));
      Some((ptr, len, index as u64 + 1))
    }
  })
}

pub(crate) fn canvas_with<R>(id: u64, f: impl FnOnce(&[u8]) -> R) -> Option<R> {
  if id == 0 {
    return None;
  }

  CANVAS_POOL.with(|pool| {
    let pool = pool.borrow();
    let (buffer, len) = pool.buffers.get(id as usize - 1)?;
    Some(f(&buffer[..*len]))
  })
}
