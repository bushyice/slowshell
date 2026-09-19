use std::{
  any::TypeId,
  cell::RefCell,
  collections::{HashMap, HashSet},
  ffi::c_void,
  os::fd::{AsRawFd, FromRawFd, OwnedFd},
  path::Path,
  path::PathBuf,
  sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
  },
  time::Duration,
};

use iced::{
  Alignment, Color, Length, Vector,
  widget::{Space, column, container, image as image_widget, row, text},
};
use iced_layershell::reexport::{
  Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use libloading::{Library, Symbol};
use miette::IntoDiagnostic;
use nix::sys::epoll::EpollFlags;
use slowshell_commons::audio::SharedAudioState;
use slowshell_commons::bluetooth::SharedBluetoothState;
use slowshell_commons::network::SharedNetworkState;
use slowshell_commons::notifications::{
  NotificationCmd, NotificationImage, NotificationItem, SharedNotificationState,
};
use slowshell_commons::popups::PopupSettings;
use slowshell_commons::power::SharedPowerState;
use slowshell_commons::system::SharedSystemState;
use slowshell_commons::tray::SharedTrayState;
#[cfg(feature = "panels")]
use slowshell_components::{
  Component, ComponentContext, ComponentFactory, ComponentOptions, ComponentRegistration,
  spaced_component,
};
use slowshell_compositor::{
  Compositor, CompositorCommand, CompositorFactory, CompositorRegistration, CompositorState,
  CompositorStore,
};
use slowshell_config::{
  Config, ConfigParser, bool_arg, find_node, float_arg, int_arg, node_children, node_name, str_arg,
  style::{ColorValue, Style, StyleValue, Theme, new_default_style},
};
use slowshell_core::{
  ActionDispatcher, Monitor, Store, Window, Workspace,
  commands::CommandEntry,
  listeners::{FdHandle, ListenerAction, Listeners},
  message::{EventFilter, ItemEffect, ItemMessage},
  types::{PayloadBox, PayloadBuilder, PayloadBuilderArgs, PayloadBuilderRegistry, Ustr, Void},
};
use slowshell_desktop::MonitorScope;
use slowshell_desktop::{DesktopItem, UpdateWhen, Visibility};
use slowshell_plugin::{
  SL_PLUGIN_ABI_VERSION, SL_PLUGIN_INIT_SYMBOL, SL_PLUGIN_META_SYMBOL, SlAccessPoint, SlAnimation,
  SlAudioSink, SlAudioState, SlBluetoothDevice, SlBluetoothState, SlCanvas, SlColor,
  SlComponentVtable, SlCompositorState, SlCompositorVtable, SlConfigParserFn, SlConnectionInfo,
  SlDesktopItemVtable, SlDesktopSettings, SlEffect, SlEthernet, SlEvent, SlHostApi, SlItemMessage,
  SlLength, SlMonitor, SlMprisPlayer, SlNetworkState, SlNode, SlNodeList, SlNotification,
  SlPayloadArgs, SlPayloadVtable, SlPluginGetMetaFn, SlPluginInitFn, SlPowerState, SlProcessInfo,
  SlRegistryNotifyFn, SlRenderableSettings, SlRenderableVtable, SlSpotlightVtable, SlStr,
  SlStyleSheet, SlStyleSheetEntry, SlSystemState, SlTheme, SlTrayItem, SlTrayState, SlWindow,
  SlWorkspace, sl_align, sl_anchor, sl_animation_easing, sl_animation_kind, sl_effect, sl_epoll,
  sl_event_kind, sl_event_mask, sl_image_source, sl_keyboard_interactivity, sl_layer,
  sl_length_unit, sl_message_kind, sl_node_kind, sl_style_color_kind, sl_style_value_kind,
  sl_update_when, sl_visibility,
};
#[cfg(feature = "spotlight")]
use slowshell_plugin::{SlSpotlightList, sl_display_style};
use slowshell_registry::{GlobalRegistry, ResourceRegistration};
#[cfg(feature = "spotlight")]
use slowshell_spotlight::{
  DisplayStyle, SpotlightAction, SpotlightActionDef, SpotlightItem, SpotlightKind, SpotlightMode,
  SpotlightModes,
};
use slowshell_widgets::{
  Animated, Backdrop, Easing, Icon, Mode, Renderable, Renderables, SizedPopup, SlideIn, clickable,
};

#[derive(Default, Clone)]
pub struct PluginSelection {
  pub enabled: Vec<String>,
  pub disabled: Vec<String>,
}

impl PluginSelection {
  pub fn from_path(path: Option<&Path>) -> Self {
    let Some(path) = path else {
      return Self::default();
    };
    let Ok(content) = std::fs::read_to_string(path) else {
      return Self::default();
    };
    let Ok(document) = content.parse::<kdl::KdlDocument>() else {
      return Self::default();
    };
    let Some(node) = find_node(document.nodes(), "plugins") else {
      return Self::default();
    };

    let mut selection = Self::default();
    for child in node_children(node) {
      let values: Vec<String> = child
        .entries()
        .iter()
        .filter_map(|entry| entry.value().as_string().map(str::to_owned))
        .collect();

      match node_name(child) {
        "enabled" => selection.enabled.extend(values),
        "disabled" => selection.disabled.extend(values),
        _ => {}
      }
    }
    selection
  }

  fn allows(&self, id: &str) -> bool {
    if self.disabled.iter().any(|disabled| disabled == id) {
      return false;
    }
    if self.enabled.is_empty() {
      return true;
    }
    self.enabled.iter().any(|enabled| enabled == id)
  }
}

pub struct PluginHost {
  #[allow(dead_code)]
  libraries: Vec<Library>,
}

impl PluginHost {
  pub fn load(registry: &mut GlobalRegistry, selection: &PluginSelection) -> Self {
    let mut libraries = Vec::new();

    for dir in plugin_dirs() {
      let Ok(entries) = std::fs::read_dir(&dir) else {
        continue;
      };

      let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
      paths.sort();

      for path in paths {
        if path.extension().and_then(|ext| ext.to_str()) != Some("so") {
          continue;
        }

        match unsafe { load_one(&path, registry, selection) } {
          Ok(Some(library)) => {
            plugin_log(format_args!("[plugin] loaded {}", path.display()));
            libraries.push(library);
          }
          Ok(None) => {}
          Err(e) => plugin_log(format_args!(
            "[plugin] failed to load {}: {e}",
            path.display()
          )),
        }
      }
    }

    let has_payloads = payloads()
      .lock()
      .map(|payloads| !payloads.is_empty())
      .unwrap_or(false);
    if has_payloads {
      registry.include_in("app", ResourceRegistration::Item(plugin_dispatcher_item));
    }

    let has_renderables = renderables()
      .lock()
      .map(|renderables| !renderables.is_empty())
      .unwrap_or(false);
    if has_renderables {
      registry.include_in(
        "app",
        ResourceRegistration::CustomConfig(register_plugin_renderables),
      );
    }

    let has_spotlights = spotlights()
      .lock()
      .map(|spotlights| !spotlights.is_empty())
      .unwrap_or(false);
    if has_spotlights {
      registry.include_in(
        "app",
        ResourceRegistration::CustomConfig(register_plugin_spotlights),
      );
    }

    let has_desktop_items = desktop_items()
      .lock()
      .map(|items| !items.is_empty())
      .unwrap_or(false);
    if has_desktop_items {
      registry.include_in("app", ResourceRegistration::Item(plugin_desktop_items));
    }

    let has_plugins = plugin_ids()
      .lock()
      .map(|ids| !ids.is_empty())
      .unwrap_or(false);
    if has_plugins {
      registry.include_in(
        "config",
        ResourceRegistration::Unknown(Box::new(ConfigParser {
          type_id: TypeId::of::<PluginConfigs>(),
          de: parse_plugin_configs,
        })),
      );
    }

    let payload_count = payloads().lock().map(|p| p.len()).unwrap_or(0);
    let renderable_count = renderables().lock().map(|r| r.len()).unwrap_or(0);
    let desktop_count = desktop_items().lock().map(|d| d.len()).unwrap_or(0);
    if payload_count + renderable_count + desktop_count > 0 {
      plugin_log(format_args!(
        "[plugin] registered {payload_count} payload(s), {renderable_count} renderable(s), {desktop_count} desktop item(s)"
      ));
    }

    Self { libraries }
  }
}

pub fn registered_surface_counts() -> (usize, usize, usize) {
  let payloads = payloads().lock().map(|p| p.len()).unwrap_or(0);
  let renderables = renderables().lock().map(|r| r.len()).unwrap_or(0);
  let desktop_items = desktop_items().lock().map(|d| d.len()).unwrap_or(0);
  (payloads, renderables, desktop_items)
}

pub fn registered_renderable_names() -> Vec<String> {
  renderables()
    .lock()
    .map(|renderables| {
      renderables
        .iter()
        .map(|entry| entry.name.to_string())
        .collect()
    })
    .unwrap_or_default()
}

unsafe fn load_one(
  path: &Path,
  registry: &mut GlobalRegistry,
  selection: &PluginSelection,
) -> miette::Result<Option<Library>> {
  let library = unsafe { Library::new(path) }.into_diagnostic()?;

  let meta_fn: Symbol<SlPluginGetMetaFn> = unsafe { library.get(SL_PLUGIN_META_SYMBOL.as_bytes()) }
    .map_err(|e| miette::miette!("missing {SL_PLUGIN_META_SYMBOL}: {e}"))?;
  let meta = unsafe { meta_fn() };

  if meta.abi_version != SL_PLUGIN_ABI_VERSION {
    return Err(miette::miette!(
      "ABI mismatch: plugin {} host {}",
      meta.abi_version,
      SL_PLUGIN_ABI_VERSION
    ));
  }

  let id = unsafe { meta.id.as_str() }
    .unwrap_or("<unknown>")
    .to_owned();

  if !selection.allows(&id) {
    plugin_log(format_args!("[plugin] {id} disabled"));
    return Ok(None);
  }

  plugin_ids().lock().unwrap().push(id.clone());

  let version = unsafe { meta.version.as_str() }
    .unwrap_or("<unknown>")
    .to_owned();
  plugin_infos().lock().unwrap().push(PluginInfo {
    id: id.clone(),
    version,
    path: path.to_path_buf(),
  });

  let init_fn: Symbol<SlPluginInitFn> = unsafe { library.get(SL_PLUGIN_INIT_SYMBOL.as_bytes()) }
    .map_err(|e| miette::miette!("missing {SL_PLUGIN_INIT_SYMBOL}: {e}"))?;

  PENDING.with(|pending| pending.borrow_mut().clear());
  CURRENT_PLUGIN.with(|current| *current.borrow_mut() = Some(id.clone()));

  let mut userdata: *mut c_void = std::ptr::null_mut();
  let status = unsafe { init_fn(&HOST_API, std::ptr::null_mut(), &mut userdata) };

  CURRENT_PLUGIN.with(|current| *current.borrow_mut() = None);

  if status != 0 {
    return Err(miette::miette!("[plugin] {id} init returned {status}"));
  }

  drain_pending(registry, &id);

  Ok(Some(library))
}

fn plugin_dirs() -> Vec<PathBuf> {
  let mut dirs = Vec::new();

  if let Ok(paths) = std::env::var("SLOWSHELL_PLUGIN_PATH") {
    dirs.extend(
      paths
        .split(':')
        .filter(|p| !p.is_empty())
        .map(PathBuf::from),
    );
  }

  let data_home = std::env::var_os("XDG_DATA_HOME")
    .map(PathBuf::from)
    .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
  if let Some(data_home) = data_home {
    dirs.push(data_home.join("slowshell/plugins"));
  }

  let data_dirs =
    std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
  dirs.extend(
    data_dirs
      .split(':')
      .filter(|d| !d.is_empty())
      .map(|d| PathBuf::from(d).join("slowshell/plugins")),
  );

  if cfg!(debug_assertions) {
    dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug"));
  }

  dirs
}

enum Pending {
  Component {
    name: String,
    vtable: *const SlComponentVtable,
  },
  Compositor {
    name: String,
    vtable: *const SlCompositorVtable,
  },
  Payload {
    command: String,
    vtable: *const SlPayloadVtable,
  },
  Renderable {
    name: String,
    vtable: *const SlRenderableVtable,
  },
  DesktopItem {
    name: String,
    vtable: *const SlDesktopItemVtable,
  },
  ConfigParser {
    name: String,
    callback: SlConfigParserFn,
  },
  Spotlight {
    name: String,
    vtable: *const SlSpotlightVtable,
  },
  Command {
    name: String,
    title: Option<String>,
    description: Option<String>,
  },
}

thread_local! {
  static PENDING: RefCell<Vec<Pending>> = const { RefCell::new(Vec::new()) };
  static CURRENT_PLUGIN: RefCell<Option<String>> = const { RefCell::new(None) };
  static PERSISTENCE_OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn drain_pending(registry: &mut GlobalRegistry, plugin_id: &str) {
  let pending = PENDING.with(|pending| std::mem::take(&mut *pending.borrow_mut()));

  for item in pending {
    match item {
      Pending::Component { name, vtable } => {
        #[cfg(not(feature = "panels"))]
        let _ = (name, vtable);
        #[cfg(feature = "panels")]
        {
          let ctx = Box::into_raw(Box::new(PluginInstance::new(plugin_id))) as usize;
          let vtable = vtable as usize;

          let factory: ComponentFactory = Box::new(move || {
            let ctx = ctx as *mut c_void;
            let vtable = vtable as *const SlComponentVtable;
            let state = unsafe { ((*vtable).create.expect("component create"))(ctx) };
            Box::new(PluginComponent::new(ctx, vtable, state)) as Box<dyn Component>
          });

          contribute(plugin_id, |contributions| {
            contributions.components.push(name.clone());
          });

          registry.include_in(
            "components",
            ResourceRegistration::Unknown(Box::new(ComponentRegistration::new(name, factory))),
          );
        }
      }

      Pending::Compositor { name, vtable } => {
        let ctx = Box::into_raw(Box::new(PluginInstance::new(plugin_id))) as usize;
        let vtable = vtable as usize;

        let factory: CompositorFactory = Box::new(move || {
          let ctx = ctx as *mut PluginInstance;
          let vtable = vtable as *const SlCompositorVtable;
          let state = unsafe { ((*vtable).create.expect("compositor create"))(ctx as *mut c_void) };
          Box::new(PluginCompositor { ctx, vtable, state }) as Box<dyn Compositor>
        });

        contribute(plugin_id, |contributions| {
          contributions.compositors.push(name.clone());
        });

        registry.include_in(
          "compositor",
          ResourceRegistration::Unknown(Box::new(CompositorRegistration::new(name, factory))),
        );
      }

      Pending::Payload { command, vtable } => {
        let ctx = Box::into_raw(Box::new(PluginInstance::new(plugin_id))) as *mut c_void;
        let state = unsafe { ((*vtable).create.expect("payload create"))(ctx) };

        payloads().lock().unwrap().push(PayloadEntry {
          command: command.clone().into(),
          ctx,
          vtable,
          state,
        });

        contribute(plugin_id, |contributions| {
          contributions.payloads.push(command.clone());
        });

        registry.include_in(
          "payload",
          ResourceRegistration::Unknown(Box::new(PayloadBuilder {
            commands: Box::leak(
              vec![Box::leak(command.into_boxed_str()) as &'static str].into_boxed_slice(),
            ),
            build: plugin_payload_build,
          })),
        );
      }

      Pending::Renderable { name, vtable } => {
        let ctx = Box::into_raw(Box::new(PluginInstance::new(plugin_id))) as *mut c_void;
        renderables().lock().unwrap().push(RenderableEntry {
          name: name.clone().into(),
          ctx,
          vtable,
        });

        contribute(plugin_id, |contributions| {
          contributions.renderables.push(name);
        });
      }

      Pending::DesktopItem { name, vtable } => {
        let ctx = Box::into_raw(Box::new(PluginInstance::new(plugin_id))) as *mut c_void;
        desktop_items().lock().unwrap().push(DesktopEntry {
          name: name.clone().into(),
          ctx,
          vtable,
        });

        contribute(plugin_id, |contributions| {
          contributions.desktop_items.push(name);
        });
      }

      Pending::ConfigParser { name, callback } => {
        contribute(plugin_id, |contributions| {
          contributions.config_parsers.push(name.clone());
        });

        register_plugin_config_parser(plugin_id, name, callback);
      }

      Pending::Spotlight { name, vtable } => {
        let ctx = Box::into_raw(Box::new(PluginInstance::new(plugin_id))) as *mut c_void;
        spotlights().lock().unwrap().push(SpotlightEntry {
          name: name.clone(),
          ctx,
          vtable,
        });

        contribute(plugin_id, |contributions| {
          contributions.spotlights.push(name);
        });
      }

      Pending::Command {
        name,
        title,
        description,
      } => {
        slowshell_core::commands::register(CommandEntry {
          name: name.clone().into(),
          title,
          description,
          icon: None,
          action: ListenerAction::Named(name.clone().into()),
        });

        contribute(plugin_id, |contributions| {
          contributions.commands.push(name);
        });
      }
    }
  }
}

struct SpotlightEntry {
  name: String,
  #[allow(unused)]
  ctx: *mut c_void,
  #[allow(unused)]
  vtable: *const SlSpotlightVtable,
}

unsafe impl Send for SpotlightEntry {}

fn spotlights() -> &'static Mutex<Vec<SpotlightEntry>> {
  static SPOTLIGHTS: OnceLock<Mutex<Vec<SpotlightEntry>>> = OnceLock::new();
  SPOTLIGHTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn registries() -> &'static Mutex<HashMap<usize, usize>> {
  static REGISTRIES: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();
  REGISTRIES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn registry_subscribers() -> &'static Mutex<HashMap<usize, Vec<(usize, SlRegistryNotifyFn)>>> {
  static SUBS: OnceLock<Mutex<HashMap<usize, Vec<(usize, SlRegistryNotifyFn)>>>> = OnceLock::new();
  SUBS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn notify_registry(key: usize, value: *mut c_void) {
  let subscribers: Vec<(usize, SlRegistryNotifyFn)> = registry_subscribers()
    .lock()
    .unwrap()
    .get(&key)
    .cloned()
    .unwrap_or_default();

  for (ctx, callback) in subscribers {
    unsafe { callback(ctx as *mut c_void, key, value) };
  }
}

unsafe extern "C" fn host_registry_register(
  _ctx: *mut c_void,
  key: usize,
  value: *mut c_void,
) -> i32 {
  {
    let mut registries = registries().lock().unwrap();
    match registries.entry(key) {
      std::collections::hash_map::Entry::Occupied(_) => return -1,
      std::collections::hash_map::Entry::Vacant(entry) => {
        entry.insert(value as usize);
      }
    }
  }
  notify_registry(key, value);
  0
}

unsafe extern "C" fn host_registry_get(_ctx: *mut c_void, key: usize) -> *mut c_void {
  registries()
    .lock()
    .unwrap()
    .get(&key)
    .map(|value| *value as *mut c_void)
    .unwrap_or(core::ptr::null_mut())
}

unsafe extern "C" fn host_registry_remove(_ctx: *mut c_void, key: usize) -> *mut c_void {
  match registries().lock().unwrap().remove(&key) {
    Some(value) => {
      notify_registry(key, core::ptr::null_mut());
      value as *mut c_void
    }
    None => core::ptr::null_mut(),
  }
}

unsafe extern "C" fn host_registry_subscribe(
  ctx: *mut c_void,
  key: usize,
  callback: SlRegistryNotifyFn,
) -> i32 {
  let existing = registries()
    .lock()
    .unwrap()
    .get(&key)
    .map(|value| *value as *mut c_void);

  registry_subscribers()
    .lock()
    .unwrap()
    .entry(key)
    .or_default()
    .push((ctx as usize, callback));

  if let Some(value) = existing {
    unsafe { callback(ctx, key, value) };
  }
  0
}

unsafe extern "C" fn host_persistence_get(ctx: *mut c_void, key: SlStr, out: *mut SlStr) -> i32 {
  if out.is_null() {
    return -1;
  }
  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };
  let Some(plugin_id) = plugin_id_from_ctx(ctx) else {
    return -1;
  };

  let value = with_plugin_persistence(&plugin_id, |map| map.get(key).cloned());
  let Some(value) = value else {
    return -1;
  };

  PERSISTENCE_OUT.with(|scratch| {
    let mut scratch = scratch.borrow_mut();
    scratch.clear();
    scratch.extend_from_slice(value.as_bytes());
    let bytes = scratch.as_slice();
    unsafe {
      *out = SlStr {
        ptr: bytes.as_ptr(),
        len: bytes.len(),
      };
    }
  });

  0
}

unsafe extern "C" fn host_persistence_set(ctx: *mut c_void, key: SlStr, value: SlStr) -> i32 {
  let (Some(key), Some(value)) = (unsafe { key.as_str() }, unsafe { value.as_str() }) else {
    return -1;
  };
  let Some(plugin_id) = plugin_id_from_ctx(ctx) else {
    return -1;
  };

  let snapshot = with_plugin_persistence(&plugin_id, |map| {
    map.insert(key.to_owned(), value.to_owned());
    map.clone()
  });
  flush_plugin_persistence(&plugin_id, &snapshot);

  0
}

unsafe extern "C" fn host_persistence_remove(ctx: *mut c_void, key: SlStr) -> i32 {
  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };
  let Some(plugin_id) = plugin_id_from_ctx(ctx) else {
    return -1;
  };

  let snapshot = with_plugin_persistence(&plugin_id, |map| {
    map.remove(key);
    map.clone()
  });
  flush_plugin_persistence(&plugin_id, &snapshot);

  0
}

struct PluginInstance {
  plugin_id: String,
  compositor_state: CompositorState,
}

impl PluginInstance {
  fn new(plugin_id: &str) -> Self {
    Self {
      plugin_id: plugin_id.to_owned(),
      compositor_state: CompositorState::default(),
    }
  }
}

fn plugin_persistence() -> &'static Mutex<HashMap<String, HashMap<String, String>>> {
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

fn plugin_id_from_ctx(ctx: *mut c_void) -> Option<String> {
  if ctx.is_null() {
    return None;
  }
  Some(
    unsafe { &*(ctx as *const PluginInstance) }
      .plugin_id
      .clone(),
  )
}

fn with_plugin_persistence<R>(
  plugin_id: &str,
  f: impl FnOnce(&mut HashMap<String, String>) -> R,
) -> R {
  let mut store = plugin_persistence().lock().unwrap();
  let map = store.entry(plugin_id.to_owned()).or_insert_with(|| {
    slowshell_core::persistence::load(persistence_file(plugin_id)).unwrap_or_default()
  });

  f(map)
}

fn flush_plugin_persistence(plugin_id: &str, map: &HashMap<String, String>) {
  if let Err(e) = slowshell_core::persistence::save(persistence_file(plugin_id), map) {
    eprintln!("[plugin:{plugin_id}] failed to persist state: {e}");
  }
}

#[derive(Clone)]
pub struct PluginPayloadArgs {
  pub command: Ustr,
  pub args: HashMap<Ustr, Ustr>,
}

struct PayloadEntry {
  command: Ustr,
  ctx: *mut c_void,
  vtable: *const SlPayloadVtable,
  state: *mut c_void,
}

unsafe impl Send for PayloadEntry {}

fn payloads() -> &'static Mutex<Vec<PayloadEntry>> {
  static PAYLOADS: OnceLock<Mutex<Vec<PayloadEntry>>> = OnceLock::new();
  PAYLOADS.get_or_init(|| Mutex::new(Vec::new()))
}

fn plugin_payload_build(command: &str, args: PayloadBuilderArgs) -> Option<PayloadBox> {
  let args = args
    .as_map()
    .into_iter()
    .map(|(key, value)| (Ustr::from(key), Ustr::from(value)))
    .collect();
  Some(PayloadBox::new(PluginPayloadArgs {
    command: command.into(),
    args,
  }))
}

fn dispatch_payload(name: &Ustr, args: &PluginPayloadArgs) {
  let entry = {
    let payloads = payloads().lock().unwrap();
    payloads
      .iter()
      .find(|entry| &entry.command == name)
      .map(|entry| (entry.ctx, entry.vtable, entry.state))
  };

  let Some((ctx, vtable, state)) = entry else {
    return;
  };

  let keys = args
    .args
    .iter()
    .map(|(key, _)| SlStr {
      ptr: key.as_bytes().as_ptr(),
      len: key.len(),
    })
    .collect::<Vec<_>>();

  let values = args
    .args
    .iter()
    .map(|(_, value)| SlStr {
      ptr: value.as_bytes().as_ptr(),
      len: value.len(),
    })
    .collect::<Vec<_>>();

  let sl_args = SlPayloadArgs {
    count: args.args.len(),
    keys: if keys.is_empty() {
      core::ptr::null()
    } else {
      keys.as_ptr()
    },
    values: if values.is_empty() {
      core::ptr::null()
    } else {
      values.as_ptr()
    },
  };
  if let Some(invoke) = unsafe { (*vtable).invoke } {
    unsafe { invoke(ctx, state, &sl_args as *const _ as *mut c_void) };
  }
}

struct PluginDispatcher;

impl DesktopItem for PluginDispatcher {
  fn id(&self) -> &str {
    "plugin-dispatcher"
  }

  fn layer(
    &self,
    _config: &Config,
    _monitor: &str,
  ) -> iced_layershell::reexport::NewLayerShellSettings {
    Default::default()
  }

  fn visibility(&self) -> Visibility {
    Visibility::Transient
  }

  fn update_strategy(&self) -> UpdateWhen {
    UpdateWhen::OnDemand
  }

  fn init_events(&self) -> Vec<EventFilter> {
    payloads()
      .lock()
      .unwrap()
      .iter()
      .map(|entry| EventFilter::Payload(entry.command.clone()))
      .collect()
  }

  fn update(
    &mut self,
    config: &Config,
    store: &mut Store,
    event: &ListenerAction,
  ) -> miette::Result<ItemEffect> {
    let _config_scope = ScopedConfig::current(config);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope(store);

    if let ListenerAction::Payload { name, payload } = event {
      let empty;
      let args = match payload
        .as_ref()
        .and_then(|payload| payload.as_this::<PluginPayloadArgs>())
      {
        Some(args) => args,
        None => {
          empty = PluginPayloadArgs {
            command: name.clone(),
            args: HashMap::new(),
          };
          &empty
        }
      };

      let (_, calls) = with_plugin_calls(|| dispatch_payload(name, args));
      if calls.redraw {
        return Ok(ItemEffect::Redraw);
      }
    }

    Ok(ItemEffect::None)
  }

  fn view<'a>(
    &'a self,
    _config: &'a Config,
    _store: &'a Store,
    _id: iced_layershell::reexport::IcedId,
    _monitor: &str,
  ) -> iced::Element<'a, ItemMessage> {
    iced::widget::space().into()
  }
}

fn plugin_dispatcher_item(
  _config: &Config,
  _store: &Store,
) -> miette::Result<Vec<Box<dyn DesktopItem + Send>>> {
  Ok(vec![Box::new(PluginDispatcher)])
}

struct RenderableEntry {
  name: Ustr,
  ctx: *mut c_void,
  vtable: *const SlRenderableVtable,
}

unsafe impl Send for RenderableEntry {}

fn renderables() -> &'static Mutex<Vec<RenderableEntry>> {
  static RENDERABLES: OnceLock<Mutex<Vec<RenderableEntry>>> = OnceLock::new();

  RENDERABLES.get_or_init(|| Mutex::new(Vec::new()))
}

struct PluginRenderable {
  ctx: *mut c_void,
  vtable: *const SlRenderableVtable,
  state: *mut c_void,
}

unsafe impl Send for PluginRenderable {}

impl PluginRenderable {
  fn settings(&self) -> SlRenderableSettings {
    let mut settings = SlRenderableSettings::default();

    if let Some(settings_fn) = unsafe { (*self.vtable).settings } {
      unsafe { settings_fn(self.ctx, self.state, &mut settings) };
    }

    settings
  }
}

impl Drop for PluginRenderable {
  fn drop(&mut self) {
    if let Some(destroy) = unsafe { (*self.vtable).destroy } {
      unsafe { destroy(self.ctx, self.state) };
    }
  }
}

impl Renderable for PluginRenderable {
  fn initialize(&self, config: &Config, store: &mut Store) -> Box<dyn std::any::Any + Send> {
    let _config_scope = ScopedConfig::current(config);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope(store);
    if let Some(initialize) = unsafe { (*self.vtable).initialize } {
      unsafe { initialize(self.ctx, self.state) };
    }
    Box::new(())
  }

  fn update(
    &mut self,
    config: &Config,
    store: &mut Store,
    _data: &dyn std::any::Any,
  ) -> Option<ItemEffect> {
    let update = unsafe { (*self.vtable).update }?;
    let _config_scope = ScopedConfig::current(config);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope(store);
    Some(effect_from_sl(unsafe { update(self.ctx, self.state) }))
  }

  fn handle_message(&mut self, message: &ItemMessage) -> Option<ItemEffect> {
    let handle = unsafe { (*self.vtable).handle_message }?;
    let message = item_message_to_sl(message);
    Some(effect_from_sl(unsafe {
      handle(self.ctx, self.state, &message)
    }))
  }

  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: iced_layershell::reexport::IcedId,
    data: &'a dyn std::any::Any,
  ) -> iced::Element<'a, ItemMessage> {
    let _config_scope = ScopedConfig::current(config);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope(store);
    reset_canvas_pool();

    let mut list = SlNodeList::EMPTY;
    if let Some(view) = unsafe { (*self.vtable).view } {
      unsafe { view(self.ctx, self.state, std::ptr::null_mut(), &mut list) };
    }

    let rendered = unsafe { render_nodes(&list, Some(id), self.ctx as usize) };
    let content = container(rendered).padding(12.0);

    let settings = self.settings();

    if !settings.wrap_popup {
      return content.into();
    }

    let Some(popup) = data.downcast_ref::<PopupSettings>() else {
      return content.into();
    };

    let mut sized = SizedPopup::new(content)
      .with_backdrop(Backdrop::transparent().with_close_on_click(true))
      .with_position(popup.position.0.get(store), popup.position.1.get(store))
      .with_panel_edge(popup.panel_edge)
      .with_panel_insets(store)
      .with_on_close(ItemMessage::Effect(id, ItemEffect::Hide));

    sized = sized.with_width(if settings.width > 0.0 {
      settings.width
    } else {
      120.0
    });
    if settings.height > 0.0 {
      sized = sized.with_height(settings.height);
    }

    sized.into()
  }
}

fn register_plugin_renderables(_config: &Config, store: &mut Store) {
  let entries: Vec<(Ustr, *mut c_void, *const SlRenderableVtable)> = renderables()
    .lock()
    .unwrap()
    .iter()
    .map(|entry| (entry.name.clone(), entry.ctx, entry.vtable))
    .collect();

  let Some(table) = store.borrow_mut::<Renderables>() else {
    return;
  };

  for (name, ctx, vtable) in entries {
    let state = unsafe { ((*vtable).create.expect("renderable create"))(ctx) };
    table.insert(name, Box::new(PluginRenderable { ctx, vtable, state }));
  }
}

#[cfg(not(feature = "spotlight"))]
fn register_plugin_spotlights(config: &Config, store: &mut Store) {
  let _ = (config, store);
}

#[cfg(feature = "spotlight")]
fn register_plugin_spotlights(config: &Config, store: &mut Store) {
  let entries: Vec<(String, *mut c_void, *const SlSpotlightVtable)> = spotlights()
    .lock()
    .unwrap()
    .iter()
    .map(|entry| (entry.name.clone(), entry.ctx, entry.vtable))
    .collect();

  let config_addr = config as *const Config as usize;

  let Some(modes) = store.borrow_mut::<SpotlightModes>() else {
    return;
  };

  for (name, ctx, vtable) in entries {
    let Some(create) = (unsafe { (*vtable).create }) else {
      continue;
    };
    let state = unsafe { create(ctx) };
    let ctx_addr = ctx as usize;
    let vtable_addr = vtable as usize;
    let state_addr = state as usize;

    let allows_triggers = unsafe { (*vtable).allows_triggers };
    let has_trigger = unsafe { (*vtable).trigger_check.is_some() };
    let display = plugin_display_styles(unsafe { (*vtable).display_styles });

    modes.register_mode(
      name.clone(),
      SpotlightMode {
        kind: SpotlightKind::Generate(Box::new(move |query, store| {
          plugin_spotlight_generate(config_addr, ctx_addr, vtable_addr, state_addr, query, store)
        })),
        display,
      },
      allows_triggers,
    );

    if has_trigger {
      let trigger_fn = make_trigger_fn(config_addr, ctx_addr, vtable_addr, state_addr);
      modes.register_trigger(name, trigger_fn);
    }
  }
}

#[cfg(feature = "spotlight")]
fn plugin_display_styles(bits: u32) -> &'static [DisplayStyle] {
  let mut styles = Vec::new();

  if bits & sl_display_style::LIST != 0 {
    styles.push(DisplayStyle::List);
  }
  if bits & sl_display_style::GRID != 0 {
    styles.push(DisplayStyle::Grid);
  }
  if bits & sl_display_style::IMAGE_LIST != 0 {
    styles.push(DisplayStyle::ImageList);
  }
  if styles.is_empty() {
    styles.push(DisplayStyle::List);
  }

  Box::leak(styles.into_boxed_slice())
}

#[cfg(feature = "spotlight")]
fn make_trigger_fn(
  config: usize,
  ctx: usize,
  vtable: usize,
  state: usize,
) -> Box<dyn Fn(&str) -> bool + Send + Sync> {
  Box::new(move |query: &str| plugin_spotlight_trigger_check(config, ctx, vtable, state, query))
}

#[cfg(feature = "spotlight")]
fn plugin_spotlight_trigger_check(
  config: usize,
  ctx: usize,
  vtable: usize,
  state: usize,
  query: &str,
) -> bool {
  let vtable_ptr = vtable as *const SlSpotlightVtable;
  let ctx_ptr = ctx as *mut c_void;
  let state_ptr = state as *mut c_void;

  let Some(trigger_check) = (unsafe { (*vtable_ptr).trigger_check }) else {
    return false;
  };

  let _config_scope = ScopedConfig::current(unsafe { &*(config as *const Config) });

  let query_sl = SlStr::from_str(query);
  unsafe { trigger_check(ctx_ptr, state_ptr, query_sl) }
}

#[cfg(feature = "spotlight")]
fn plugin_spotlight_generate(
  config: usize,
  ctx: usize,
  vtable: usize,
  state: usize,
  query: &str,
  store: &Store,
) -> Vec<SpotlightItem> {
  let vtable_ptr = vtable as *const SlSpotlightVtable;
  let ctx_ptr = ctx as *mut c_void;
  let state_ptr = state as *mut c_void;

  let Some(generate) = (unsafe { (*vtable_ptr).generate }) else {
    return Vec::new();
  };

  let _config_scope = ScopedConfig::current(unsafe { &*(config as *const Config) });
  let _compositor_scope = compositor_scope(store);
  let _notifications_scope = notifications_scope(store);
  let _store_scope = store_scope(store);

  let mut list = SlSpotlightList::default();
  if unsafe { generate(ctx_ptr, state_ptr, SlStr::from_str(query), &mut list) } != 0
    || list.items.is_null()
    || list.count == 0
  {
    return Vec::new();
  }

  let items = unsafe { std::slice::from_raw_parts(list.items, list.count) };
  items
    .iter()
    .enumerate()
    .map(|(index, item)| {
      let icon = unsafe { item.icon.as_str() }
        .filter(|icon| !icon.is_empty())
        .map(str::to_owned);
      let title = unsafe { item.title.as_str() }
        .unwrap_or_default()
        .to_owned();
      let subtitle = unsafe { item.subtitle.as_str() }
        .filter(|subtitle| !subtitle.is_empty())
        .map(str::to_owned);
      let image = unsafe { item.image.as_str() }
        .filter(|image| !image.is_empty())
        .map(std::path::PathBuf::from);
      let action_label = unsafe { item.action_label.as_str() }
        .filter(|label| !label.is_empty())
        .map(str::to_owned);
      let tags = if item.tags.is_null() || item.tag_count == 0 {
        None
      } else {
        Some(
          (0..item.tag_count)
            .filter_map(|i| unsafe { (*item.tags.add(i)).as_str() })
            .map(str::to_owned)
            .collect::<Vec<_>>(),
        )
      };

      let action = SpotlightAction::Custom(std::sync::Arc::new(move || {
        let vtable = vtable as *const SlSpotlightVtable;
        let ctx = ctx as *mut c_void;
        let state = state as *mut c_void;
        if let Some(activate) = unsafe { (*vtable).activate } {
          unsafe { activate(ctx, state, index) };
        }
      }));

      SpotlightItem {
        image,
        image_bytes: None,
        image_icon: icon,
        title,
        subtitle,
        subtext: None,
        tags,
        actions: vec![SpotlightActionDef {
          title: action_label,
          action,
        }],
      }
    })
    .collect()
}

struct DesktopEntry {
  name: Ustr,
  ctx: *mut c_void,
  vtable: *const SlDesktopItemVtable,
}

unsafe impl Send for DesktopEntry {}

fn desktop_items() -> &'static Mutex<Vec<DesktopEntry>> {
  static ITEMS: OnceLock<Mutex<Vec<DesktopEntry>>> = OnceLock::new();
  ITEMS.get_or_init(|| Mutex::new(Vec::new()))
}

fn anchor_from_bits(bits: u32) -> Anchor {
  let mut anchor = Anchor::empty();
  if bits & sl_anchor::TOP != 0 {
    anchor |= Anchor::Top;
  }
  if bits & sl_anchor::BOTTOM != 0 {
    anchor |= Anchor::Bottom;
  }
  if bits & sl_anchor::LEFT != 0 {
    anchor |= Anchor::Left;
  }
  if bits & sl_anchor::RIGHT != 0 {
    anchor |= Anchor::Right;
  }
  anchor
}

struct PluginDesktopItem {
  id: Ustr,
  ctx: *mut c_void,
  vtable: *const SlDesktopItemVtable,
  state: *mut c_void,
  event_name: Ustr,
  timers: Vec<(i32, u32)>,
  fds: Vec<(i32, u32)>,
}

unsafe impl Send for PluginDesktopItem {}

impl PluginDesktopItem {
  fn new(
    name: Ustr,
    ctx: *mut c_void,
    vtable: *const SlDesktopItemVtable,
    state: *mut c_void,
  ) -> Self {
    let instance = NEXT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed);
    Self {
      id: name,
      ctx,
      vtable,
      state,
      event_name: format!("component/plugin.{instance}").into(),
      timers: Vec::new(),
      fds: Vec::new(),
    }
  }

  fn settings(&self) -> SlDesktopSettings {
    let mut settings = SlDesktopSettings::default();
    if let Some(settings_fn) = unsafe { (*self.vtable).settings } {
      unsafe { settings_fn(self.ctx, self.state, &mut settings) };
    }
    settings
  }
}

impl Drop for PluginDesktopItem {
  fn drop(&mut self) {
    if let Some(destroy) = unsafe { (*self.vtable).destroy } {
      unsafe { destroy(self.ctx, self.state) };
    }
  }
}

impl DesktopItem for PluginDesktopItem {
  fn id(&self) -> &str {
    &self.id
  }

  fn layer(&self, _config: &Config, monitor: &str) -> NewLayerShellSettings {
    let settings = self.settings();

    let namespace = unsafe { settings.ns.as_str() }.unwrap_or("");
    let namespace = if namespace.is_empty() {
      self.id.to_string()
    } else {
      namespace.to_owned()
    };

    NewLayerShellSettings {
      layer: match settings.layer {
        sl_layer::BACKGROUND => Layer::Background,
        sl_layer::BOTTOM => Layer::Bottom,
        sl_layer::OVERLAY => Layer::Overlay,
        _ => Layer::Top,
      },
      anchor: anchor_from_bits(settings.anchor),
      exclusive_zone: Some(settings.exclusive_zone),
      size: if settings.width == 0 || settings.height == 0 {
        None
      } else {
        Some((settings.width, settings.height))
      },
      margin: Some((
        settings.margin[0],
        settings.margin[1],
        settings.margin[2],
        settings.margin[3],
      )),
      keyboard_interactivity: match settings.keyboard_interactivity {
        sl_keyboard_interactivity::ON_DEMAND => KeyboardInteractivity::OnDemand,
        sl_keyboard_interactivity::EXCLUSIVE => KeyboardInteractivity::Exclusive,
        _ => KeyboardInteractivity::None,
      },
      events_transparent: settings.events_transparent,
      namespace: Some(namespace),
      output_option: if monitor.is_empty() {
        OutputOption::Active
      } else {
        OutputOption::OutputName(monitor.to_owned())
      },
    }
  }

  fn visibility(&self) -> Visibility {
    match self.settings().visibility {
      sl_visibility::TRANSIENT => Visibility::Transient,
      sl_visibility::TOGGLEABLE => Visibility::Toggleable(self.settings().visible),
      _ => Visibility::Visible,
    }
  }

  fn update_strategy(&self) -> UpdateWhen {
    match self.settings().update_when {
      sl_update_when::EVERY_FRAME => UpdateWhen::EveryFrame,
      sl_update_when::ON_EVENT => UpdateWhen::OnEvent,
      _ => UpdateWhen::OnDemand,
    }
  }

  fn monitor(&self, _config: &Config) -> MonitorScope {
    let settings = self.settings();
    if settings.per_monitor {
      MonitorScope::PerMonitor
    } else {
      let monitor = unsafe { settings.monitor.as_str() }.unwrap_or("");
      if monitor.is_empty() {
        MonitorScope::Single(None)
      } else {
        MonitorScope::Single(Some(monitor.into()))
      }
    }
  }

  fn init_events(&self) -> Vec<EventFilter> {
    let mask = unsafe { (*self.vtable).events }
      .map(|events| unsafe { events(self.ctx, self.state) })
      .unwrap_or(0);

    let mut filters = filters_from_mask(mask);
    if mask & (sl_event_mask::TICK | sl_event_mask::FD) != 0 {
      filters.push(EventFilter::Named(self.event_name.clone()));
    }
    filters
  }

  fn initialize(&mut self, store: &mut Store) -> miette::Result<Void> {
    clear_plugin_calls(&mut self.timers, &mut self.fds, store);
    let _store_scope = store_scope(store);
    if let Some(initialize) = unsafe { (*self.vtable).initialize } {
      let (_, calls) = with_plugin_calls(|| unsafe {
        initialize(self.ctx, self.state, std::ptr::null_mut());
      });
      apply_plugin_calls(
        &self.event_name,
        &mut self.timers,
        &mut self.fds,
        store,
        calls,
      );
    }
    Ok(Void)
  }

  fn update(
    &mut self,
    config: &Config,
    store: &mut Store,
    event: &ListenerAction,
  ) -> miette::Result<ItemEffect> {
    let _config_scope = ScopedConfig::current(config);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope(store);
    let Some(update) = (unsafe { (*self.vtable).update }) else {
      return Ok(ItemEffect::None);
    };

    let targeted = match event {
      ListenerAction::Timer { name, .. } | ListenerAction::Signal { name, .. } => {
        &**name == &*self.event_name
      }
      _ => false,
    };

    let mut effect_sl = SlEffect::default();
    let (_, calls) = with_plugin_calls(|| {
      if targeted {
        let (kind, action, fd) = match event {
          ListenerAction::Signal { fd, .. } => {
            let action = self
              .fds
              .iter()
              .find(|(registered, _)| registered == fd)
              .map(|(_, action)| *action)
              .unwrap_or(0);
            (sl_event_kind::FD, action, *fd)
          }
          ListenerAction::Timer { fd, .. } => {
            let tag = self
              .timers
              .iter()
              .find(|(registered, _)| registered == fd)
              .map(|(_, tag)| *tag)
              .unwrap_or(0);
            (sl_event_kind::TICK, tag, *fd)
          }
          _ => (sl_event_kind::TICK, 0, -1),
        };

        let event = SlEvent {
          kind,
          name: SlStr::from_str(&self.event_name),
          bag: std::ptr::null_mut(),
          action,
          fd,
        };
        effect_sl = unsafe { update(self.ctx, self.state, &event) };
      } else {
        let event = sl_event_from(event);
        effect_sl = unsafe { update(self.ctx, self.state, &event) };
      }
    });

    let redraw = apply_plugin_calls(
      &self.event_name,
      &mut self.timers,
      &mut self.fds,
      store,
      calls,
    );

    if effect_sl.code == sl_effect::SUBSCRIBE {
      return Ok(ItemEffect::Subscribe(self.init_events()));
    }
    let mut effect = effect_from_sl(effect_sl);
    if effect == ItemEffect::None && (targeted || redraw) {
      effect = ItemEffect::Redraw;
    }
    Ok(effect)
  }

  fn handle_message(&mut self, store: Option<&mut Store>, message: &ItemMessage) -> ItemEffect {
    let Some(handle) = (unsafe { (*self.vtable).handle_message }) else {
      return ItemEffect::None;
    };
    let _compositor_scope = store.as_deref().map(compositor_scope);
    let _notifications_scope = store.as_deref().map(notifications_scope);
    let message = item_message_to_sl(message);
    effect_from_sl(unsafe { handle(self.ctx, self.state, &message) })
  }

  fn view<'a>(
    &'a self,
    config: &'a Config,
    store: &'a Store,
    id: iced_layershell::reexport::IcedId,
    monitor: &str,
  ) -> iced::Element<'a, ItemMessage> {
    let _config_scope = ScopedConfig::current(config);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope(store);
    reset_canvas_pool();

    let mut list = SlNodeList::EMPTY;
    if let Some(view) = unsafe { (*self.vtable).view } {
      let mut bag = Bag::Monitor(monitor);
      unsafe {
        view(
          self.ctx,
          self.state,
          &mut bag as *mut _ as *mut c_void,
          &mut list,
        )
      };
    }
    unsafe { render_nodes(&list, Some(id), self.ctx as usize) }
  }
}

fn plugin_desktop_items(
  _config: &Config,
  _store: &Store,
) -> miette::Result<Vec<Box<dyn DesktopItem + Send>>> {
  let entries: Vec<(Ustr, *mut c_void, *const SlDesktopItemVtable)> = desktop_items()
    .lock()
    .unwrap()
    .iter()
    .map(|entry| (entry.name.clone(), entry.ctx, entry.vtable))
    .collect();

  Ok(
    entries
      .into_iter()
      .map(|(name, ctx, vtable)| {
        let state = unsafe { ((*vtable).create.expect("desktop item create"))(ctx) };
        Box::new(PluginDesktopItem::new(name, ctx, vtable, state)) as Box<dyn DesktopItem + Send>
      })
      .collect(),
  )
}

#[derive(Default)]
pub struct PluginConfigs {
  pub blocks: HashMap<String, HashMap<String, String>>,
}

fn plugin_ids() -> &'static Mutex<Vec<String>> {
  static IDS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
  IDS.get_or_init(|| Mutex::new(Vec::new()))
}

#[derive(Clone)]
pub struct PluginInfo {
  pub id: String,
  pub version: String,
  pub path: PathBuf,
}

fn plugin_infos() -> &'static Mutex<Vec<PluginInfo>> {
  static INFOS: OnceLock<Mutex<Vec<PluginInfo>>> = OnceLock::new();
  INFOS.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn loaded_plugins() -> Vec<PluginInfo> {
  plugin_infos().lock().unwrap().clone()
}

pub fn renderable_names() -> Vec<String> {
  renderables()
    .lock()
    .unwrap()
    .iter()
    .map(|entry| entry.name.to_string())
    .collect()
}

pub fn spotlight_names() -> Vec<String> {
  spotlights()
    .lock()
    .unwrap()
    .iter()
    .map(|entry| entry.name.clone())
    .collect()
}

#[derive(Default, Clone)]
pub struct PluginContributions {
  pub components: Vec<String>,
  pub compositors: Vec<String>,
  pub payloads: Vec<String>,
  pub renderables: Vec<String>,
  pub desktop_items: Vec<String>,
  pub config_parsers: Vec<String>,
  pub spotlights: Vec<String>,
  pub commands: Vec<String>,
  pub styles: Vec<String>,
}

fn plugin_contributions() -> &'static Mutex<HashMap<String, PluginContributions>> {
  static CONTRIBUTIONS: OnceLock<Mutex<HashMap<String, PluginContributions>>> = OnceLock::new();
  CONTRIBUTIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn contribute(plugin_id: &str, update: impl FnOnce(&mut PluginContributions)) {
  let mut contributions = plugin_contributions().lock().unwrap();
  update(contributions.entry(plugin_id.to_owned()).or_default());
}

pub fn contributions(plugin_id: &str) -> PluginContributions {
  plugin_contributions()
    .lock()
    .unwrap()
    .get(plugin_id)
    .cloned()
    .unwrap_or_default()
}

static SILENT: AtomicBool = AtomicBool::new(false);

pub fn set_silent(silent: bool) {
  SILENT.store(silent, Ordering::Relaxed);
}

fn plugin_log(message: std::fmt::Arguments) {
  if !SILENT.load(Ordering::Relaxed) {
    eprintln!("{message}");
  }
}

fn plugin_configs() -> &'static Mutex<HashMap<String, HashMap<String, String>>> {
  static CONFIGS: OnceLock<Mutex<HashMap<String, HashMap<String, String>>>> = OnceLock::new();
  CONFIGS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn plugin_config_parsers() -> &'static Mutex<Vec<(String, usize, String, SlConfigParserFn)>> {
  static PARSERS: OnceLock<Mutex<Vec<(String, usize, String, SlConfigParserFn)>>> = OnceLock::new();
  PARSERS.get_or_init(|| Mutex::new(Vec::new()))
}

fn plugin_errors() -> &'static Mutex<HashMap<String, (u32, String)>> {
  static ERRORS: OnceLock<Mutex<HashMap<String, (u32, String)>>> = OnceLock::new();
  ERRORS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn last_plugin_error(plugin_id: &str) -> Option<(u32, String)> {
  plugin_errors().lock().unwrap().get(plugin_id).cloned()
}

fn record_plugin_error(plugin_id: &str, code: u32, message: &str) {
  plugin_errors()
    .lock()
    .unwrap()
    .insert(plugin_id.to_owned(), (code, message.to_owned()));
  eprintln!("[plugin:{plugin_id}] error {code}: {message}");
}

fn register_plugin_config_parser(plugin_id: &str, name: String, callback: SlConfigParserFn) {
  let ctx = Box::into_raw(Box::new(PluginInstance::new(plugin_id))) as usize;
  plugin_config_parsers()
    .lock()
    .unwrap()
    .push((plugin_id.to_owned(), ctx, name, callback));
}

unsafe extern "C" fn host_config_parser_register(
  _host: *mut c_void,
  name: SlStr,
  callback: SlConfigParserFn,
) -> i32 {
  let Some(name) = (unsafe { name.as_str() }) else {
    return -1;
  };

  PENDING.with(|pending| {
    pending.borrow_mut().push(Pending::ConfigParser {
      name: name.to_owned(),
      callback,
    });
  });
  0
}

fn node_value(node: &kdl::KdlNode) -> Option<String> {
  if let Some(value) = str_arg(node, 0) {
    return Some(value.to_owned());
  }
  if let Some(value) = int_arg(node, 0) {
    return Some(value.to_string());
  }
  if let Some(value) = float_arg(node, 0) {
    return Some(value.to_string());
  }
  if let Some(value) = bool_arg(node, 0) {
    return Some(value.to_string());
  }
  None
}

fn parse_plugin_configs(
  nodes: &[kdl::KdlNode],
) -> miette::Result<Option<Box<dyn std::any::Any + Send + Sync>>> {
  let ids = plugin_ids().lock().unwrap().clone();
  let mut blocks: HashMap<String, HashMap<String, String>> = HashMap::new();

  for id in ids {
    let Some(node) = find_node(nodes, &id) else {
      continue;
    };

    let mut block = HashMap::new();
    for child in node_children(node) {
      if let Some(value) = node_value(child) {
        block.insert(node_name(child).to_owned(), value);
      }
    }

    blocks.insert(id, block);
  }

  let has_any = !blocks.is_empty();
  *plugin_configs().lock().unwrap() = blocks.clone();

  let parsers: Vec<(String, usize, String, SlConfigParserFn)> =
    plugin_config_parsers().lock().unwrap().clone();
  for (plugin_id, ctx, name, callback) in parsers {
    let ctx = ctx as *mut c_void;
    let result = match find_node(nodes, &name) {
      Some(node) => {
        let text = node.to_string();
        unsafe { callback(ctx, SlStr::from_str(&name), SlStr::from_str(&text)) }
      }
      None => unsafe { callback(ctx, SlStr::from_str(&name), SlStr::EMPTY) },
    };
    if result != 0 {
      record_plugin_error(&plugin_id, 1, &format!("rejected config entry `{name}`"));
    }
  }

  if has_any {
    Ok(Some(Box::new(PluginConfigs { blocks })))
  } else {
    Ok(None)
  }
}

fn plugin_config_value(ctx: *mut c_void, key: &str) -> Option<String> {
  if ctx.is_null() {
    return None;
  }

  let plugin_id = unsafe { &*(ctx as *const PluginInstance) }
    .plugin_id
    .as_str();
  let configs = plugin_configs().lock().unwrap();
  configs
    .get(plugin_id)
    .and_then(|block| block.get(key))
    .cloned()
}

thread_local! {
  static CURRENT_CONFIG: RefCell<Option<*const Config>> = const { RefCell::new(None) };
  static STYLE_STAGING: RefCell<Option<StagedStyle>> = const { RefCell::new(None) };
  static CONFIG_STAGING: RefCell<Option<Box<str>>> = const { RefCell::new(None) };
  static CURRENT_COMPOSITOR: RefCell<Option<*const CompositorState>> = const { RefCell::new(None) };
  static COMPOSITOR_STAGING: RefCell<Option<StagedCompositor>> = const { RefCell::new(None) };
  static CURRENT_NOTIFICATIONS: RefCell<Option<SharedNotificationState>> = const { RefCell::new(None) };
  static CURRENT_LISTENERS: RefCell<Option<*mut Listeners>> = const { RefCell::new(None) };
  static CURRENT_STORE: RefCell<Option<*const Store>> = const { RefCell::new(None) };
  static SERVICE_STAGE: RefCell<Option<ServiceStage>> = const { RefCell::new(None) };
}

struct StagedStyle {
  #[allow(dead_code)]
  style: Style,
  entries: Vec<SlStyleSheetEntry>,
}

struct ScopedConfig {
  previous: Option<*const Config>,
}

impl ScopedConfig {
  fn current(config: &Config) -> Self {
    let previous = CURRENT_CONFIG.with(|cell| cell.borrow_mut().replace(config as *const Config));
    Self { previous }
  }
}

impl Drop for ScopedConfig {
  fn drop(&mut self) {
    CURRENT_CONFIG.with(|cell| *cell.borrow_mut() = self.previous);
  }
}

fn current_config() -> Option<&'static Config> {
  CURRENT_CONFIG
    .with(|cell| *cell.borrow())
    .map(|ptr| unsafe { &*ptr })
}

struct ScopedCompositor {
  previous: Option<*const CompositorState>,
}

impl ScopedCompositor {
  fn current(state: Option<&CompositorState>) -> Self {
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

fn compositor_scope(store: &Store) -> ScopedCompositor {
  let state = store
    .borrow::<CompositorStore>()
    .and_then(|store| store.state().ok());
  ScopedCompositor::current(state)
}

fn current_compositor() -> Option<&'static CompositorState> {
  CURRENT_COMPOSITOR
    .with(|cell| *cell.borrow())
    .map(|ptr| unsafe { &*ptr })
}

struct ScopedNotifications {
  previous: Option<SharedNotificationState>,
}

impl ScopedNotifications {
  fn current(shared: Option<SharedNotificationState>) -> Self {
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

fn notifications_scope(store: &Store) -> ScopedNotifications {
  ScopedNotifications::current(store.borrow::<SharedNotificationState>().cloned())
}

fn current_notifications() -> Option<SharedNotificationState> {
  CURRENT_NOTIFICATIONS.with(|cell| cell.borrow().clone())
}

struct ScopedListeners {
  previous: Option<*mut Listeners>,
}

impl ScopedListeners {
  fn current(listeners: &mut Listeners) -> Self {
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

struct ScopedStore {
  previous: Option<*const Store>,
}

impl ScopedStore {
  fn current(store: &Store) -> Self {
    let previous = CURRENT_STORE
      .with(|cell| std::mem::replace(&mut *cell.borrow_mut(), Some(store as *const Store)));
    Self { previous }
  }
}

impl Drop for ScopedStore {
  fn drop(&mut self) {
    CURRENT_STORE.with(|cell| *cell.borrow_mut() = self.previous);
  }
}

fn store_scope(store: &Store) -> ScopedStore {
  ScopedStore::current(store)
}

fn current_store() -> Option<&'static Store> {
  CURRENT_STORE
    .with(|cell| *cell.borrow())
    .map(|ptr| unsafe { &*ptr })
}

#[allow(dead_code)]
enum ServiceStage {
  Audio {
    strings: Vec<Box<[u8]>>,
    sinks: Vec<SlAudioSink>,
    state: SlAudioState,
  },
  Bluetooth {
    strings: Vec<Box<[u8]>>,
    devices: Vec<SlBluetoothDevice>,
    state: SlBluetoothState,
  },
  Network {
    strings: Vec<Box<[u8]>>,
    networks: Vec<SlAccessPoint>,
    state: SlNetworkState,
  },
  System {
    strings: Vec<Box<[u8]>>,
    processes: Vec<SlProcessInfo>,
    state: SlSystemState,
  },
  Tray {
    strings: Vec<Box<[u8]>>,
    items: Vec<SlTrayItem>,
    state: SlTrayState,
  },
  Power {
    strings: Vec<Box<[u8]>>,
    profiles: Vec<SlStr>,
    state: SlPowerState,
  },
}

unsafe extern "C" fn host_dispatch(_ctx: *mut c_void, command: SlStr) -> i32 {
  let Some(command) = (unsafe { command.as_str() }) else {
    return -1;
  };
  let Some(store) = current_store() else {
    return -1;
  };
  let Some(dispatcher) = store.borrow::<ActionDispatcher>() else {
    return -1;
  };
  if dispatcher.dispatch(command) { 0 } else { -1 }
}

unsafe extern "C" fn host_dispatch_with_string(
  _ctx: *mut c_void,
  name: SlStr,
  payload: SlStr,
) -> i32 {
  let Some(name) = (unsafe { name.as_str() }) else {
    return -1;
  };
  let payload = unsafe { payload.as_str() }.unwrap_or("");
  let Some(store) = current_store() else {
    return -1;
  };
  let Some(dispatcher) = store.borrow::<ActionDispatcher>() else {
    return -1;
  };
  let Some(builders) = store.borrow::<std::sync::Arc<PayloadBuilderRegistry>>() else {
    return -1;
  };
  let Some(args) = slowshell_core::types::split_args(payload) else {
    return -1;
  };

  if dispatcher.dispatch_with_args(name, &args, builders) {
    0
  } else {
    -1
  }
}

unsafe extern "C" fn host_audio_state_get(_ctx: *mut c_void, out: *mut SlAudioState) -> i32 {
  if out.is_null() {
    return -1;
  }
  let Some(store) = current_store() else {
    return -1;
  };
  let Some(shared) = store.borrow::<SharedAudioState>() else {
    return -1;
  };

  let mut strings: Vec<Box<[u8]>> = Vec::new();
  let inner = shared.state.lock().unwrap();
  let player = shared.player.lock().unwrap().clone();

  let default_sink = stage_str(&mut strings, inner.default_sink_name.as_deref());
  let mut sinks = Vec::with_capacity(inner.sinks.len());
  for sink in &inner.sinks {
    sinks.push(SlAudioSink {
      id: sink.id,
      name: stage_str(&mut strings, Some(&sink.name)),
      description: stage_str(&mut strings, Some(&sink.description)),
      volume: sink.volume,
      muted: sink.muted,
      is_default: sink.is_default,
    });
  }

  let (has_player, player_sl) = match &player {
    Some(player) => (
      true,
      SlMprisPlayer {
        identity: stage_str(&mut strings, Some(&player.identity)),
        title: stage_str(&mut strings, Some(&player.title)),
        artist: stage_str(&mut strings, Some(&player.artist)),
        album: stage_str(&mut strings, Some(&player.album)),
        art_url: stage_str(&mut strings, player.art_url.as_deref()),
        playback_status: stage_str(&mut strings, Some(&player.playback_status)),
        can_play_pause: player.can_play_pause,
        can_go_next: player.can_go_next,
        can_go_previous: player.can_go_previous,
      },
    ),
    None => (false, SlMprisPlayer::default()),
  };

  let mut stage = ServiceStage::Audio {
    strings,
    sinks,
    state: SlAudioState::default(),
  };
  if let ServiceStage::Audio { sinks, state, .. } = &mut stage {
    *state = SlAudioState {
      volume: inner.volume,
      muted: inner.muted,
      default_sink,
      sinks: sinks.as_ptr(),
      sink_count: sinks.len(),
      has_player,
      player: player_sl,
    };
  }
  write_service_stage(stage, |stage| match stage {
    ServiceStage::Audio { state, .. } => unsafe { *out = *state },
    _ => {}
  })
}

unsafe extern "C" fn host_bluetooth_state_get(
  _ctx: *mut c_void,
  out: *mut SlBluetoothState,
) -> i32 {
  if out.is_null() {
    return -1;
  }
  let Some(store) = current_store() else {
    return -1;
  };
  let Some(shared) = store.borrow::<SharedBluetoothState>() else {
    return -1;
  };

  let mut strings: Vec<Box<[u8]>> = Vec::new();
  let inner = shared.state.lock().unwrap();
  let adapter_name = stage_str(&mut strings, inner.adapter_name.as_deref());
  let mut devices = Vec::with_capacity(inner.devices.len());
  for device in &inner.devices {
    devices.push(SlBluetoothDevice {
      address: stage_str(&mut strings, Some(&device.address)),
      name: stage_str(&mut strings, Some(&device.name)),
      icon: stage_str(&mut strings, device.icon.as_deref()),
      paired: device.paired,
      connected: device.connected,
      trusted: device.trusted,
      battery: device.battery.map(i32::from).unwrap_or(-1),
      rssi: device.rssi.map(i32::from).unwrap_or(i32::MIN),
    });
  }

  let mut stage = ServiceStage::Bluetooth {
    strings,
    devices,
    state: SlBluetoothState::default(),
  };
  if let ServiceStage::Bluetooth { devices, state, .. } = &mut stage {
    *state = SlBluetoothState {
      powered: shared.powered.load(Ordering::Relaxed),
      discovering: shared.discovering.load(Ordering::Relaxed),
      adapter_name,
      devices: devices.as_ptr(),
      device_count: devices.len(),
    };
  }
  write_service_stage(stage, |stage| match stage {
    ServiceStage::Bluetooth { state, .. } => unsafe { *out = *state },
    _ => {}
  })
}

unsafe extern "C" fn host_network_state_get(_ctx: *mut c_void, out: *mut SlNetworkState) -> i32 {
  if out.is_null() {
    return -1;
  }
  let Some(store) = current_store() else {
    return -1;
  };
  let Some(shared) = store.borrow::<SharedNetworkState>() else {
    return -1;
  };

  let mut strings: Vec<Box<[u8]>> = Vec::new();
  let inner = shared.state.lock().unwrap();

  let access_point =
    |strings: &mut Vec<Box<[u8]>>, ap: &slowshell_commons::network::AccessPoint| SlAccessPoint {
      ssid: stage_str(strings, Some(&ap.ssid)),
      signal: ap.signal,
      secured: ap.secured,
      in_use: ap.in_use,
      saved: ap.saved,
    };

  let wifi_connected = inner
    .wifi_connected
    .as_ref()
    .map(|ap| access_point(&mut strings, ap));
  let ethernet = inner.ethernet.as_ref().map(|eth| SlEthernet {
    iface: stage_str(&mut strings, Some(&eth.iface)),
    speed: eth.speed,
    carrier: eth.carrier,
    connected: eth.connected,
  });
  let mut networks = Vec::with_capacity(inner.networks.len());
  for ap in &inner.networks {
    networks.push(access_point(&mut strings, ap));
  }
  let connected = inner.connected.as_ref().map(|info| SlConnectionInfo {
    label: stage_str(&mut strings, Some(&info.label)),
    mac: stage_str(&mut strings, Some(&info.mac)),
    is_wifi: info.is_wifi,
  });

  let mut stage = ServiceStage::Network {
    strings,
    networks,
    state: SlNetworkState::default(),
  };
  if let ServiceStage::Network {
    networks, state, ..
  } = &mut stage
  {
    *state = SlNetworkState {
      wifi_enabled: shared.wifi_enabled.load(Ordering::Relaxed),
      wifi_hardware_enabled: shared.wifi_hardware_enabled.load(Ordering::Relaxed),
      scanning: shared.scanning.load(Ordering::Relaxed),
      busy: shared.busy.load(Ordering::Relaxed),
      has_wifi_connected: wifi_connected.is_some(),
      wifi_connected: wifi_connected.unwrap_or_default(),
      has_ethernet: ethernet.is_some(),
      ethernet: ethernet.unwrap_or_default(),
      networks: networks.as_ptr(),
      network_count: networks.len(),
      has_connected: connected.is_some(),
      connected: connected.unwrap_or_default(),
    };
  }
  write_service_stage(stage, |stage| match stage {
    ServiceStage::Network { state, .. } => unsafe { *out = *state },
    _ => {}
  })
}

unsafe extern "C" fn host_system_state_get(_ctx: *mut c_void, out: *mut SlSystemState) -> i32 {
  if out.is_null() {
    return -1;
  }
  let Some(store) = current_store() else {
    return -1;
  };
  let Some(shared) = store.borrow::<SharedSystemState>() else {
    return -1;
  };

  let mut strings: Vec<Box<[u8]>> = Vec::new();
  let snapshot = shared.snapshot.lock().unwrap();
  let mut processes = Vec::with_capacity(snapshot.top_processes.len());
  for process in &snapshot.top_processes {
    processes.push(SlProcessInfo {
      pid: process.pid,
      name: stage_str(&mut strings, Some(&process.name)),
      cpu_usage: process.cpu_usage,
      memory: process.memory,
    });
  }

  let mut stage = ServiceStage::System {
    strings,
    processes,
    state: SlSystemState::default(),
  };
  if let ServiceStage::System {
    processes, state, ..
  } = &mut stage
  {
    *state = SlSystemState {
      cpu_usage: snapshot.cpu_usage,
      mem_usage: snapshot.mem_usage,
      mem_total: snapshot.mem_total,
      mem_used: snapshot.mem_used,
      has_temperature: snapshot.temperature.is_some(),
      temperature: snapshot.temperature.unwrap_or(0.0),
      load: snapshot.load,
      network_state: snapshot.network_state,
      network_rx: snapshot.network_rx,
      network_tx: snapshot.network_tx,
      top_processes: processes.as_ptr(),
      process_count: processes.len(),
    };
  }
  write_service_stage(stage, |stage| match stage {
    ServiceStage::System { state, .. } => unsafe { *out = *state },
    _ => {}
  })
}

unsafe extern "C" fn host_tray_state_get(_ctx: *mut c_void, out: *mut SlTrayState) -> i32 {
  if out.is_null() {
    return -1;
  }
  let Some(store) = current_store() else {
    return -1;
  };
  let Some(shared) = store.borrow::<SharedTrayState>() else {
    return -1;
  };

  let mut strings: Vec<Box<[u8]>> = Vec::new();
  let inner = shared.state.lock().unwrap();
  let mut items = Vec::with_capacity(inner.items.len());
  for item in &inner.items {
    let (has_pixmap, width, height, pixmap) = match &item.pixmap {
      Some((width, height, pixels)) => {
        let bytes = pixels.clone().into_boxed_slice();
        let pixmap = SlStr {
          ptr: bytes.as_ptr(),
          len: bytes.len(),
        };
        strings.push(bytes);
        (true, *width, *height, pixmap)
      }
      None => (false, 0, 0, SlStr::EMPTY),
    };
    items.push(SlTrayItem {
      address: stage_str(&mut strings, Some(&item.address)),
      title: stage_str(&mut strings, Some(&item.title)),
      icon_name: stage_str(&mut strings, item.icon_name.as_deref()),
      menu_path: stage_str(&mut strings, item.menu_path.as_deref()),
      has_pixmap,
      pixmap_width: width,
      pixmap_height: height,
      pixmap,
    });
  }

  let mut stage = ServiceStage::Tray {
    strings,
    items,
    state: SlTrayState::default(),
  };
  if let ServiceStage::Tray { items, state, .. } = &mut stage {
    *state = SlTrayState {
      items: items.as_ptr(),
      item_count: items.len(),
    };
  }
  write_service_stage(stage, |stage| match stage {
    ServiceStage::Tray { state, .. } => unsafe { *out = *state },
    _ => {}
  })
}

unsafe extern "C" fn host_power_state_get(_ctx: *mut c_void, out: *mut SlPowerState) -> i32 {
  if out.is_null() {
    return -1;
  }
  let Some(store) = current_store() else {
    return -1;
  };
  let Some(shared) = store.borrow::<SharedPowerState>() else {
    return -1;
  };

  let mut strings: Vec<Box<[u8]>> = Vec::new();
  let data = shared.data.lock().unwrap();

  let status = stage_str(&mut strings, Some(&data.status));
  let time_remaining = stage_str(&mut strings, data.time_remaining.as_deref());
  let active_profile = stage_str(
    &mut strings,
    data.active_profile.as_ref().map(|profile| profile.as_str()),
  );
  let device_name = stage_str(&mut strings, Some(&data.device_name));
  let mut profiles = Vec::with_capacity(data.available_profiles.len());
  for profile in &data.available_profiles {
    profiles.push(stage_str(&mut strings, Some(profile.as_str())));
  }

  let mut stage = ServiceStage::Power {
    strings,
    profiles,
    state: SlPowerState::default(),
  };
  if let ServiceStage::Power {
    profiles, state, ..
  } = &mut stage
  {
    *state = SlPowerState {
      has_percent: data.percent.is_some(),
      percent: data.percent.unwrap_or(0),
      charging: data.charging,
      status,
      has_health: data.health.is_some(),
      health: data.health.unwrap_or(0),
      has_energy_now: data.energy_now_wh.is_some(),
      energy_now_wh: data.energy_now_wh.unwrap_or(0.0),
      has_energy_full: data.energy_full_wh.is_some(),
      energy_full_wh: data.energy_full_wh.unwrap_or(0.0),
      has_power: data.power_w.is_some(),
      power_w: data.power_w.unwrap_or(0.0),
      time_remaining,
      has_active_profile: data.active_profile.is_some(),
      active_profile,
      available_profiles: profiles.as_ptr(),
      available_profile_count: profiles.len(),
      brightness_percent: data.brightness_percent,
      brightness_max: data.brightness_max,
      brightness_current: data.brightness_current,
      device_name,
    };
  }
  write_service_stage(stage, |stage| match stage {
    ServiceStage::Power { state, .. } => unsafe { *out = *state },
    _ => {}
  })
}

#[allow(clippy::single_match)]
fn write_service_stage(stage: ServiceStage, write: impl FnOnce(&ServiceStage)) -> i32 {
  SERVICE_STAGE.with(|cell| *cell.borrow_mut() = Some(stage));
  SERVICE_STAGE.with(|cell| {
    if let Some(stage) = cell.borrow().as_ref() {
      write(stage);
    }
  });
  0
}

#[derive(Default)]
struct StagedCompositor {
  #[allow(dead_code)]
  strings: Vec<Box<[u8]>>,
  monitors: Vec<SlMonitor>,
  workspaces: Vec<SlWorkspace>,
  window: Option<SlWindow>,
  state: SlCompositorState,
}

fn stage_str(strings: &mut Vec<Box<[u8]>>, value: Option<&str>) -> SlStr {
  match value {
    Some(value) => {
      let bytes = value.as_bytes().to_vec().into_boxed_slice();
      let string = SlStr {
        ptr: bytes.as_ptr(),
        len: bytes.len(),
      };
      strings.push(bytes);
      string
    }
    None => SlStr::EMPTY,
  }
}

impl StagedCompositor {
  fn new(state: &CompositorState) -> Self {
    let mut strings = Vec::new();

    let mut monitors = Vec::with_capacity(state.monitors.len());
    for monitor in state.monitors.values() {
      let name = stage_str(&mut strings, Some(&monitor.name));
      monitors.push(SlMonitor {
        name,
        width: monitor.width,
        height: monitor.height,
        scale: monitor.scale as f32,
      });
    }

    let mut workspaces = Vec::with_capacity(state.workspaces.len());
    for workspace in &state.workspaces {
      let output = stage_str(&mut strings, workspace.output.as_deref());
      let name = stage_str(&mut strings, workspace.name.as_deref());
      workspaces.push(SlWorkspace {
        id: workspace.id,
        idx: workspace.idx,
        output,
        name,
        is_active: workspace.is_active,
        is_focused: workspace.is_focused,
        is_urgent: workspace.is_urgent,
      });
    }

    let window = state.active_window.as_ref().map(|window| SlWindow {
      id: window.id,
      title: stage_str(&mut strings, Some(&window.title)),
      wclass: stage_str(&mut strings, Some(&window.class)),
    });

    let mut staged = StagedCompositor {
      strings,
      monitors,
      workspaces,
      window,
      state: SlCompositorState::default(),
    };

    staged.state = SlCompositorState {
      monitors: staged.monitors.as_ptr(),
      monitor_count: staged.monitors.len(),
      workspaces: staged.workspaces.as_ptr(),
      workspace_count: staged.workspaces.len(),
      active_window: staged
        .window
        .as_ref()
        .map_or(core::ptr::null(), |window| window as *const SlWindow),
      overview_active: state.overview_active,
    };
    staged
  }
}

fn theme_to_sl(theme: &Theme) -> SlTheme {
  let color = |color: Color| SlColor {
    r: color.r,
    g: color.g,
    b: color.b,
    a: color.a,
  };

  SlTheme {
    base: color(theme.base),
    crust: color(theme.crust),
    mantle: color(theme.mantle),
    primary: color(theme.primary),
    secondary: color(theme.secondary),
    green: color(theme.green),
    red: color(theme.red),
    blue: color(theme.blue),
    yellow: color(theme.yellow),
    orange: color(theme.orange),
    text: color(theme.text),
    subtext: color(theme.subtext),
    overlay: color(theme.overlay),
  }
}

fn style_to_sheet(style: &Style) -> Vec<SlStyleSheetEntry> {
  style
    .entries()
    .map(|(key, value)| {
      let mut entry = SlStyleSheetEntry {
        key: SlStr::from_str(key),
        ..SlStyleSheetEntry::default()
      };
      match value {
        StyleValue::String(s) => {
          entry.kind = sl_style_value_kind::STRING;
          entry.text = SlStr::from_str(s);
        }
        StyleValue::Integer(i) => {
          entry.kind = sl_style_value_kind::INTEGER;
          entry.integer = *i;
        }
        StyleValue::Float(f) => {
          entry.kind = sl_style_value_kind::FLOAT;
          entry.float_value = *f;
        }
        StyleValue::Boolean(b) => {
          entry.kind = sl_style_value_kind::BOOLEAN;
          entry.boolean = *b;
        }
        StyleValue::Color(color) => {
          entry.kind = sl_style_value_kind::COLOR;
          match color {
            ColorValue::Transparent => entry.color_kind = sl_style_color_kind::TRANSPARENT,
            ColorValue::Theme(name) => {
              entry.color_kind = sl_style_color_kind::THEME;
              entry.text = SlStr::from_str(name);
            }
            ColorValue::Color(hex) => {
              entry.color_kind = sl_style_color_kind::HEX;
              entry.text = SlStr::from_str(hex);
            }
          }
        }
      }
      entry
    })
    .collect()
}

fn sheet_to_style(sheet: &SlStyleSheet) -> Style {
  let mut style = Style::default();
  if sheet.entries.is_null() {
    return style;
  }
  let entries = unsafe { core::slice::from_raw_parts(sheet.entries, sheet.count as usize) };
  for entry in entries {
    let Some(key) = (unsafe { entry.key.as_str() }) else {
      continue;
    };
    let value = match entry.kind {
      sl_style_value_kind::COLOR => {
        let color = match entry.color_kind {
          sl_style_color_kind::THEME => {
            ColorValue::Theme((unsafe { entry.text.as_str() }).unwrap_or("").to_owned())
          }
          sl_style_color_kind::HEX => {
            ColorValue::Color((unsafe { entry.text.as_str() }).unwrap_or("").to_owned())
          }
          _ => ColorValue::Transparent,
        };
        StyleValue::Color(color)
      }
      sl_style_value_kind::INTEGER => StyleValue::Integer(entry.integer),
      sl_style_value_kind::FLOAT => StyleValue::Float(entry.float_value),
      sl_style_value_kind::BOOLEAN => StyleValue::Boolean(entry.boolean),
      _ => StyleValue::String((unsafe { entry.text.as_str() }).unwrap_or("").to_owned()),
    };
    style.insert(key, value);
  }
  style
}

static HOST_API: SlHostApi = SlHostApi {
  size: std::mem::size_of::<SlHostApi>() as u32,
  register_component: Some(host_register_component),
  register_compositor: Some(host_register_compositor),
  request_redraw: Some(host_request_redraw),
  register_fd: Some(host_register_fd),
  log: Some(host_log),
  get_str: Some(host_get_str),
  get_f64: Some(host_get_f64),
  get_i64: Some(host_get_i64),
  get_bool: Some(host_get_bool),
  set_compositor_state: Some(host_set_compositor_state),
  set_interval: Some(host_set_interval),
  register_payload: Some(host_register_payload),
  register_renderable: Some(host_register_renderable),
  register_spotlight: Some(host_register_spotlight),
  register_desktop_item: Some(host_register_desktop_item),
  canvas: Some(host_canvas_alloc),
  config_get_str: Some(host_config_get_str),
  config_get_f64: Some(host_config_get_f64),
  config_get_i64: Some(host_config_get_i64),
  config_get_bool: Some(host_config_get_bool),
  style_get: Some(host_style_get),
  theme_get: Some(host_theme_get),
  style_register: Some(host_style_register),
  compositor_state_get: Some(host_compositor_state_get),
  config_parser_register: Some(host_config_parser_register),
  plugin_error: Some(host_plugin_error),
  notify: Some(host_notify),
  compositor_register_fd: Some(host_compositor_register_fd),
  registry_register: Some(host_registry_register),
  registry_get: Some(host_registry_get),
  registry_remove: Some(host_registry_remove),
  registry_subscribe: Some(host_registry_subscribe),
  dispatch: Some(host_dispatch),
  audio_state_get: Some(host_audio_state_get),
  bluetooth_state_get: Some(host_bluetooth_state_get),
  network_state_get: Some(host_network_state_get),
  system_state_get: Some(host_system_state_get),
  tray_state_get: Some(host_tray_state_get),
  power_state_get: Some(host_power_state_get),
  persistence_get: Some(host_persistence_get),
  persistence_set: Some(host_persistence_set),
  persistence_remove: Some(host_persistence_remove),
  dispatch_with_string: Some(host_dispatch_with_string),
  register_command: Some(host_register_command),
};

unsafe extern "C" fn host_register_component(
  _host: *mut c_void,
  name: SlStr,
  vtable: *const SlComponentVtable,
  _userdata: *mut c_void,
) {
  let name = unsafe { name.as_str() }.unwrap_or("").to_owned();
  PENDING.with(|pending| {
    pending
      .borrow_mut()
      .push(Pending::Component { name, vtable });
  });
}

unsafe extern "C" fn host_register_compositor(
  _host: *mut c_void,
  name: SlStr,
  vtable: *const SlCompositorVtable,
  _userdata: *mut c_void,
) {
  let name = unsafe { name.as_str() }.unwrap_or("").to_owned();
  PENDING.with(|pending| {
    pending
      .borrow_mut()
      .push(Pending::Compositor { name, vtable });
  });
}

unsafe extern "C" fn host_register_payload(
  _host: *mut c_void,
  command: SlStr,
  vtable: *const SlPayloadVtable,
  _userdata: *mut c_void,
) {
  let command = unsafe { command.as_str() }.unwrap_or("").to_owned();
  PENDING.with(|pending| {
    pending
      .borrow_mut()
      .push(Pending::Payload { command, vtable });
  });
}

unsafe extern "C" fn host_register_renderable(
  _host: *mut c_void,
  name: SlStr,
  vtable: *const SlRenderableVtable,
  _userdata: *mut c_void,
) {
  let name = unsafe { name.as_str() }.unwrap_or("").to_owned();
  PENDING.with(|pending| {
    pending
      .borrow_mut()
      .push(Pending::Renderable { name, vtable });
  });
}

unsafe extern "C" fn host_register_spotlight(
  _host: *mut c_void,
  name: SlStr,
  vtable: *const SlSpotlightVtable,
  _userdata: *mut c_void,
) {
  let name = unsafe { name.as_str() }.unwrap_or("").to_owned();
  PENDING.with(|pending| {
    pending
      .borrow_mut()
      .push(Pending::Spotlight { name, vtable });
  });
}

unsafe extern "C" fn host_register_desktop_item(
  _host: *mut c_void,
  name: SlStr,
  vtable: *const SlDesktopItemVtable,
  _userdata: *mut c_void,
) {
  let name = unsafe { name.as_str() }.unwrap_or("").to_owned();
  PENDING.with(|pending| {
    pending
      .borrow_mut()
      .push(Pending::DesktopItem { name, vtable });
  });
}

unsafe extern "C" fn host_register_command(
  _host: *mut c_void,
  name: SlStr,
  title: SlStr,
  description: SlStr,
) -> i32 {
  let Some(name) = (unsafe { name.as_str() }) else {
    return -1;
  };
  let title = unsafe { title.as_str() }
    .filter(|title| !title.is_empty())
    .map(str::to_owned);
  let description = unsafe { description.as_str() }
    .filter(|description| !description.is_empty())
    .map(str::to_owned);

  let name = name.to_owned();
  PENDING.with(|pending| {
    pending.borrow_mut().push(Pending::Command {
      name,
      title,
      description,
    });
  });

  0
}

unsafe extern "C" fn host_request_redraw(_ctx: *mut c_void, _window: u64) {
  CURRENT_CALLS.with(|calls| {
    if let Some(calls) = calls.borrow_mut().as_mut() {
      calls.redraw = true;
    }
  });
}

unsafe extern "C" fn host_register_fd(_ctx: *mut c_void, fd: i32, action: u32) -> i32 {
  if fd < 0 {
    return -1;
  }

  CURRENT_CALLS.with(|calls| {
    if let Some(calls) = calls.borrow_mut().as_mut() {
      calls.fds.push((fd, action));
      0
    } else {
      -1
    }
  })
}

unsafe extern "C" fn host_set_interval(
  _ctx: *mut c_void,
  millis: u32,
  repeating: bool,
  tag: u32,
) -> i32 {
  CURRENT_CALLS.with(|calls| {
    if let Some(calls) = calls.borrow_mut().as_mut() {
      calls.timers.push((millis, repeating, tag));
      0
    } else {
      -1
    }
  })
}

unsafe extern "C" fn host_log(ctx: *mut c_void, _level: i32, message: SlStr) {
  let plugin = if ctx.is_null() {
    "?"
  } else {
    unsafe { &*(ctx as *const PluginInstance) }
      .plugin_id
      .as_str()
  };

  if let Some(message) = unsafe { message.as_str() } {
    eprintln!("[plugin:{plugin}] {message}");
  }
}

unsafe extern "C" fn host_plugin_error(ctx: *mut c_void, code: u32, message: SlStr) {
  let plugin = if ctx.is_null() {
    "?".to_owned()
  } else {
    unsafe { &*(ctx as *const PluginInstance) }
      .plugin_id
      .clone()
  };
  let message = unsafe { message.as_str() }.unwrap_or_default();
  record_plugin_error(&plugin, code, message);
}

static NEXT_NOTIFICATION_ID: AtomicU32 = AtomicU32::new(1_000_000);

unsafe extern "C" fn host_notify(ctx: *mut c_void, notification: *const SlNotification) -> i32 {
  let Some(notification) = (unsafe { notification.as_ref() }) else {
    return -1;
  };
  let Some(shared) = current_notifications() else {
    return -1;
  };

  let plugin_id = if ctx.is_null() {
    None
  } else {
    Some(
      unsafe { &*(ctx as *const PluginInstance) }
        .plugin_id
        .clone(),
    )
  };

  let summary = unsafe { notification.summary.as_str() }
    .unwrap_or_default()
    .to_owned();
  let body = unsafe { notification.body.as_str() }
    .unwrap_or_default()
    .to_owned();
  let app_name = unsafe { notification.app_name.as_str() }
    .map(str::to_owned)
    .or_else(|| plugin_id.clone())
    .unwrap_or_else(|| "Plugin".to_owned());
  let app_icon = unsafe { notification.app_icon.as_str() }.map(str::to_owned);

  let mut actions = Vec::new();
  if !notification.actions.is_null() {
    for index in 0..notification.action_count {
      let action = unsafe { &*notification.actions.add(index) };
      let (Some(command), Some(label)) = (unsafe { action.command.as_str() }, unsafe {
        action.label.as_str()
      }) else {
        continue;
      };
      actions.push((command.to_owned(), label.to_owned()));
    }
  }

  let image = app_icon.as_deref().and_then(|icon| {
    let path = std::path::PathBuf::from(icon.strip_prefix("file://").unwrap_or(icon));
    path.is_absolute().then_some(NotificationImage::Path(path))
  });

  let id = NEXT_NOTIFICATION_ID.fetch_add(1, Ordering::Relaxed);
  let item = NotificationItem {
    id,
    app_name,
    app_icon,
    image,
    summary,
    body,
    actions,
    urgency: notification.urgency,
    created_at: std::time::Instant::now(),
    plugin: plugin_id,
  };

  let is_dnd = shared.dnd.load(Ordering::Relaxed);
  {
    let mut state = shared.state.lock().unwrap();
    state.history.retain(|n| n.id != id);
    state.history.insert(0, item.clone());
    state.unread_count += 1;
    if !is_dnd {
      state.popups.retain(|n| n.id != id);
      state.popups.push(item);
    }
  }
  shared.bump();

  let timeout = if notification.timeout_ms < 0 {
    5000
  } else {
    notification.timeout_ms as u64
  };
  if timeout > 0 {
    let _ = shared
      .cmd_tx
      .send(NotificationCmd::ExpireAfter(id, timeout));
  }

  id as i32
}

pub enum Bag<'a> {
  #[cfg(feature = "panels")]
  Options(Option<&'a ComponentOptions>),
  #[cfg(not(feature = "panels"))]
  Options(Option<()>),
  Monitor(&'a str),
}

#[cfg(feature = "panels")]
impl<'a> Bag<'a> {
  fn get_str(&self, key: &str) -> Option<&'a str> {
    match self {
      Bag::Options(options) => options.and_then(|options| options.str(key)),
      Bag::Monitor(monitor) => (key == "monitor").then_some(*monitor),
    }
  }

  fn get_i64(&self, key: &str) -> Option<i64> {
    match self {
      Bag::Options(options) => options.and_then(|options| options.int(key)).map(i64::from),
      Bag::Monitor(_) => None,
    }
  }

  fn get_f64(&self, key: &str) -> Option<f64> {
    match self {
      Bag::Options(options) => options
        .and_then(|options| options.number(key))
        .map(f64::from),
      Bag::Monitor(_) => None,
    }
  }

  fn get_bool(&self, key: &str) -> Option<bool> {
    match self {
      Bag::Options(options) => options.and_then(|options| options.bool(key)),
      Bag::Monitor(_) => None,
    }
  }
}

unsafe extern "C" fn host_get_str(bag: *mut c_void, key: SlStr, out: *mut SlStr) -> i32 {
  if bag.is_null() || out.is_null() {
    return -1;
  }

  #[cfg(not(feature = "panels"))]
  let _ = key;

  #[cfg(feature = "panels")]
  let bag = unsafe { &*(bag as *const Bag) };
  #[cfg(feature = "panels")]
  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };
  #[cfg(feature = "panels")]
  let Some(value) = bag.get_str(key) else {
    return -1;
  };

  #[cfg(feature = "panels")]
  unsafe {
    *out = SlStr::from_str(value)
  };
  0
}

unsafe extern "C" fn host_get_f64(bag: *mut c_void, key: SlStr, out: *mut f64) -> i32 {
  if bag.is_null() || out.is_null() {
    return -1;
  }

  #[cfg(not(feature = "panels"))]
  let _ = key;

  #[cfg(feature = "panels")]
  let bag = unsafe { &*(bag as *const Bag) };
  #[cfg(feature = "panels")]
  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };

  #[cfg(feature = "panels")]
  let Some(value) = bag.get_f64(key) else {
    return -1;
  };

  #[cfg(feature = "panels")]
  unsafe {
    *out = value
  };
  0
}

unsafe extern "C" fn host_get_i64(bag: *mut c_void, key: SlStr, out: *mut i64) -> i32 {
  if bag.is_null() || out.is_null() {
    return -1;
  }

  #[cfg(not(feature = "panels"))]
  let _ = key;

  #[cfg(feature = "panels")]
  let bag = unsafe { &*(bag as *const Bag) };
  #[cfg(feature = "panels")]
  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };
  #[cfg(feature = "panels")]
  let Some(value) = bag.get_i64(key) else {
    return -1;
  };

  #[cfg(feature = "panels")]
  unsafe {
    *out = value
  };
  0
}

unsafe extern "C" fn host_get_bool(bag: *mut c_void, key: SlStr, out: *mut bool) -> i32 {
  if bag.is_null() || out.is_null() {
    return -1;
  }

  #[cfg(not(feature = "panels"))]
  let _ = key;

  #[cfg(feature = "panels")]
  let bag = unsafe { &*(bag as *const Bag) };
  #[cfg(feature = "panels")]
  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };
  #[cfg(feature = "panels")]
  let Some(value) = bag.get_bool(key) else {
    return -1;
  };

  #[cfg(feature = "panels")]
  unsafe {
    *out = value
  };
  0
}

unsafe extern "C" fn host_config_get_str(ctx: *mut c_void, key: SlStr, out: *mut SlStr) -> i32 {
  if out.is_null() {
    return -1;
  }

  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };
  let Some(value) = plugin_config_value(ctx, key) else {
    return -1;
  };

  CONFIG_STAGING.with(|cell| {
    let mut staged = cell.borrow_mut();
    *staged = Some(value.into_boxed_str());
    let value = staged.as_ref().expect("just stored");
    unsafe {
      *out = SlStr {
        ptr: value.as_ptr(),
        len: value.len(),
      }
    };
  });
  0
}

unsafe extern "C" fn host_config_get_i64(ctx: *mut c_void, key: SlStr, out: *mut i64) -> i32 {
  if out.is_null() {
    return -1;
  }

  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };
  let Some(value) = plugin_config_value(ctx, key).and_then(|value| value.parse().ok()) else {
    return -1;
  };

  unsafe { *out = value };
  0
}

unsafe extern "C" fn host_config_get_f64(ctx: *mut c_void, key: SlStr, out: *mut f64) -> i32 {
  if out.is_null() {
    return -1;
  }

  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };
  let Some(value) = plugin_config_value(ctx, key).and_then(|value| value.parse().ok()) else {
    return -1;
  };

  unsafe { *out = value };
  0
}

unsafe extern "C" fn host_config_get_bool(ctx: *mut c_void, key: SlStr, out: *mut bool) -> i32 {
  if out.is_null() {
    return -1;
  }

  let Some(key) = (unsafe { key.as_str() }) else {
    return -1;
  };
  let Some(value) = plugin_config_value(ctx, key).and_then(|value| value.parse().ok()) else {
    return -1;
  };

  unsafe { *out = value };
  0
}

unsafe extern "C" fn host_style_get(_ctx: *mut c_void, name: SlStr, out: *mut SlStyleSheet) -> i32 {
  let Some(config) = current_config() else {
    return -1;
  };
  let Some(name) = (unsafe { name.as_str() }) else {
    return -1;
  };
  if !config.has_style(name) {
    return -1;
  }

  let style = config.style(name);
  STYLE_STAGING.with(|cell| {
    let mut staging = cell.borrow_mut();
    *staging = Some(StagedStyle {
      entries: style_to_sheet(&style),
      style,
    });
    let staged = staging.as_ref().expect("just stored");
    unsafe {
      *out = SlStyleSheet {
        entries: staged.entries.as_ptr(),
        count: staged.entries.len() as u32,
      };
    }
  });
  0
}

unsafe extern "C" fn host_theme_get(_ctx: *mut c_void, out: *mut SlTheme) -> i32 {
  let Some(config) = current_config() else {
    return -1;
  };
  if out.is_null() {
    return -1;
  }
  unsafe { *out = theme_to_sl(&config.theme) };
  0
}

unsafe extern "C" fn host_style_register(
  _ctx: *mut c_void,
  name: SlStr,
  sheet: *const SlStyleSheet,
) -> i32 {
  let Some(name) = (unsafe { name.as_str() }) else {
    return -1;
  };
  if sheet.is_null() {
    return -1;
  }
  let style = sheet_to_style(unsafe { &*sheet });
  new_default_style(Ustr::from(name), style);

  CURRENT_PLUGIN.with(|current| {
    if let Some(plugin_id) = current.borrow().as_deref() {
      contribute(plugin_id, |contributions| {
        contributions.styles.push(name.to_owned());
      });
    }
  });

  0
}

unsafe extern "C" fn host_set_compositor_state(ctx: *mut c_void, state: *const SlCompositorState) {
  if ctx.is_null() || state.is_null() {
    return;
  }

  let instance = unsafe { &mut *(ctx as *mut PluginInstance) };
  instance.compositor_state = unsafe { convert_compositor_state(&*state) };
}

unsafe extern "C" fn host_compositor_state_get(
  _ctx: *mut c_void,
  out: *mut SlCompositorState,
) -> i32 {
  if out.is_null() {
    return -1;
  }

  let Some(state) = current_compositor() else {
    return -1;
  };

  COMPOSITOR_STAGING.with(|cell| {
    let mut staging = cell.borrow_mut();
    *staging = Some(StagedCompositor::new(state));
    let staged = staging.as_ref().expect("just stored");
    unsafe { *out = staged.state };
  });
  0
}

unsafe extern "C" fn host_compositor_register_fd(_ctx: *mut c_void, fd: i32, flags: u32) -> i32 {
  if fd < 0 {
    return -1;
  }
  let Some(listeners) = CURRENT_LISTENERS.with(|cell| *cell.borrow()) else {
    return -1;
  };
  let listeners = unsafe { &mut *listeners };

  let flags = if flags == 0 {
    EpollFlags::EPOLLIN | EpollFlags::EPOLLET
  } else {
    let mut mapped = EpollFlags::empty();
    if flags & sl_epoll::IN != 0 {
      mapped |= EpollFlags::EPOLLIN;
    }
    if flags & sl_epoll::ET != 0 {
      mapped |= EpollFlags::EPOLLET;
    }
    mapped
  };

  listeners.flag(fd, flags);
  listeners.action(fd, ListenerAction::UpdateCompositor);
  0
}

unsafe fn convert_compositor_state(state: &SlCompositorState) -> CompositorState {
  let mut monitors = HashMap::new();
  for index in 0..state.monitor_count {
    let monitor = unsafe { &*state.monitors.add(index) };
    let Some(name) = (unsafe { monitor.name.as_str() }) else {
      continue;
    };
    monitors.insert(
      Ustr::from(name),
      Monitor {
        name: name.into(),
        active_workspace: 0,
        width: monitor.width,
        height: monitor.height,
        scale: monitor.scale as f64,
      },
    );
  }

  let mut workspaces = Vec::with_capacity(state.workspace_count);
  for index in 0..state.workspace_count {
    let workspace = unsafe { &*state.workspaces.add(index) };
    workspaces.push(Workspace {
      id: workspace.id,
      idx: workspace.idx,
      name: unsafe { workspace.name.as_str() }.map(str::to_owned),
      output: unsafe { workspace.output.as_str() }.map(str::to_owned),
      is_active: workspace.is_active,
      is_focused: workspace.is_focused,
      is_urgent: workspace.is_urgent,
    });
  }

  let active_window = if state.active_window.is_null() {
    None
  } else {
    let window = unsafe { &*state.active_window };
    Some(Window {
      id: window.id,
      title: unsafe { window.title.as_str() }
        .unwrap_or_default()
        .to_owned(),
      class: unsafe { window.wclass.as_str() }
        .unwrap_or_default()
        .to_owned(),
      is_active: true,
      metadata: HashMap::new(),
    })
  };

  CompositorState {
    monitors,
    active_window,
    active_windows: Vec::new(),
    workspaces,
    overview_active: state.overview_active,
  }
}

#[derive(Default)]
struct PluginCalls {
  timers: Vec<(u32, bool, u32)>,
  fds: Vec<(i32, u32)>,
  redraw: bool,
}

thread_local! {
  static CURRENT_CALLS: RefCell<Option<PluginCalls>> = const { RefCell::new(None) };
}

#[derive(Default)]
struct CanvasPool {
  buffers: Vec<Vec<u8>>,
  in_use: usize,
}

thread_local! {
  static CANVAS_POOL: RefCell<CanvasPool> = RefCell::new(CanvasPool::default());
}

fn reset_canvas_pool() {
  CANVAS_POOL.with(|pool| pool.borrow_mut().in_use = 0);
}

#[derive(Default)]
struct ImageCache {
  ctx: usize,
  entries: HashMap<(usize, u64), image_widget::Handle>,
  seen: HashSet<(usize, u64)>,
}

thread_local! {
  static IMAGE_CACHE: RefCell<ImageCache> = RefCell::new(ImageCache::default());
}

fn image_cache_begin(ctx: usize) {
  IMAGE_CACHE.with(|cache| {
    let mut cache = cache.borrow_mut();
    cache.ctx = ctx;
    cache.seen.clear();
  });
}

fn image_cache_end() {
  IMAGE_CACHE.with(|cache| {
    let mut cache = cache.borrow_mut();
    let ImageCache { ctx, entries, seen } = &mut *cache;
    let ctx = *ctx;
    entries.retain(|key, _| key.0 != ctx || seen.contains(key));
  });
}

fn image_cache_get(key: u64) -> Option<image_widget::Handle> {
  IMAGE_CACHE.with(|cache| {
    let mut cache = cache.borrow_mut();
    let id = (cache.ctx, key);
    cache.seen.insert(id);
    cache.entries.get(&id).cloned()
  })
}

fn image_cache_put(key: u64, handle: image_widget::Handle) {
  IMAGE_CACHE.with(|cache| {
    let mut cache = cache.borrow_mut();
    let id = (cache.ctx, key);
    cache.seen.insert(id);
    cache.entries.insert(id, handle);
  })
}

fn image_hash_key(bytes: &[u8]) -> u64 {
  let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
  for byte in bytes {
    hash ^= u64::from(*byte);
    hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
  }
  hash | (1u64 << 63)
}

unsafe extern "C" fn host_canvas_alloc(_ctx: *mut c_void, width: u32, height: u32) -> *mut u8 {
  if width == 0 || height == 0 {
    return std::ptr::null_mut();
  }

  let len = width as usize * height as usize * 4;

  CANVAS_POOL.with(|pool| {
    let mut pool = pool.borrow_mut();
    let index = pool.in_use;

    if index < pool.buffers.len() {
      let buffer = &mut pool.buffers[index];
      if buffer.len() < len {
        buffer.resize(len, 0);
      }
      let ptr = buffer.as_mut_ptr();
      pool.in_use = index + 1;
      ptr
    } else {
      let mut buffer = vec![0u8; len];
      let ptr = buffer.as_mut_ptr();
      pool.buffers.push(buffer);
      pool.in_use = index + 1;
      ptr
    }
  })
}

fn with_plugin_calls<R>(f: impl FnOnce() -> R) -> (R, PluginCalls) {
  CURRENT_CALLS.with(|calls| *calls.borrow_mut() = Some(PluginCalls::default()));
  let result = f();
  let calls = CURRENT_CALLS
    .with(|calls| calls.borrow_mut().take())
    .unwrap_or_default();
  (result, calls)
}

static NEXT_INSTANCE_ID: AtomicU64 = AtomicU64::new(1);

#[cfg(feature = "panels")]
struct PluginComponent {
  ctx: *mut c_void,
  vtable: *const SlComponentVtable,
  state: *mut c_void,
  event_name: Ustr,
  timers: Vec<(i32, u32)>,
  fds: Vec<(i32, u32)>,
}

#[cfg(feature = "panels")]
unsafe impl Send for PluginComponent {}

#[cfg(feature = "panels")]
impl PluginComponent {
  fn new(ctx: *mut c_void, vtable: *const SlComponentVtable, state: *mut c_void) -> Self {
    let id = NEXT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed);
    Self {
      ctx,
      vtable,
      state,
      event_name: format!("component/plugin.{id}").into(),
      timers: Vec::new(),
      fds: Vec::new(),
    }
  }

  fn apply_calls(&mut self, store: &mut Store, calls: PluginCalls) -> bool {
    apply_plugin_calls(
      &self.event_name,
      &mut self.timers,
      &mut self.fds,
      store,
      calls,
    )
  }
}

fn apply_plugin_calls(
  event_name: &Ustr,
  timers: &mut Vec<(i32, u32)>,
  fds: &mut Vec<(i32, u32)>,
  store: &mut Store,
  calls: PluginCalls,
) -> bool {
  let redraw = calls.redraw;

  let Some(handle) = store.borrow::<FdHandle>() else {
    return redraw;
  };

  for (millis, repeating, tag) in calls.timers {
    let duration = Duration::from_millis(millis as u64);
    let result = if repeating {
      handle.set_named_interval(duration, event_name.clone())
    } else {
      handle.set_named_timer(duration, event_name.clone())
    };

    if let Ok(fd) = result {
      timers.push((fd, tag));
    }
  }

  for (fd, action) in calls.fds {
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    let raw = owned.as_raw_fd();
    handle.watch(
      owned,
      ListenerAction::Signal {
        name: event_name.clone(),
        fd: raw,
      },
    );
    fds.push((raw, action));
  }

  redraw
}

fn clear_plugin_calls(timers: &mut Vec<(i32, u32)>, fds: &mut Vec<(i32, u32)>, store: &Store) {
  let Some(handle) = store.borrow::<FdHandle>() else {
    return;
  };
  for (fd, _) in timers.drain(..) {
    handle.stop_timer(fd);
  }
  for (fd, _) in fds.drain(..) {
    handle.stop_timer(fd);
  }
}

#[cfg(feature = "panels")]
impl Drop for PluginComponent {
  fn drop(&mut self) {
    if let Some(destroy) = unsafe { (*self.vtable).destroy } {
      unsafe { destroy(self.ctx, self.state) };
    }
  }
}

#[cfg(feature = "panels")]
impl Component for PluginComponent {
  fn events(&self) -> Vec<EventFilter> {
    let Some(events) = (unsafe { (*self.vtable).events }) else {
      return Vec::new();
    };

    let mask = unsafe { events(self.ctx, self.state) };
    let mut filters = filters_from_mask(mask);

    if mask & (sl_event_mask::TICK | sl_event_mask::FD) != 0 {
      filters.push(EventFilter::Named(self.event_name.clone()));
    }

    filters
  }

  fn check_view(&self, store: &Store, options: Option<&ComponentOptions>) -> bool {
    let Some(check_view) = (unsafe { (*self.vtable).check_view }) else {
      return true;
    };

    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope(store);
    let mut bag = Bag::Options(options);
    unsafe { check_view(self.ctx, self.state, &mut bag as *mut _ as *mut c_void) }
  }

  fn watch(&mut self, store: &mut Store, options: Option<&ComponentOptions>) {
    clear_plugin_calls(&mut self.timers, &mut self.fds, store);
    let _store_scope = store_scope(store);
    let Some(watch) = (unsafe { (*self.vtable).watch }) else {
      return;
    };

    let (_, calls) = with_plugin_calls(|| {
      let mut bag = Bag::Options(options);
      unsafe { watch(self.ctx, self.state, &mut bag as *mut _ as *mut c_void) };
    });
    self.apply_calls(store, calls);
  }

  fn stop(&mut self, store: &Store, _options: Option<&ComponentOptions>) {
    clear_plugin_calls(&mut self.timers, &mut self.fds, store);
    let _store_scope = store_scope(store);
    if let Some(stop) = unsafe { (*self.vtable).stop } {
      unsafe { stop(self.ctx, self.state) };
    }
  }

  fn update(
    &mut self,
    config: &Config,
    store: &mut Store,
    event: &ListenerAction,
    _options: Option<&ComponentOptions>,
  ) -> miette::Result<ItemEffect> {
    let _config_scope = ScopedConfig::current(config);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope(store);
    let Some(update) = (unsafe { (*self.vtable).update }) else {
      return Ok(ItemEffect::None);
    };

    let targeted = match event {
      ListenerAction::Timer { name, .. } | ListenerAction::Signal { name, .. } => {
        &**name == &*self.event_name
      }
      _ => false,
    };

    let mut effect_sl = SlEffect::default();
    let (_, calls) = with_plugin_calls(|| {
      if targeted {
        let (kind, action, fd) = match event {
          ListenerAction::Signal { fd, .. } => {
            let action = self
              .fds
              .iter()
              .find(|(registered, _)| registered == fd)
              .map(|(_, action)| *action)
              .unwrap_or(0);
            (sl_event_kind::FD, action, *fd)
          }
          ListenerAction::Timer { fd, .. } => {
            let tag = self
              .timers
              .iter()
              .find(|(registered, _)| registered == fd)
              .map(|(_, tag)| *tag)
              .unwrap_or(0);
            (sl_event_kind::TICK, tag, *fd)
          }
          _ => (sl_event_kind::TICK, 0, -1),
        };

        let event = SlEvent {
          kind,
          name: SlStr::from_str(&self.event_name),
          bag: std::ptr::null_mut(),
          action,
          fd,
        };
        effect_sl = unsafe { update(self.ctx, self.state, &event) };
      } else {
        let event = sl_event_from(event);
        effect_sl = unsafe { update(self.ctx, self.state, &event) };
      }
    });

    let redraw = self.apply_calls(store, calls);
    if effect_sl.code == sl_effect::SUBSCRIBE {
      return Ok(ItemEffect::Subscribe(self.events()));
    }
    let mut effect = effect_from_sl(effect_sl);
    if effect == ItemEffect::None && (targeted || redraw) {
      effect = ItemEffect::Redraw;
    }
    Ok(effect)
  }

  fn view<'a>(
    &self,
    config: &Config,
    store: &Store,
    ctx: &ComponentContext,
    options: Option<&ComponentOptions>,
  ) -> iced::Element<'a, ItemMessage> {
    let _config_scope = ScopedConfig::current(config);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope(store);
    reset_canvas_pool();

    let mut list = SlNodeList::EMPTY;
    if let Some(view) = unsafe { (*self.vtable).view } {
      let mut bag = Bag::Options(options);
      unsafe {
        view(
          self.ctx,
          self.state,
          &mut bag as *mut _ as *mut c_void,
          &mut list,
        )
      };
    }

    let rendered = unsafe { render_nodes(&list, None, self.ctx as usize) };
    spaced_component(config, ctx, rendered, unsafe { (*self.vtable).hoverable })
  }
}

fn to_color(color: SlColor) -> Color {
  Color::from_rgba(color.r, color.g, color.b, color.a)
}

fn alignment(align: u32) -> Alignment {
  match align {
    sl_align::CENTER => Alignment::Center,
    sl_align::END => Alignment::End,
    _ => Alignment::Start,
  }
}

fn size(sl: SlLength) -> Length {
  match sl.unit {
    sl_length_unit::FILL => Length::Fill,
    sl_length_unit::FIXED => Length::Fixed(sl.value),
    _ => Length::Shrink,
  }
}

unsafe fn render_nodes(
  list: &SlNodeList,
  id: Option<iced_layershell::reexport::IcedId>,
  cache_ctx: usize,
) -> iced::Element<'static, ItemMessage> {
  image_cache_begin(cache_ctx);
  let element = unsafe { render_nodes_inner(list, id) };
  image_cache_end();
  element
}

unsafe fn render_nodes_inner(
  list: &SlNodeList,
  id: Option<iced_layershell::reexport::IcedId>,
) -> iced::Element<'static, ItemMessage> {
  if list.nodes.is_null() || list.len == 0 {
    return iced::widget::space().into();
  }

  if list.len == 1 {
    return unsafe { render_node(&*list.nodes, id) };
  }

  let children: Vec<iced::Element<'static, ItemMessage>> = (0..list.len)
    .map(|index| unsafe { render_node(&*list.nodes.add(index), id) })
    .collect();

  column(children).into()
}

unsafe fn render_node(
  node: &SlNode,
  id: Option<iced_layershell::reexport::IcedId>,
) -> iced::Element<'static, ItemMessage> {
  let animation = node.animation;
  let sliding = animation.kind != sl_animation_kind::NONE;

  if !animation.tween {
    let element = unsafe { render_plain(node, id) };
    return if sliding {
      slide_element(element, animation)
    } else {
      element
    };
  }

  let mode = animation_mode(animation);
  let base = *node;
  let text = unsafe { node.text.as_str() }.unwrap_or_default().to_owned();
  let action = unsafe { node.action.as_str() }
    .unwrap_or_default()
    .to_owned();

  let element: iced::Element<'static, ItemMessage> = match node.kind {
    sl_node_kind::PROGRESS => {
      let target = node.value;
      Animated::new(target, move |value| {
        let mut local = base;
        local.value = *value;
        local.text = borrow_str(&text);
        local.action = borrow_str(&action);
        unsafe { render_plain(&local, id) }
      })
      .animation(mode)
      .animates_layout(true)
      .into()
    }

    sl_node_kind::TEXT => {
      let target = if node.style.has_text_color {
        to_color(node.style.text_color)
      } else {
        Color::WHITE
      };
      Animated::new(target, move |color| {
        let mut local = base;
        local.style.has_text_color = true;
        local.style.text_color = sl_color(*color);
        local.text = borrow_str(&text);
        local.action = borrow_str(&action);
        unsafe { render_plain(&local, id) }
      })
      .animation(mode)
      .into()
    }

    sl_node_kind::ICON if node.style.has_text_color => {
      let target = to_color(node.style.text_color);
      Animated::new(target, move |color| {
        let mut local = base;
        local.style.has_text_color = true;
        local.style.text_color = sl_color(*color);
        local.text = borrow_str(&text);
        local.action = borrow_str(&action);
        unsafe { render_plain(&local, id) }
      })
      .animation(mode)
      .into()
    }

    _ => unsafe { render_plain(node, id) },
  };

  if sliding {
    slide_element(element, animation)
  } else {
    element
  }
}

fn borrow_str(text: &str) -> SlStr {
  SlStr {
    ptr: text.as_ptr(),
    len: text.len(),
  }
}

fn sl_color(color: Color) -> SlColor {
  SlColor {
    r: color.r,
    g: color.g,
    b: color.b,
    a: color.a,
  }
}

fn animation_mode(animation: SlAnimation) -> Mode {
  let easing = match animation.easing {
    sl_animation_easing::LINEAR => Easing::LINEAR,
    sl_animation_easing::EASE => Easing::EASE,
    sl_animation_easing::EASE_IN => Easing::EASE_IN,
    sl_animation_easing::EASE_OUT => Easing::EASE_OUT,
    sl_animation_easing::EASE_IN_OUT => Easing::EASE_IN_OUT,
    _ => Easing::EASE_OUT,
  };

  easing
    .with_duration(Duration::from_millis(animation.duration_ms.max(1) as u64))
    .into()
}

fn slide_element(
  element: iced::Element<'static, ItemMessage>,
  animation: SlAnimation,
) -> iced::Element<'static, ItemMessage> {
  let offset = Vector::new(animation.offset_x, animation.offset_y);
  SlideIn::new(element, offset)
    .animation(animation_mode(animation))
    .into()
}

unsafe fn render_plain(
  node: &SlNode,
  id: Option<iced_layershell::reexport::IcedId>,
) -> iced::Element<'static, ItemMessage> {
  let body: iced::Element<'static, ItemMessage> = match node.kind {
    sl_node_kind::ROW | sl_node_kind::COLUMN => {
      let children: Vec<iced::Element<'static, ItemMessage>> = if node.children.is_null() {
        Vec::new()
      } else {
        (0..node.child_count)
          .map(|index| unsafe { render_node(&*node.children.add(index), id) })
          .collect()
      };

      if node.kind == sl_node_kind::ROW {
        row(children)
          .spacing(node.spacing)
          .align_y(alignment(node.align_y))
          .into()
      } else {
        column(children)
          .spacing(node.spacing)
          .align_x(alignment(node.align_x))
          .into()
      }
    }

    sl_node_kind::ICON => {
      let name = unsafe { node.text.as_str() }.unwrap_or_default().to_owned();
      let mut icon = Icon::<ItemMessage>::new(name).size(node.size.max(1.0) as u16);
      if node.style.has_text_color {
        icon = icon.color(to_color(node.style.text_color));
      }
      icon.into()
    }

    sl_node_kind::TEXT => {
      let content = unsafe { node.text.as_str() }.unwrap_or_default().to_owned();
      let color = if node.style.has_text_color {
        to_color(node.style.text_color)
      } else {
        Color::WHITE
      };
      text(content).size(node.size).color(color).into()
    }

    sl_node_kind::PROGRESS => progress_element(node),
    sl_node_kind::CANVAS => unsafe { canvas_element(node) },
    sl_node_kind::IMAGE => unsafe { image_element(node) },

    sl_node_kind::CONTAINER => {
      if node.children.is_null() || node.child_count == 0 {
        iced::widget::space().into()
      } else {
        unsafe { render_node(&*node.children, id) }
      }
    }

    sl_node_kind::SCROLLABLE => {
      if node.children.is_null() || node.child_count == 0 {
        iced::widget::space().into()
      } else {
        let child = unsafe { render_node(&*node.children, id) };
        iced::widget::scrollable(child)
          .width(size(node.width))
          .height(size(node.height))
          .into()
      }
    }
    _ => iced::widget::space().into(),
  };

  let padding = iced::Padding {
    top: node.padding[0],
    right: node.padding[1],
    bottom: node.padding[2],
    left: node.padding[3],
  };

  let mut wrapped = container(body).padding(padding);

  if node.kind == sl_node_kind::CONTAINER {
    wrapped = wrapped
      .align_x(alignment(node.align_x))
      .align_y(alignment(node.align_y));
    wrapped = wrapped.width(size(node.width)).height(size(node.height));
  }

  if node.style.has_background || node.style.has_border || node.style.has_text_color {
    let background: Option<iced::Background> = node
      .style
      .has_background
      .then(|| to_color(node.style.background).into());
    let text_color = node
      .style
      .has_text_color
      .then(|| to_color(node.style.text_color));
    let border = node.style.border;
    let has_border = node.style.has_border;

    wrapped = wrapped.style(move |_theme| iced::widget::container::Style {
      background,
      border: if has_border {
        iced::Border {
          color: to_color(border.color),
          width: border.width,
          radius: border.radius.into(),
        }
      } else {
        iced::Border::default()
      },
      text_color,
      ..Default::default()
    });
  }

  let node_action = unsafe { node.action.as_str() }.unwrap_or_default();
  let effect = id.filter(|_| node.has_effect).map(|_| node.effect);
  if effect.is_none() && node_action.is_empty() {
    return wrapped.into();
  }

  let command = node_action.to_owned();
  clickable(wrapped.into(), move |_, _, _| {
    let action = (!command.is_empty()).then(|| ListenerAction::Payload {
      name: Ustr::from(&command),
      payload: None,
    });
    if let (Some(effect), Some(id)) = (effect, id) {
      return Some(match action {
        Some(action) => ItemMessage::EffectAction(id, ItemEffect::Custom(effect), action),
        None => ItemMessage::Effect(id, ItemEffect::Custom(effect)),
      });
    }
    action.map(ItemMessage::Action)
  })
}

fn progress_element(node: &SlNode) -> iced::Element<'static, ItemMessage> {
  let value = node.value.clamp(0.0, 1.0);
  let color = if node.style.has_text_color {
    to_color(node.style.text_color)
  } else {
    Color::WHITE
  };

  let filled = (value * 100.0).round().max(1.0) as u16;
  let empty = ((1.0 - value) * 100.0).round().max(1.0) as u16;

  iced::widget::row![
    container(Space::new())
      .width(Length::FillPortion(filled))
      .height(Length::Fixed(4.0))
      .style(move |_theme| iced::widget::container::Style {
        background: Some(color.into()),
        ..Default::default()
      }),
    container(Space::new())
      .width(Length::FillPortion(empty))
      .height(Length::Fixed(4.0)),
  ]
  .width(Length::Fixed(48.0))
  .into()
}

unsafe fn canvas_element(node: &SlNode) -> iced::Element<'static, ItemMessage> {
  let canvas = node.canvas;
  if canvas.data.is_null() || canvas.width == 0 || canvas.height == 0 {
    return iced::widget::space().into();
  }

  if canvas.cache_key != 0 {
    if let Some(handle) = image_cache_get(canvas.cache_key) {
      return image_widget(handle)
        .width(size(node.width))
        .height(size(node.height))
        .content_fit(iced::ContentFit::Fill)
        .into();
    }
  }

  let Some(pixels) = (unsafe { read_canvas_pixels(&canvas) }) else {
    return iced::widget::space().into();
  };
  let handle = image_widget::Handle::from_rgba(canvas.width, canvas.height, pixels);
  if canvas.cache_key != 0 {
    image_cache_put(canvas.cache_key, handle.clone());
  }

  image_widget(handle)
    .width(size(node.width))
    .height(size(node.height))
    .content_fit(iced::ContentFit::Fill)
    .into()
}

unsafe fn read_canvas_pixels(canvas: &SlCanvas) -> Option<Vec<u8>> {
  let row = canvas.width as usize * 4;
  let stride = if canvas.stride == 0 {
    row
  } else {
    canvas.stride as usize
  };
  if stride < row {
    eprintln!("[plugin] canvas stride {stride} is smaller than width * 4 ({row})");
    return None;
  }

  if stride == row {
    let len = row * canvas.height as usize;
    Some(unsafe { std::slice::from_raw_parts(canvas.data, len) }.to_vec())
  } else {
    let mut pixels = Vec::with_capacity(row * canvas.height as usize);
    for y in 0..canvas.height as usize {
      let source = unsafe { canvas.data.add(y * stride) };
      pixels.extend_from_slice(unsafe { std::slice::from_raw_parts(source, row) });
    }
    Some(pixels)
  }
}

unsafe fn image_element(node: &SlNode) -> iced::Element<'static, ItemMessage> {
  let image = node.image;
  let handle = match image.source {
    sl_image_source::RAW_RGBA => {
      if image.data.len == 0 || image.width == 0 || image.height == 0 {
        return iced::widget::space().into();
      }
      let bytes = unsafe { image.data.as_bytes() }.unwrap_or_default();
      let expected = image.width as usize * image.height as usize * 4;
      if bytes.len() < expected {
        eprintln!(
          "[plugin] raw RGBA image is {} bytes, expected {expected}",
          bytes.len()
        );
        return iced::widget::space().into();
      }
      if image.cache_key != 0 {
        if let Some(handle) = image_cache_get(image.cache_key) {
          return image_widget(handle)
            .width(size(node.width))
            .height(size(node.height))
            .into();
        }
      }
      let handle =
        image_widget::Handle::from_rgba(image.width, image.height, bytes[..expected].to_vec());
      if image.cache_key != 0 {
        image_cache_put(image.cache_key, handle.clone());
      }
      handle
    }
    sl_image_source::ENCODED => {
      let bytes = unsafe { image.data.as_bytes() }.unwrap_or_default();
      if bytes.is_empty() {
        return iced::widget::space().into();
      }
      let key = if image.cache_key == 0 {
        image_hash_key(bytes)
      } else {
        image.cache_key
      };
      if let Some(handle) = image_cache_get(key) {
        return image_widget(handle)
          .width(size(node.width))
          .height(size(node.height))
          .into();
      }
      match ::image::load_from_memory(bytes) {
        Ok(decoded) => {
          let rgba = decoded.to_rgba8();
          let (width, height) = rgba.dimensions();
          let handle = image_widget::Handle::from_rgba(width, height, rgba.into_raw());
          image_cache_put(key, handle.clone());
          handle
        }
        Err(err) => {
          eprintln!("[plugin] failed to decode image: {err}");
          return iced::widget::space().into();
        }
      }
    }
    sl_image_source::PATH => {
      let Some(path) = (unsafe { image.data.as_str() }) else {
        return iced::widget::space().into();
      };
      image_widget::Handle::from_path(path)
    }
    _ => return iced::widget::space().into(),
  };

  image_widget(handle)
    .width(size(node.width))
    .height(size(node.height))
    .into()
}

fn filters_from_mask(mask: u32) -> Vec<EventFilter> {
  let mut filters = Vec::new();
  if mask & sl_event_mask::CONFIG_RELOAD != 0 {
    filters.push(EventFilter::Named("config.reload".into()));
  }
  if mask & sl_event_mask::COMPOSITOR_UPDATE != 0 {
    filters.push(EventFilter::UpdateCompositor);
  }
  if mask & sl_event_mask::FRAME != 0 {
    filters.push(EventFilter::Frame);
  }
  filters
}

fn effect_from_sl(effect: SlEffect) -> ItemEffect {
  match effect.code {
    sl_effect::REDRAW => ItemEffect::Redraw,
    sl_effect::HIDE => ItemEffect::Hide,
    sl_effect::SHOW => ItemEffect::Show,
    sl_effect::REALLY_HIDE => ItemEffect::ReallyHide,
    sl_effect::DESTROY => ItemEffect::Destroy,
    sl_effect::REALLY_DESTROY => ItemEffect::ReallyDestroy,
    sl_effect::CUSTOM => ItemEffect::Custom(effect.custom),
    _ => ItemEffect::None,
  }
}

fn item_message_to_sl(message: &ItemMessage) -> SlItemMessage {
  let mut out = SlItemMessage::default();
  match message {
    ItemMessage::Effect(_, effect) => {
      out.kind = sl_message_kind::EFFECT;
      out.effect = item_effect_to_code(effect);
      if let ItemEffect::Custom(custom) = effect {
        out.custom = *custom;
      }
    }
    ItemMessage::Action(action) => {
      out.kind = sl_message_kind::ACTION;
      if let Some(name) = action_payload_name(action) {
        out.name = name;
      }
    }
    ItemMessage::EffectAction(_, effect, action) => {
      out.kind = sl_message_kind::EFFECT_ACTION;
      out.effect = item_effect_to_code(effect);
      if let Some(name) = action_payload_name(action) {
        out.name = name;
      }
    }
    _ => out.kind = sl_message_kind::NOOP,
  }
  out
}

fn item_effect_to_code(effect: &ItemEffect) -> u32 {
  match effect {
    ItemEffect::None => sl_effect::NONE,
    ItemEffect::Redraw => sl_effect::REDRAW,
    ItemEffect::Subscribe(_) => sl_effect::SUBSCRIBE,
    ItemEffect::Hide => sl_effect::HIDE,
    ItemEffect::Show => sl_effect::SHOW,
    ItemEffect::ReallyHide => sl_effect::REALLY_HIDE,
    ItemEffect::Destroy => sl_effect::DESTROY,
    ItemEffect::ReallyDestroy => sl_effect::REALLY_DESTROY,
    ItemEffect::Custom(_) => sl_effect::CUSTOM,
    ItemEffect::UpdateWindow(_) => sl_effect::NONE,
  }
}

fn action_payload_name(action: &ListenerAction) -> Option<SlStr> {
  match action {
    ListenerAction::Payload { name, .. } | ListenerAction::Named(name) => {
      Some(SlStr::from_str(name))
    }
    _ => None,
  }
}

fn sl_event_from(event: &ListenerAction) -> SlEvent {
  match event {
    ListenerAction::UpdateCompositor => SlEvent {
      kind: sl_event_kind::COMPOSITOR_UPDATE,
      ..SlEvent::default()
    },
    ListenerAction::Frame => SlEvent {
      kind: sl_event_kind::FRAME,
      ..SlEvent::default()
    },
    ListenerAction::Named(name)
    | ListenerAction::Signal { name, .. }
    | ListenerAction::Timer { name, .. }
      if &**name == "config.reload" =>
    {
      SlEvent {
        kind: sl_event_kind::CONFIG_RELOAD,
        name: SlStr::from_str("config.reload"),
        ..SlEvent::default()
      }
    }
    _ => SlEvent::default(),
  }
}

struct PluginCompositor {
  ctx: *mut PluginInstance,
  vtable: *const SlCompositorVtable,
  state: *mut c_void,
}

unsafe impl Send for PluginCompositor {}
unsafe impl Sync for PluginCompositor {}

impl Drop for PluginCompositor {
  fn drop(&mut self) {
    if let Some(destroy) = unsafe { (*self.vtable).destroy } {
      unsafe { destroy(self.ctx as *mut c_void, self.state) };
    }
  }
}

impl Compositor for PluginCompositor {
  fn state(&self) -> &CompositorState {
    unsafe { &(*self.ctx).compositor_state }
  }

  fn send_cmd(&mut self, cmd: CompositorCommand) -> miette::Result<Void> {
    let (command, arg) = match cmd {
      CompositorCommand::FocusWorkspace(index) => (
        slowshell_plugin::sl_compositor_command::FOCUS_WORKSPACE,
        index,
      ),
      CompositorCommand::FocusWindow(id) => (
        slowshell_plugin::sl_compositor_command::FOCUS_WINDOW,
        id as i32,
      ),
    };

    if let Some(send) = unsafe { (*self.vtable).send_command } {
      unsafe { send(self.ctx as *mut c_void, self.state, command, arg) };
    }

    Ok(Void)
  }

  fn initialize(
    &mut self,
    config: &Config,
    listeners: &mut slowshell_core::listeners::Listeners,
  ) -> miette::Result<Void> {
    let _config_scope = ScopedConfig::current(config);
    let _listeners_scope = ScopedListeners::current(listeners);
    if let Some(initialize) = unsafe { (*self.vtable).initialize } {
      unsafe { initialize(self.ctx as *mut c_void, self.state) };
    }
    Ok(Void)
  }

  fn is_active(&self, _config: &Config) -> bool {
    let Some(is_active) = (unsafe { (*self.vtable).is_active }) else {
      return false;
    };
    unsafe { is_active(self.ctx as *mut c_void, self.state) }
  }

  fn update_state(
    &mut self,
    _store: Option<&Store>,
    _tx: futures_channel::mpsc::UnboundedSender<slowshell_core::message::Message>,
  ) -> miette::Result<Void> {
    if let Some(update) = unsafe { (*self.vtable).update_state } {
      unsafe { update(self.ctx as *mut c_void, self.state) };
    }
    Ok(Void)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn collects_timer_fd_and_redraw_requests() {
    let (_, calls) = with_plugin_calls(|| unsafe {
      host_set_interval(std::ptr::null_mut(), 250, true, 9);
      host_set_interval(std::ptr::null_mut(), 500, false, 4);
      host_register_fd(std::ptr::null_mut(), 7, 3);
      host_request_redraw(std::ptr::null_mut(), 0);
    });

    assert_eq!(calls.timers, vec![(250, true, 9), (500, false, 4)]);
    assert_eq!(calls.fds, vec![(7, 3)]);
    assert!(calls.redraw);
  }

  static FAKE_PAYLOAD_CALLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

  static PAYLOAD_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

  unsafe extern "C" fn fake_payload_create(_ctx: *mut c_void) -> *mut c_void {
    std::ptr::null_mut()
  }

  unsafe extern "C" fn fake_payload_invoke(
    _ctx: *mut c_void,
    _state: *mut c_void,
    _args: *mut c_void,
  ) {
    FAKE_PAYLOAD_CALLED.store(true, Ordering::Relaxed);
  }

  static FAKE_PAYLOAD: SlPayloadVtable = SlPayloadVtable {
    size: std::mem::size_of::<SlPayloadVtable>() as u32,
    create: Some(fake_payload_create),
    destroy: None,
    invoke: Some(fake_payload_invoke),
  };

  static FAKE_RENDERABLE: SlRenderableVtable = SlRenderableVtable {
    size: std::mem::size_of::<SlRenderableVtable>() as u32,
    create: Some(fake_renderable_create),
    destroy: None,
    view: None,
    settings: None,
    initialize: None,
    update: None,
    handle_message: None,
  };

  unsafe extern "C" fn fake_renderable_create(_ctx: *mut c_void) -> *mut c_void {
    std::ptr::null_mut()
  }

  #[test]
  fn collects_registered_renderables() {
    renderables().lock().unwrap().clear();

    PENDING.with(|pending| {
      pending.borrow_mut().push(Pending::Renderable {
        name: "test/menu".into(),
        vtable: &FAKE_RENDERABLE,
      });
    });

    let mut registry = GlobalRegistry::default();
    drain_pending(&mut registry, "test-plugin");

    assert!(
      renderables()
        .lock()
        .unwrap()
        .iter()
        .any(|entry| &*entry.name == "test/menu")
    );

    renderables().lock().unwrap().clear();
  }

  static FAKE_DESKTOP: SlDesktopItemVtable = SlDesktopItemVtable {
    size: std::mem::size_of::<SlDesktopItemVtable>() as u32,
    create: Some(fake_desktop_create),
    destroy: None,
    settings: None,
    initialize: None,
    events: None,
    update: None,
    view: None,
    handle_message: None,
  };

  unsafe extern "C" fn fake_desktop_create(_ctx: *mut c_void) -> *mut c_void {
    std::ptr::null_mut()
  }

  #[test]
  fn dispatcher_invokes_bare_payload() {
    let _guard = PAYLOAD_TEST_LOCK.lock().unwrap();
    FAKE_PAYLOAD_CALLED.store(false, Ordering::Relaxed);
    payloads().lock().unwrap().clear();

    PENDING.with(|pending| {
      pending.borrow_mut().push(Pending::Payload {
        command: "test.click".into(),
        vtable: &FAKE_PAYLOAD,
      });
    });

    let mut registry = GlobalRegistry::default();
    drain_pending(&mut registry, "test-plugin");

    let mut dispatcher = PluginDispatcher;
    let mut store = Store::new();
    let event = ListenerAction::Payload {
      name: "test.click".into(),
      payload: None,
    };
    dispatcher
      .update(&Config::default(), &mut store, &event)
      .unwrap();

    assert!(FAKE_PAYLOAD_CALLED.load(Ordering::Relaxed));

    payloads().lock().unwrap().clear();
    let _ = registry.inside("payload");
  }

  #[test]
  fn plugin_selection_from_config() {
    let path = std::env::temp_dir().join("slowshell-plugin-selection-test.kdl");
    std::fs::write(
      &path,
      "plugins {\n  enabled \"a\" \"c\"\n  disabled \"b\"\n}\n",
    )
    .unwrap();

    let selection = PluginSelection::from_path(Some(&path));
    assert!(selection.allows("a"));
    assert!(!selection.allows("b"));
    assert!(selection.allows("c"));
    assert!(!selection.allows("d"));
    assert_eq!(PluginSelection::from_path(Some(&path)).enabled.len(), 2);

    let _ = std::fs::remove_file(&path);
  }

  #[test]
  fn canvas_pool_reuses_buffers() {
    reset_canvas_pool();
    let first = unsafe { host_canvas_alloc(std::ptr::null_mut(), 4, 4) };
    assert!(!first.is_null());

    let second = unsafe { host_canvas_alloc(std::ptr::null_mut(), 2, 2) };
    assert!(!second.is_null());
    assert_ne!(first, second);

    reset_canvas_pool();
    let reused = unsafe { host_canvas_alloc(std::ptr::null_mut(), 4, 4) };
    assert_eq!(first, reused);

    assert!(unsafe { host_canvas_alloc(std::ptr::null_mut(), 0, 4) }.is_null());
  }

  #[test]
  fn plugin_renderables_are_inserted() {
    renderables().lock().unwrap().clear();

    PENDING.with(|pending| {
      pending.borrow_mut().push(Pending::Renderable {
        name: "test/menu".into(),
        vtable: &FAKE_RENDERABLE,
      });
    });

    let mut registry = GlobalRegistry::default();
    drain_pending(&mut registry, "test-plugin");

    let mut store = Store::new();
    store.insert(Renderables::default());
    register_plugin_renderables(&Config::default(), &mut store);

    assert!(
      store
        .borrow::<Renderables>()
        .is_some_and(|table| table.contains_key(&Ustr::from("test/menu")))
    );

    renderables().lock().unwrap().clear();
  }

  #[test]
  fn collects_registered_desktop_items() {
    desktop_items().lock().unwrap().clear();

    PENDING.with(|pending| {
      pending.borrow_mut().push(Pending::DesktopItem {
        name: "test/widget".into(),
        vtable: &FAKE_DESKTOP,
      });
    });

    let mut registry = GlobalRegistry::default();
    drain_pending(&mut registry, "test-plugin");

    assert!(
      desktop_items()
        .lock()
        .unwrap()
        .iter()
        .any(|entry| &*entry.name == "test/widget")
    );

    desktop_items().lock().unwrap().clear();
  }

  #[test]
  fn dispatches_registered_payload() {
    let _guard = PAYLOAD_TEST_LOCK.lock().unwrap();
    FAKE_PAYLOAD_CALLED.store(false, Ordering::Relaxed);
    payloads().lock().unwrap().clear();

    PENDING.with(|pending| {
      pending.borrow_mut().push(Pending::Payload {
        command: "test.cmd".into(),
        vtable: &FAKE_PAYLOAD,
      });
    });

    let mut registry = GlobalRegistry::default();
    drain_pending(&mut registry, "test-plugin");

    assert!(
      payloads()
        .lock()
        .unwrap()
        .iter()
        .any(|entry| &*entry.command == "test.cmd")
    );

    let args = PluginPayloadArgs {
      command: "".into(),
      args: HashMap::new(),
    };
    dispatch_payload(&Ustr::from("test.cmd"), &args);
    assert!(FAKE_PAYLOAD_CALLED.load(Ordering::Relaxed));

    payloads().lock().unwrap().clear();
    let _ = registry.inside("payload");
  }

  #[test]
  fn builds_payload_args() {
    let raw = [Ustr::from("name=home"), Ustr::from("password=secret")];
    let payload = plugin_payload_build("example.connect", PayloadBuilderArgs(&raw)).unwrap();
    let args = payload.as_this::<PluginPayloadArgs>().unwrap();

    assert_eq!(args.args.get("name").map(|value| &**value), Some("home"));
    assert_eq!(
      args.args.get("password").map(|value| &**value),
      Some("secret")
    );
  }

  #[test]
  fn parses_plugin_config_blocks() {
    plugin_ids().lock().unwrap().push("test-plugin".into());

    let doc: kdl::KdlDocument = "test-plugin {\n  label \"hi\"\n  count 5\n}\n"
      .parse()
      .unwrap();
    let parsed = parse_plugin_configs(doc.nodes()).unwrap().unwrap();
    let configs = parsed.downcast::<PluginConfigs>().unwrap();
    let block = &configs.blocks["test-plugin"];

    assert_eq!(block["label"], "hi");
    assert_eq!(block["count"], "5");

    plugin_ids().lock().unwrap().clear();
    plugin_configs().lock().unwrap().clear();
  }

  #[test]
  fn renders_empty_list() {
    let list = SlNodeList::EMPTY;
    let _ = unsafe { render_nodes(&list, None, 0) };
  }

  #[test]
  fn renders_container_with_image() {
    let pixels = [
      0u8, 0, 0, 255, 255, 255, 255, 255, 0, 0, 0, 255, 255, 255, 255, 255,
    ];
    let image_node = SlNode {
      kind: sl_node_kind::IMAGE,
      image: slowshell_plugin::SlImage {
        source: sl_image_source::RAW_RGBA,
        data: SlStr {
          ptr: pixels.as_ptr(),
          len: pixels.len(),
        },
        width: 2,
        height: 2,
        cache_key: 0,
      },
      ..SlNode::default()
    };
    let container = SlNode {
      kind: sl_node_kind::CONTAINER,
      align_x: sl_align::CENTER,
      padding: [4.0; 4],
      children: &image_node,
      child_count: 1,
      ..SlNode::default()
    };
    let list = SlNodeList {
      nodes: &container,
      len: 1,
    };

    let _ = unsafe { render_nodes(&list, None, 0) };
  }

  #[test]
  fn renders_animated_nodes() {
    let text = SlNode {
      kind: sl_node_kind::TEXT,
      text: SlStr::from_str("hello"),
      size: 12.0,
      style: slowshell_plugin::SlStyle {
        text_color: SlColor {
          r: 1.0,
          g: 1.0,
          b: 1.0,
          a: 1.0,
        },
        has_text_color: true,
        ..slowshell_plugin::SlStyle::default()
      },
      animation: SlAnimation {
        kind: sl_animation_kind::SLIDE,
        easing: sl_animation_easing::EASE_OUT,
        duration_ms: 150,
        offset_x: 0.0,
        offset_y: -8.0,
        tween: true,
      },
      ..SlNode::default()
    };

    let progress = SlNode {
      kind: sl_node_kind::PROGRESS,
      value: 0.5,
      animation: SlAnimation {
        tween: true,
        ..SlAnimation::default()
      },
      ..SlNode::default()
    };

    let nodes = [text, progress];
    let list = SlNodeList {
      nodes: nodes.as_ptr(),
      len: nodes.len(),
    };

    let _ = unsafe { render_nodes(&list, None, 0) };
  }

  #[test]
  fn renders_canvas_buffer() {
    let mut pixels = [0u8; 16];
    pixels[3] = 255;
    let canvas_node = SlNode {
      kind: sl_node_kind::CANVAS,
      canvas: slowshell_plugin::SlCanvas {
        width: 2,
        height: 2,
        stride: 0,
        data: pixels.as_mut_ptr(),
        cache_key: 0,
      },
      ..SlNode::default()
    };
    let list = SlNodeList {
      nodes: &canvas_node,
      len: 1,
    };

    let _ = unsafe { render_nodes(&list, None, 0) };
  }

  #[test]
  fn canvas_stride_is_repacked_to_tight_rgba() {
    let mut pixels = [0u8; 24];
    pixels[0..4].copy_from_slice(&[255, 0, 0, 255]);
    pixels[4..8].copy_from_slice(&[0, 255, 0, 255]);
    pixels[12..16].copy_from_slice(&[0, 0, 255, 255]);
    pixels[16..20].copy_from_slice(&[255, 255, 255, 255]);
    let canvas = slowshell_plugin::SlCanvas {
      width: 2,
      height: 2,
      stride: 12,
      data: pixels.as_mut_ptr(),
      cache_key: 0,
    };

    let packed = unsafe { read_canvas_pixels(&canvas) }.expect("packed");
    assert_eq!(packed.len(), 16);
    assert_eq!(&packed[0..4], &[255, 0, 0, 255]);
    assert_eq!(&packed[4..8], &[0, 255, 0, 255]);
    assert_eq!(&packed[8..12], &[0, 0, 255, 255]);
    assert_eq!(&packed[12..16], &[255, 255, 255, 255]);
  }

  #[test]
  fn image_cache_is_scoped_and_trimmed() {
    let handle = || image_widget::Handle::from_rgba(1, 1, vec![0, 0, 0, 255]);

    image_cache_begin(1);
    image_cache_put(5, handle());
    assert!(image_cache_get(5).is_some());
    image_cache_end();
    assert!(image_cache_get(5).is_some());

    image_cache_begin(2);
    assert!(image_cache_get(5).is_none());
    image_cache_end();

    image_cache_begin(1);
    assert!(image_cache_get(6).is_none());
    image_cache_end();
    image_cache_begin(1);
    assert!(image_cache_get(5).is_none());
    image_cache_end();
  }

  #[test]
  fn renders_row_of_icon_and_text() {
    let icon = SlNode {
      kind: sl_node_kind::ICON,
      text: SlStr::from_str("applications-system-symbolic"),
      size: 16.0,
      ..SlNode::default()
    };
    let label = SlNode {
      kind: sl_node_kind::TEXT,
      text: SlStr::from_str("hello"),
      size: 12.0,
      ..SlNode::default()
    };
    let children = [icon, label];
    let row = SlNode {
      kind: sl_node_kind::ROW,
      spacing: 4.0,
      children: children.as_ptr(),
      child_count: 2,
      ..SlNode::default()
    };
    let list = SlNodeList {
      nodes: &row,
      len: 1,
    };

    let _ = unsafe { render_nodes(&list, None, 0) };
  }

  #[test]
  fn style_get_serializes_config_style() {
    let dir = std::env::temp_dir().join(format!("slowshell-style-get-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.kdl");
    std::fs::write(
      &path,
      r##"
style "example/card" {
  background "#10101e"
  accent "primary"
  radius 8
  shadow #true
}
"##,
    )
    .unwrap();

    let config = Config::from_path_or_default(Some(&path), vec![]);
    let _scope = ScopedConfig::current(&config);

    let mut sheet = SlStyleSheet::default();
    let rc = unsafe {
      host_style_get(
        std::ptr::null_mut(),
        SlStr::from_str("example/card"),
        &mut sheet,
      )
    };
    assert_eq!(rc, 0);
    assert_eq!(sheet.count, 4);

    let entries = unsafe { core::slice::from_raw_parts(sheet.entries, sheet.count as usize) };
    let background = entries
      .iter()
      .find(|entry| unsafe { entry.key.as_str() } == Some("background"))
      .expect("background entry");
    assert_eq!(background.kind, sl_style_value_kind::COLOR);
    assert_eq!(background.color_kind, sl_style_color_kind::HEX);
    assert_eq!(unsafe { background.text.as_str() }, Some("#10101e"));

    let accent = entries
      .iter()
      .find(|entry| unsafe { entry.key.as_str() } == Some("accent"))
      .expect("accent entry");

    assert_eq!(accent.kind, sl_style_value_kind::STRING);
    assert_eq!(unsafe { accent.text.as_str() }, Some("primary"));

    let radius = entries
      .iter()
      .find(|entry| unsafe { entry.key.as_str() } == Some("radius"))
      .expect("radius entry");
    assert_eq!(radius.kind, sl_style_value_kind::INTEGER);
    assert_eq!(radius.integer, 8);

    let shadow = entries
      .iter()
      .find(|entry| unsafe { entry.key.as_str() } == Some("shadow"))
      .expect("shadow entry");
    assert_eq!(shadow.kind, sl_style_value_kind::BOOLEAN);
    assert!(shadow.boolean);

    let rc =
      unsafe { host_style_get(std::ptr::null_mut(), SlStr::from_str("missing"), &mut sheet) };
    assert_ne!(rc, 0);

    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn theme_get_serializes_config_theme() {
    let config = Config::default();
    let _scope = ScopedConfig::current(&config);

    let mut theme = SlTheme::default();
    let rc = unsafe { host_theme_get(std::ptr::null_mut(), &mut theme) };
    assert_eq!(rc, 0);

    assert!((theme.primary.r - 235.0 / 255.0).abs() < 1e-3);
    assert!((theme.primary.g - 160.0 / 255.0).abs() < 1e-3);
    assert!((theme.primary.b - 172.0 / 255.0).abs() < 1e-3);
  }

  #[test]
  fn style_register_adds_to_default_styles() {
    let key = String::from("background");
    let hex = String::from("#ff0000");
    let entry = SlStyleSheetEntry {
      key: SlStr {
        ptr: key.as_ptr(),
        len: key.len(),
      },
      kind: sl_style_value_kind::COLOR,
      color_kind: sl_style_color_kind::HEX,
      text: SlStr {
        ptr: hex.as_ptr(),
        len: hex.len(),
      },
      ..SlStyleSheetEntry::default()
    };
    let entries = [entry];
    let sheet = SlStyleSheet {
      entries: entries.as_ptr(),
      count: 1,
    };

    let name = format!("test.registered.{}", std::process::id());
    let rc = unsafe { host_style_register(std::ptr::null_mut(), SlStr::from_str(&name), &sheet) };
    assert_eq!(rc, 0);

    let dir = std::env::temp_dir().join(format!("slowshell-style-register-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.kdl");
    std::fs::write(&path, "").unwrap();

    let config = Config::from_path_or_default(Some(&path), vec![]);
    let style = config.style(&name);
    assert!(!style.is_empty());
    assert!(matches!(
      style.get("background"),
      Some(StyleValue::Color(ColorValue::Color(c))) if c == "#ff0000"
    ));

    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn renders_container_with_explicit_lengths() {
    let child = SlNode {
      kind: sl_node_kind::TEXT,
      text: SlStr::from_str("inside"),
      size: 12.0,
      ..SlNode::default()
    };
    let container = SlNode {
      kind: sl_node_kind::CONTAINER,
      children: &child,
      child_count: 1,
      align_x: sl_align::START,
      width: SlLength {
        unit: sl_length_unit::FILL,
        value: 0.0,
      },
      height: SlLength {
        unit: sl_length_unit::FIXED,
        value: 32.0,
      },
      ..SlNode::default()
    };
    let list = SlNodeList {
      nodes: &container,
      len: 1,
    };

    let _ = unsafe { render_nodes(&list, None, 0) };
  }

  static CONFIG_PARSER_SEEN: std::sync::Mutex<Vec<(String, Option<String>)>> =
    std::sync::Mutex::new(Vec::new());

  unsafe extern "C" fn capture_config_parser(_ctx: *mut c_void, name: SlStr, block: SlStr) -> i32 {
    let name = unsafe { name.as_str() }.unwrap_or_default().to_owned();
    let block = unsafe { block.as_str() }.map(str::to_owned);
    CONFIG_PARSER_SEEN.lock().unwrap().push((name, block));
    0
  }

  #[test]
  fn config_parser_register_calls_back_with_kdl_text() {
    let name = format!("test.configparser.{}", std::process::id());
    register_plugin_config_parser("test.parser.plugin", name.clone(), capture_config_parser);

    let source = format!("{name} {{\n    label \"hi\"\n}}");
    let document: kdl::KdlDocument = source.parse().expect("kdl");
    let _ = parse_plugin_configs(document.nodes()).unwrap();
    {
      let seen = CONFIG_PARSER_SEEN.lock().unwrap();
      let (_, block) = seen
        .iter()
        .rev()
        .find(|(entry, block)| entry == &name && block.is_some())
        .expect("callback recorded");
      let block = block.as_deref().expect("present entry");
      assert!(block.contains("label"), "block: {block}");
      assert!(block.contains("hi"), "block: {block}");
    }

    CONFIG_PARSER_SEEN.lock().unwrap().clear();
    let empty: kdl::KdlDocument = "".parse().expect("empty kdl");
    let _ = parse_plugin_configs(empty.nodes()).unwrap();
    let seen = CONFIG_PARSER_SEEN.lock().unwrap();
    let (_, block) = seen
      .iter()
      .rev()
      .find(|(entry, _)| entry == &name)
      .expect("callback recorded");
    assert!(block.is_none());
  }

  static CONFIG_RELOAD_SEEN: std::sync::Mutex<Vec<(String, Option<String>)>> =
    std::sync::Mutex::new(Vec::new());

  unsafe extern "C" fn capture_reload_parser(_ctx: *mut c_void, name: SlStr, block: SlStr) -> i32 {
    let name = unsafe { name.as_str() }.unwrap_or_default().to_owned();
    let block = unsafe { block.as_str() }.map(str::to_owned);
    CONFIG_RELOAD_SEEN.lock().unwrap().push((name, block));
    0
  }

  #[test]
  fn plugin_config_parser_reruns_on_config_reload() {
    let name = format!("test.reloadparser.{}", std::process::id());
    register_plugin_config_parser("test.reload.plugin", name.clone(), capture_reload_parser);

    let dir = std::env::temp_dir().join(format!("slowshell-parser-reload-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.kdl");

    let parser = ConfigParser {
      type_id: TypeId::of::<PluginConfigs>(),
      de: parse_plugin_configs,
    };

    std::fs::write(&path, format!("{name} {{\n  label \"first\"\n}}\n")).unwrap();
    let mut config = Config::from_path_or_default(Some(&path), vec![parser]);

    let first = {
      let seen = CONFIG_RELOAD_SEEN.lock().unwrap();
      seen
        .iter()
        .rev()
        .find(|(entry, block)| entry == &name && block.is_some())
        .map(|(_, block)| block.clone())
    };
    assert!(
      matches!(first, Some(Some(ref text)) if text.contains("first")),
      "first: {first:?}"
    );

    std::fs::write(&path, format!("{name} {{\n  label \"second\"\n}}\n")).unwrap();
    config.reload().unwrap();

    let second = {
      let seen = CONFIG_RELOAD_SEEN.lock().unwrap();
      seen
        .iter()
        .rev()
        .find(|(entry, block)| entry == &name && block.is_some())
        .map(|(_, block)| block.clone())
    };
    assert!(
      matches!(second, Some(Some(ref text)) if text.contains("second")),
      "second: {second:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
  }

  #[test]
  fn plugin_error_is_recorded_per_plugin() {
    let instance = PluginInstance::new("test.error.plugin");
    let ctx = &instance as *const PluginInstance as *mut c_void;

    unsafe { host_plugin_error(ctx, 42, SlStr::from_str("boom")) };

    assert_eq!(
      last_plugin_error("test.error.plugin"),
      Some((42, "boom".to_owned()))
    );
  }

  #[test]
  fn compositor_state_get_serializes_active_state() {
    let mut monitors = HashMap::new();
    monitors.insert(
      Ustr::from("DP-1"),
      Monitor {
        name: "DP-1".into(),
        active_workspace: 0,
        width: 1920,
        height: 1080,
        scale: 1.0,
      },
    );
    let state = CompositorState {
      monitors,
      active_window: Some(Window {
        id: 1,
        title: "terminal".into(),
        class: "kitty".into(),
        is_active: true,
        metadata: HashMap::new(),
      }),
      active_windows: Vec::new(),
      workspaces: vec![Workspace {
        id: 7,
        idx: 3,
        name: Some("three".into()),
        output: Some("DP-1".into()),
        is_active: true,
        is_focused: true,
        is_urgent: false,
      }],
      overview_active: true,
    };
    let scope = ScopedCompositor::current(Some(&state));

    let mut out = SlCompositorState::default();
    let rc = unsafe { host_compositor_state_get(std::ptr::null_mut(), &mut out) };
    assert_eq!(rc, 0);
    assert_eq!(out.monitor_count, 1);
    assert_eq!(out.workspace_count, 1);
    assert_eq!(unsafe { (*(out.monitors)).name.as_str() }, Some("DP-1"));

    let workspace = unsafe { &*out.workspaces };
    assert_eq!(unsafe { workspace.name.as_str() }, Some("three"));
    assert_eq!(unsafe { workspace.output.as_str() }, Some("DP-1"));
    assert_eq!(workspace.idx, 3);
    assert!(workspace.is_focused);
    assert!(out.overview_active);

    let window = unsafe { &*out.active_window };
    assert_eq!(unsafe { window.title.as_str() }, Some("terminal"));
    assert_eq!(unsafe { window.wclass.as_str() }, Some("kitty"));

    drop(scope);
    let mut missing = SlCompositorState::default();
    assert_ne!(
      unsafe { host_compositor_state_get(std::ptr::null_mut(), &mut missing) },
      0
    );

    let geometry = unsafe { &*out.monitors };
    assert_eq!(geometry.width, 1920);
    assert_eq!(geometry.height, 1080);
    assert_eq!(geometry.scale, 1.0);
  }

  #[cfg(feature = "panels")]
  #[test]
  fn monitor_bag_exposes_the_monitor_name() {
    let bag = Bag::Monitor("DP-1");
    assert_eq!(bag.get_str("monitor"), Some("DP-1"));
    assert_eq!(bag.get_str("other"), None);
    assert_eq!(bag.get_i64("monitor"), None);
    assert_eq!(bag.get_f64("monitor"), None);
    assert_eq!(bag.get_bool("monitor"), None);
  }

  #[test]
  fn notify_pushes_into_shared_state() {
    use slowshell_commons::notifications::NotificationState;
    use slowshell_plugin::SlNotificationAction;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64};

    let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    let shared: SharedNotificationState = Arc::new(NotificationState {
      dnd: AtomicBool::new(false),
      state: Mutex::new(Default::default()),
      ui: Mutex::new(Default::default()),
      revision: AtomicU64::new(0),
      cmd_tx,
      notify_fd: AtomicI32::new(-1),
    });

    let mut store = Store::new();
    store.insert(shared.clone());
    let _scope = notifications_scope(&store);

    let instance = PluginInstance::new("test.notify.plugin");
    let ctx = &instance as *const PluginInstance as *mut c_void;

    let actions = [SlNotificationAction {
      command: SlStr::from_str("example.reset"),
      label: SlStr::from_str("Reset"),
    }];
    let notification = SlNotification {
      summary: SlStr::from_str("Counter reset"),
      body: SlStr::from_str("body"),
      actions: actions.as_ptr(),
      action_count: 1,
      ..SlNotification::default()
    };

    let id = unsafe { host_notify(ctx, &notification) };
    assert!(id > 0);

    let state = shared.state.lock().unwrap();
    assert_eq!(state.history.len(), 1);
    assert_eq!(state.history[0].summary, "Counter reset");
    assert_eq!(state.history[0].app_name, "test.notify.plugin");
    assert_eq!(state.history[0].actions[0].0, "example.reset");
    assert_eq!(
      state.history[0].plugin.as_deref(),
      Some("test.notify.plugin")
    );
    assert_eq!(state.popups.len(), 1);
  }

  #[test]
  fn notify_schedules_timeout() {
    use slowshell_commons::notifications::{NotificationCmd, NotificationState};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64};

    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    let shared: SharedNotificationState = Arc::new(NotificationState {
      dnd: AtomicBool::new(false),
      state: Mutex::new(Default::default()),
      ui: Mutex::new(Default::default()),
      revision: AtomicU64::new(0),
      cmd_tx,
      notify_fd: AtomicI32::new(-1),
    });

    let mut store = Store::new();
    store.insert(shared.clone());
    let _scope = notifications_scope(&store);

    let instance = PluginInstance::new("test.notify.timeout");
    let ctx = &instance as *const PluginInstance as *mut c_void;

    let timed = SlNotification {
      summary: SlStr::from_str("hi"),
      timeout_ms: 250,
      ..SlNotification::default()
    };
    assert!(unsafe { host_notify(ctx, &timed) } > 0);
    assert!(matches!(
      cmd_rx.try_recv(),
      Ok(NotificationCmd::ExpireAfter(_, 250))
    ));

    let sticky = SlNotification {
      summary: SlStr::from_str("hi2"),
      timeout_ms: 0,
      ..SlNotification::default()
    };
    assert!(unsafe { host_notify(ctx, &sticky) } > 0);
    assert!(cmd_rx.try_recv().is_err());
  }

  #[cfg(feature = "panels")]
  #[test]
  fn component_check_view_is_honored() {
    unsafe extern "C" fn deny(_ctx: *mut c_void, _state: *mut c_void, _opts: *mut c_void) -> bool {
      false
    }
    unsafe extern "C" fn allow(_ctx: *mut c_void, _state: *mut c_void, _opts: *mut c_void) -> bool {
      true
    }
    unsafe extern "C" fn noop_create(_ctx: *mut c_void) -> *mut c_void {
      core::ptr::null_mut()
    }

    let base = SlComponentVtable {
      size: std::mem::size_of::<SlComponentVtable>() as u32,
      create: Some(noop_create),
      destroy: None,
      events: None,
      watch: None,
      update: None,
      view: None,
      check_view: Some(deny),
      stop: None,
      hoverable: false,
    };
    let component = PluginComponent::new(core::ptr::null_mut(), &base, core::ptr::null_mut());
    let store = Store::new();
    #[cfg(feature = "panels")]
    assert!(!component.check_view(&store, None));

    let plain = SlComponentVtable {
      check_view: Some(allow),
      ..base
    };
    let component = PluginComponent::new(core::ptr::null_mut(), &plain, core::ptr::null_mut());
    assert!(component.check_view(&store, None));

    let absent = SlComponentVtable {
      check_view: None,
      ..base
    };
    let component = PluginComponent::new(core::ptr::null_mut(), &absent, core::ptr::null_mut());
    assert!(component.check_view(&store, None));
  }

  unsafe extern "C" fn reject_config_parser(_ctx: *mut c_void, _name: SlStr, _block: SlStr) -> i32 {
    1
  }

  #[test]
  fn config_parser_rejection_is_reported() {
    let plugin_id = format!("test.reject.{}", std::process::id());
    let name = format!("test.rejectentry.{}", std::process::id());
    register_plugin_config_parser(&plugin_id, name.clone(), reject_config_parser);

    let source = format!("{name} {{ x 1 }}");
    let document: kdl::KdlDocument = source.parse().expect("kdl");
    let _ = parse_plugin_configs(document.nodes()).unwrap();

    let error = last_plugin_error(&plugin_id).expect("error recorded");
    assert_eq!(error.0, 1);
    assert!(error.1.contains(&name), "message: {}", error.1);
  }

  #[test]
  fn effect_codes_map_to_item_effects() {
    let code = |code| {
      effect_from_sl(SlEffect {
        code,
        custom: [0; 4],
      })
    };
    assert_eq!(code(sl_effect::NONE), ItemEffect::None);
    assert_eq!(code(sl_effect::REDRAW), ItemEffect::Redraw);
    assert_eq!(code(sl_effect::HIDE), ItemEffect::Hide);
    assert_eq!(code(sl_effect::SHOW), ItemEffect::Show);
    assert_eq!(code(sl_effect::REALLY_HIDE), ItemEffect::ReallyHide);
    assert_eq!(code(sl_effect::DESTROY), ItemEffect::Destroy);
    assert_eq!(code(sl_effect::REALLY_DESTROY), ItemEffect::ReallyDestroy);
    assert_eq!(
      effect_from_sl(SlEffect {
        code: sl_effect::CUSTOM,
        custom: [40, 1, 2, 3],
      }),
      ItemEffect::Custom([40, 1, 2, 3])
    );
  }

  unsafe extern "C" fn desktop_settings_stub(
    _ctx: *mut c_void,
    _state: *mut c_void,
    out: *mut SlDesktopSettings,
  ) {
    if let Some(out) = unsafe { out.as_mut() } {
      out.visibility = sl_visibility::TRANSIENT;
      out.update_when = sl_update_when::EVERY_FRAME;
    }
  }

  unsafe extern "C" fn desktop_create_stub(_ctx: *mut c_void) -> *mut c_void {
    core::ptr::null_mut()
  }

  #[test]
  fn desktop_settings_control_visibility_and_update_strategy() {
    let vtable = SlDesktopItemVtable {
      size: std::mem::size_of::<SlDesktopItemVtable>() as u32,
      create: Some(desktop_create_stub),
      destroy: None,
      settings: Some(desktop_settings_stub),
      initialize: None,
      events: None,
      update: None,
      view: None,
      handle_message: None,
    };

    let item = PluginDesktopItem::new(
      "test-desktop".into(),
      core::ptr::null_mut(),
      &vtable,
      core::ptr::null_mut(),
    );
    assert_eq!(item.visibility(), Visibility::Transient);
    assert_eq!(item.update_strategy(), UpdateWhen::EveryFrame);
  }

  #[test]
  fn renderable_update_and_handle_message_are_wired() {
    unsafe extern "C" fn create(_ctx: *mut c_void) -> *mut c_void {
      core::ptr::null_mut()
    }
    unsafe extern "C" fn update(_ctx: *mut c_void, _state: *mut c_void) -> SlEffect {
      SlEffect {
        code: sl_effect::CUSTOM,
        custom: [40, 7, 0, 0],
      }
    }
    unsafe extern "C" fn handle(
      _ctx: *mut c_void,
      _state: *mut c_void,
      _message: *const SlItemMessage,
    ) -> SlEffect {
      SlEffect {
        code: sl_effect::HIDE,
        custom: [0; 4],
      }
    }

    let vtable = SlRenderableVtable {
      size: std::mem::size_of::<SlRenderableVtable>() as u32,
      create: Some(create),
      destroy: None,
      view: None,
      settings: None,
      initialize: None,
      update: Some(update),
      handle_message: Some(handle),
    };
    let mut renderable = PluginRenderable {
      ctx: core::ptr::null_mut(),
      vtable: &vtable,
      state: core::ptr::null_mut(),
    };

    let mut store = Store::new();
    assert_eq!(
      renderable.update(&Config::default(), &mut store, &()),
      Some(ItemEffect::Custom([40, 7, 0, 0]))
    );
    assert_eq!(
      renderable.handle_message(&ItemMessage::Noop),
      Some(ItemEffect::Hide)
    );
  }

  #[test]
  fn desktop_handle_message_is_wired() {
    unsafe extern "C" fn create(_ctx: *mut c_void) -> *mut c_void {
      core::ptr::null_mut()
    }
    unsafe extern "C" fn handle(
      _ctx: *mut c_void,
      _state: *mut c_void,
      _message: *const SlItemMessage,
    ) -> SlEffect {
      SlEffect {
        code: sl_effect::CUSTOM,
        custom: [40, 9, 0, 0],
      }
    }

    let vtable = SlDesktopItemVtable {
      size: std::mem::size_of::<SlDesktopItemVtable>() as u32,
      create: Some(create),
      destroy: None,
      settings: None,
      initialize: None,
      events: None,
      update: None,
      view: None,
      handle_message: Some(handle),
    };
    let mut item = PluginDesktopItem::new(
      "test-desktop-msg".into(),
      core::ptr::null_mut(),
      &vtable,
      core::ptr::null_mut(),
    );
    assert_eq!(
      item.handle_message(None, &ItemMessage::Noop),
      ItemEffect::Custom([40, 9, 0, 0])
    );
  }

  #[test]
  fn renders_scrollable_and_effect_clickable() {
    let text = SlNode {
      kind: sl_node_kind::TEXT,
      text: SlStr::from_str("x"),
      size: 12.0,
      ..SlNode::default()
    };
    let clickable = SlNode {
      kind: sl_node_kind::CONTAINER,
      children: &text,
      child_count: 1,
      has_effect: true,
      effect: [40, 3, 0, 0],
      ..SlNode::default()
    };
    let scrollable = SlNode {
      kind: sl_node_kind::SCROLLABLE,
      children: &clickable,
      child_count: 1,
      height: SlLength {
        unit: sl_length_unit::FILL,
        value: 0.0,
      },
      ..SlNode::default()
    };
    let list = SlNodeList {
      nodes: &scrollable,
      len: 1,
    };

    let id = iced_layershell::reexport::IcedId::unique();
    let _ = unsafe { render_nodes(&list, Some(id), 0) };
  }

  #[test]
  fn service_getters_snapshot_live_state() {
    use slowshell_commons::audio::{AudioCmd, AudioInner, AudioSink, AudioState, MprisPlayer};
    use std::sync::{Arc, Mutex};

    let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel::<AudioCmd>();
    let shared: SharedAudioState = Arc::new(AudioState {
      state: Mutex::new(AudioInner {
        volume: 0.5,
        muted: false,
        default_sink_name: Some("speakers".into()),
        sinks: vec![AudioSink {
          id: 1,
          name: "sink0".into(),
          description: "Speakers".into(),
          volume: 0.5,
          muted: false,
          is_default: true,
        }],
      }),
      player: Mutex::new(Some(MprisPlayer {
        bus_name: "org.mpris.MediaPlayer2.mpv".into(),
        identity: "mpv".into(),
        title: "Song".into(),
        artist: "Artist".into(),
        album: "Album".into(),
        art_url: Some("https://example.test/art".into()),
        playback_status: "Playing".into(),
        can_play_pause: true,
        can_go_next: true,
        can_go_previous: false,
      })),
      revision: AtomicU64::new(0),
      cmd_tx,
    });

    let mut store = Store::new();
    store.insert(shared);
    let _scope = store_scope(&store);

    let mut out = SlAudioState::default();
    let code = unsafe { host_audio_state_get(std::ptr::null_mut(), &mut out) };
    assert_eq!(code, 0);
    assert_eq!(unsafe { out.default_sink.as_str() }, Some("speakers"));
    assert_eq!(out.sink_count, 1);
    let sink = unsafe { &*out.sinks };
    assert_eq!(unsafe { sink.description.as_str() }, Some("Speakers"));
    assert!(out.has_player);
    assert_eq!(unsafe { out.player.title.as_str() }, Some("Song"));
  }

  #[test]
  fn power_getter_snapshots_battery_and_brightness() {
    use slowshell_commons::power::{
      PowerCmd, PowerData, PowerProfile, PowerState as PowerServiceState,
    };
    use std::sync::{Arc, Mutex};

    let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel::<PowerCmd>();
    let shared: SharedPowerState = Arc::new(PowerServiceState {
      data: Mutex::new(PowerData {
        percent: Some(63),
        charging: true,
        status: "Charging".into(),
        health: Some(95),
        active_profile: Some(PowerProfile::Balanced),
        available_profiles: vec![PowerProfile::PowerSaver, PowerProfile::Balanced],
        brightness_percent: 70,
        brightness_max: 1000,
        brightness_current: 700,
        device_name: "BAT0".into(),
        ..Default::default()
      }),
      revision: AtomicU64::new(0),
      cmd_tx,
    });

    let mut store = Store::new();
    store.insert(shared);
    let _scope = store_scope(&store);

    let mut out = SlPowerState::default();
    let code = unsafe { host_power_state_get(std::ptr::null_mut(), &mut out) };
    assert_eq!(code, 0);
    assert!(out.has_percent);
    assert_eq!(out.percent, 63);
    assert!(out.charging);
    assert_eq!(unsafe { out.status.as_str() }, Some("Charging"));
    assert_eq!(unsafe { out.device_name.as_str() }, Some("BAT0"));
    assert!(out.has_active_profile);
    assert_eq!(unsafe { out.active_profile.as_str() }, Some("balanced"));
    assert_eq!(out.available_profile_count, 2);
    assert_eq!(
      unsafe { (*out.available_profiles).as_str() },
      Some("power-saver")
    );
    assert_eq!(out.brightness_current, 700);
  }

  #[test]
  fn dispatch_sends_payload_action() {
    let (tx, mut rx) = futures_channel::mpsc::unbounded::<slowshell_core::message::Message>();
    let mut store = Store::new();
    store.insert(ActionDispatcher(tx));
    let _scope = store_scope(&store);

    let code = unsafe { host_dispatch(std::ptr::null_mut(), SlStr::from_str("audio.toggle_mute")) };
    assert_eq!(code, 0);

    match rx.try_recv().expect("message") {
      slowshell_core::message::Message::FdUpdate(ListenerAction::Payload { name, .. }) => {
        assert_eq!(&*name, "audio.toggle_mute");
      }
      _ => panic!("unexpected message"),
    }
  }

  #[test]
  fn dispatch_with_string_builds_payload() {
    let (tx, mut rx) = futures_channel::mpsc::unbounded::<slowshell_core::message::Message>();
    let mut store = Store::new();
    store.insert(ActionDispatcher(tx));

    let mut builders = PayloadBuilderRegistry::new();
    builders.register(PayloadBuilder {
      commands: &["test.echo"],
      build: |_, _| Some(PayloadBox::new(7u32)),
    });
    store.insert(std::sync::Arc::new(builders));
    let _scope = store_scope(&store);

    let code = unsafe {
      host_dispatch_with_string(
        std::ptr::null_mut(),
        SlStr::from_str("test.echo"),
        SlStr::from_str("7"),
      )
    };
    assert_eq!(code, 0);

    match rx.try_recv().expect("message") {
      slowshell_core::message::Message::FdUpdate(ListenerAction::Payload { name, payload }) => {
        assert_eq!(&*name, "test.echo");
        assert_eq!(payload.expect("payload").enforce::<u32>(), &7);
      }
      _ => panic!("unexpected message"),
    }
  }

  #[test]
  fn dispatch_with_string_falls_back_to_named() {
    let (tx, mut rx) = futures_channel::mpsc::unbounded::<slowshell_core::message::Message>();
    let mut store = Store::new();
    store.insert(ActionDispatcher(tx));
    store.insert(std::sync::Arc::new(PayloadBuilderRegistry::new()));
    let _scope = store_scope(&store);

    let code = unsafe {
      host_dispatch_with_string(
        std::ptr::null_mut(),
        SlStr::from_str("test.named"),
        SlStr::from_str(""),
      )
    };
    assert_eq!(code, 0);

    match rx.try_recv().expect("message") {
      slowshell_core::message::Message::FdUpdate(ListenerAction::Named(name)) => {
        assert_eq!(&*name, "test.named");
      }
      _ => panic!("unexpected message"),
    }
  }

  #[test]
  fn registers_plugin_command() {
    let name = "test.plugin.command";
    slowshell_core::commands::commands().unregister(name);

    let code = unsafe {
      host_register_command(
        std::ptr::null_mut(),
        SlStr::from_str(name),
        SlStr::from_str("Plugin Command"),
        SlStr::from_str("Does a thing"),
      )
    };
    assert_eq!(code, 0);

    let mut registry = GlobalRegistry::default();
    drain_pending(&mut registry, "test-plugin");

    let entry = slowshell_core::commands::commands()
      .get(name)
      .expect("registered command");
    assert_eq!(entry.title.as_deref(), Some("Plugin Command"));
    assert_eq!(entry.description.as_deref(), Some("Does a thing"));

    slowshell_core::commands::commands().unregister(name);
  }

  #[test]
  fn frame_events_round_trip() {
    assert_eq!(
      filters_from_mask(sl_event_mask::FRAME),
      vec![EventFilter::Frame]
    );

    let sl = sl_event_from(&ListenerAction::Frame);
    assert_eq!(sl.kind, sl_event_kind::FRAME);

    let filter: EventFilter = (&ListenerAction::Frame).into();
    assert_eq!(filter, EventFilter::Frame);
    assert!(EventFilter::Frame.matches(&ListenerAction::Frame));
  }
}
