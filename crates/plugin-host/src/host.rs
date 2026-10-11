use std::sync::{
  Arc, OnceLock,
  atomic::{AtomicU32, Ordering},
};

use abi_stable::{
  sabi_trait::TD_Opaque,
  std_types::{RBox, ROption, RStr, RString, RVec},
};
use slowshell_commons::notifications::{NotificationCmd, NotificationImage, NotificationItem};
use slowshell_config::style::new_default_style;
use slowshell_core::{
  ErasedEntry, StoreKey,
  listeners::{CompositorEvent, ListenerAction},
  types::PayloadBuilderRegistry,
};
use slowshell_plugin_abi::{
  AudioState, BluetoothState, Color, ComponentFactoryBox, CompositorFactoryBox, CompositorState,
  ConfigParserBox, DesktopItemFactoryBox, ErasedRef, ErasedValue, Host, Host_TO, NetworkState,
  Notification, PayloadBox, PayloadFactoryBox, PayloadParserBox, PowerState,
  QuickSettingsFactoryBox, RenderableFactoryBox, ServiceFactoryBox, SpotlightFactoryBox,
  StoreCallbackBox, SystemState, Theme, TrayState, TypeKey,
};

use crate::{
  services,
  state::{
    Pending, current_plugin, current_store, current_store_mut, plugin_configs, record_fd,
    record_redraw, record_timer, set_current_compositor_state,
  },
};

#[cfg(feature = "panels")]
use crate::state::current_options;

static NEXT_NOTIFICATION_ID: AtomicU32 = AtomicU32::new(1_000_000);

struct HostImpl;

pub(crate) fn host() -> &'static Host_TO<RBox<()>> {
  static HOST: OnceLock<Host_TO<RBox<()>>> = OnceLock::new();
  HOST.get_or_init(|| Host_TO::from_value(HostImpl, TD_Opaque))
}

fn plugin_id() -> String {
  current_plugin().unwrap_or_else(|| "?".to_owned())
}

fn config_value(key: &str) -> Option<String> {
  let plugin_id = current_plugin()?;
  plugin_configs()
    .lock()
    .unwrap()
    .get(&plugin_id)
    .and_then(|block| block.get(key))
    .cloned()
}

fn to_core_key(key: TypeKey) -> StoreKey {
  StoreKey {
    type_name: key.name.as_str(),
    size: key.size,
    align: key.align,
  }
}

impl Host for HostImpl {
  fn log(&self, _level: i32, message: RString) {
    eprintln!("[plugin:{}] {message}", plugin_id());
  }

  fn plugin_error(&self, code: u32, message: RString) {
    crate::state::record_plugin_error(&plugin_id(), code, &message);
  }

  fn option_str(&self, key: RStr<'_>) -> ROption<RString> {
    #[cfg(feature = "panels")]
    {
      if let Some(options) = current_options()
        && let Some(value) = options.str(&*key)
      {
        return ROption::RSome(RString::from(value));
      }
    }
    let _ = key;
    ROption::RNone
  }

  fn option_f64(&self, key: RStr<'_>) -> ROption<f64> {
    #[cfg(feature = "panels")]
    {
      if let Some(options) = current_options()
        && let Some(value) = options.number(&*key)
      {
        return ROption::RSome(value as f64);
      }
    }
    let _ = key;
    ROption::RNone
  }

  fn option_i64(&self, key: RStr<'_>) -> ROption<i64> {
    #[cfg(feature = "panels")]
    {
      if let Some(options) = current_options()
        && let Some(value) = options.int(&*key)
      {
        return ROption::RSome(value as i64);
      }
    }
    let _ = key;
    ROption::RNone
  }

  fn option_bool(&self, key: RStr<'_>) -> ROption<bool> {
    #[cfg(feature = "panels")]
    {
      if let Some(options) = current_options()
        && let Some(value) = options.bool(&*key)
      {
        return ROption::RSome(value);
      }
    }
    let _ = key;
    ROption::RNone
  }

  fn config_str(&self, key: RStr<'_>) -> ROption<RString> {
    match config_value(key.as_str()) {
      Some(value) => ROption::RSome(RString::from(value)),
      None => ROption::RNone,
    }
  }

  fn config_f64(&self, key: RStr<'_>) -> ROption<f64> {
    config_value(&key)
      .and_then(|value| value.parse().ok())
      .map_or(ROption::RNone, ROption::RSome)
  }

  fn config_i64(&self, key: RStr<'_>) -> ROption<i64> {
    config_value(&key)
      .and_then(|value| value.parse().ok())
      .map_or(ROption::RNone, ROption::RSome)
  }

  fn config_bool(&self, key: RStr<'_>) -> ROption<bool> {
    config_value(&key)
      .and_then(|value| value.parse().ok())
      .map_or(ROption::RNone, ROption::RSome)
  }

  fn set_interval(&self, millis: u32, repeating: bool, tag: u32) -> i32 {
    record_timer(millis, repeating, tag);
    0
  }

  fn register_fd(&self, fd: i32, action: u32) -> i32 {
    if fd < 0 {
      return -1;
    }
    record_fd(fd, action);
    0
  }

  fn request_redraw(&self, _window: u64) {
    record_redraw();
  }

  fn compositor_state(&self) -> ROption<CompositorState> {
    services::compositor_state().map_or(ROption::RNone, ROption::RSome)
  }

  fn set_compositor_state(&self, state: CompositorState) {
    set_current_compositor_state(services::convert_compositor_state(&state));
  }

  fn compositor_register_fd(&self, fd: i32, flags: u32) -> i32 {
    if fd < 0 {
      return -1;
    }
    let Some(listeners) = crate::state::current_listeners() else {
      return -1;
    };
    let listeners = unsafe { &mut *listeners };

    let flags = if flags == 0 {
      nix::sys::epoll::EpollFlags::EPOLLIN | nix::sys::epoll::EpollFlags::EPOLLET
    } else {
      let mut mapped = nix::sys::epoll::EpollFlags::empty();
      if flags & 1 != 0 {
        mapped |= nix::sys::epoll::EpollFlags::EPOLLIN;
      }
      if flags & 2 != 0 {
        mapped |= nix::sys::epoll::EpollFlags::EPOLLET;
      }
      mapped
    };

    listeners.flag(fd, flags);
    listeners.action(fd, ListenerAction::UpdateCompositor(CompositorEvent::ALL));
    0
  }

  fn audio_state(&self) -> ROption<AudioState> {
    services::audio_state().map_or(ROption::RNone, ROption::RSome)
  }

  fn bluetooth_state(&self) -> ROption<BluetoothState> {
    services::bluetooth_state().map_or(ROption::RNone, ROption::RSome)
  }

  fn network_state(&self) -> ROption<NetworkState> {
    services::network_state().map_or(ROption::RNone, ROption::RSome)
  }

  fn system_state(&self) -> ROption<SystemState> {
    services::system_state().map_or(ROption::RNone, ROption::RSome)
  }

  fn tray_state(&self) -> ROption<TrayState> {
    services::tray_state().map_or(ROption::RNone, ROption::RSome)
  }

  fn power_state(&self) -> ROption<PowerState> {
    services::power_state().map_or(ROption::RNone, ROption::RSome)
  }

  fn style_number(&self, classes: RStr<'_>, property: RStr<'_>) -> ROption<f32> {
    services::style_number(classes.as_str(), property.as_str())
      .map_or(ROption::RNone, ROption::RSome)
  }

  fn style_bool(&self, classes: RStr<'_>, property: RStr<'_>) -> ROption<bool> {
    services::style_bool(classes.as_str(), property.as_str()).map_or(ROption::RNone, ROption::RSome)
  }

  fn style_color(&self, classes: RStr<'_>, property: RStr<'_>) -> ROption<Color> {
    services::style_color(classes.as_str(), property.as_str())
      .map_or(ROption::RNone, ROption::RSome)
  }

  fn theme(&self) -> Theme {
    services::theme()
  }

  fn register_style(&self, css: RStr<'_>) -> i32 {
    new_default_style(css.as_str());
    let plugin_id = plugin_id();
    crate::state::contribute(&plugin_id, |contributions| {
      contributions.styles.push(css.to_string());
    });
    0
  }

  fn dispatch(&self, command: RStr<'_>) -> bool {
    let Some(store) = current_store() else {
      return crate::service::current_dispatcher()
        .is_some_and(|dispatcher| dispatcher.dispatch(command.as_str()));
    };
    let Some(dispatcher) = store.borrow::<slowshell_core::ActionDispatcher>() else {
      return false;
    };
    dispatcher.dispatch(command.as_str())
  }

  fn dispatch_with_string(&self, name: RStr<'_>, payload: RStr<'_>) -> bool {
    let Some(store) = current_store() else {
      let Some(dispatcher) = crate::service::current_dispatcher() else {
        return false;
      };
      let Some(builders) = crate::service::current_builders() else {
        return false;
      };
      let Some(args) = slowshell_core::types::split_args(payload.as_str()) else {
        return false;
      };
      return dispatcher.dispatch_with_args(name.as_str(), &args, &builders);
    };
    let Some(dispatcher) = store.borrow::<slowshell_core::ActionDispatcher>() else {
      return false;
    };
    let Some(builders) = store.borrow::<Arc<PayloadBuilderRegistry>>() else {
      return false;
    };
    let Some(args) = slowshell_core::types::split_args(payload.as_str()) else {
      return false;
    };
    dispatcher.dispatch_with_args(name.as_str(), &args, builders)
  }

  fn persistence_get(&self, key: RStr<'_>) -> ROption<RString> {
    let plugin_id = plugin_id();
    let value = crate::state::with_plugin_persistence(&plugin_id, |map| map.get(&*key).cloned());
    value.map_or(ROption::RNone, |value| ROption::RSome(RString::from(value)))
  }

  fn persistence_set(&self, key: RStr<'_>, value: RStr<'_>) -> bool {
    let plugin_id = plugin_id();
    let snapshot = crate::state::with_plugin_persistence(&plugin_id, |map| {
      map.insert(key.to_string(), value.to_string());
      map.clone()
    });
    crate::state::flush_plugin_persistence(&plugin_id, &snapshot);
    true
  }

  fn persistence_remove(&self, key: RStr<'_>) -> bool {
    let plugin_id = plugin_id();
    let snapshot = crate::state::with_plugin_persistence(&plugin_id, |map| {
      map.remove(&*key);
      map.clone()
    });
    crate::state::flush_plugin_persistence(&plugin_id, &snapshot);
    true
  }

  fn notify(&self, notification: Notification) -> ROption<u32> {
    let Some(shared) = crate::state::current_notifications() else {
      return ROption::RNone;
    };

    let plugin_id = current_plugin();
    let app_name = notification
      .app_name
      .clone()
      .into_option()
      .map(|name| name.to_string())
      .or_else(|| plugin_id.clone())
      .unwrap_or_else(|| "Plugin".to_owned());
    let app_icon = notification
      .app_icon
      .clone()
      .into_option()
      .map(|icon| icon.to_string());

    let actions = notification
      .actions
      .iter()
      .map(|action| (action.command.to_string(), action.label.to_string()))
      .collect::<Vec<_>>();

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
      summary: notification.summary.to_string(),
      body: notification.body.to_string(),
      actions,
      urgency: notification.urgency,
      created_at: std::time::Instant::now(),
      plugin: plugin_id,
    };

    let is_dnd = shared.dnd.load(Ordering::Relaxed);
    {
      let mut guard = shared.state.lock().unwrap();
      let state = Arc::make_mut(&mut guard);
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

    ROption::RSome(id)
  }

  fn store_insert(&self, key: TypeKey, value: ErasedValue) -> bool {
    let Some(store) = current_store_mut() else {
      return false;
    };

    let entry = ErasedEntry {
      key: to_core_key(key),
      ptr: value.ptr,
      drop_fn: value.drop_fn,
    };
    std::mem::forget(value);

    let inserted = unsafe { (*store).erased_insert(entry) };
    if inserted {
      crate::store::notify(key);
    }
    inserted
  }

  fn store_get<'a>(
    &self,
    _ctx: &slowshell_plugin_abi::Context<'a>,
    key: TypeKey,
  ) -> ROption<ErasedRef<'a>> {
    let Some(store) = current_store() else {
      return ROption::RNone;
    };
    match store.erased_get(to_core_key(key)) {
      Some(ptr) => ROption::RSome(ErasedRef::new(key, ptr)),
      None => ROption::RNone,
    }
  }

  fn store_remove(&self, key: TypeKey) -> ROption<ErasedValue> {
    let Some(store) = current_store_mut() else {
      return ROption::RNone;
    };
    let entry = unsafe { (*store).erased_remove(to_core_key(key)) };
    match entry {
      Some(entry) => {
        let value = ErasedValue {
          key,
          ptr: entry.ptr,
          drop_fn: entry.drop_fn,
        };
        std::mem::forget(entry);
        crate::store::notify(key);
        ROption::RSome(value)
      }
      None => ROption::RNone,
    }
  }

  fn store_contains(&self, key: TypeKey) -> bool {
    current_store().is_some_and(|store| store.erased_contains(to_core_key(key)))
  }

  fn store_subscribe(&self, key: TypeKey, callback: StoreCallbackBox) {
    crate::store::subscribe(key, current_plugin().unwrap_or_default(), callback);
  }

  fn register_component(&self, name: RString, factory: ComponentFactoryBox) {
    crate::state::PENDING.with(|pending| {
      pending.borrow_mut().push(Pending::Component {
        name: name.to_string(),
        factory,
      });
    });
  }

  fn register_renderable(&self, name: RString, factory: RenderableFactoryBox) {
    crate::state::PENDING.with(|pending| {
      pending.borrow_mut().push(Pending::Renderable {
        name: name.to_string(),
        factory,
      });
    });
  }

  fn register_payload(&self, command: RString, factory: PayloadFactoryBox) {
    crate::state::PENDING.with(|pending| {
      pending.borrow_mut().push(Pending::Payload {
        command: command.to_string(),
        factory,
      });
    });
  }

  fn register_desktop_item(&self, name: RString, factory: DesktopItemFactoryBox) {
    crate::state::PENDING.with(|pending| {
      pending.borrow_mut().push(Pending::DesktopItem {
        name: name.to_string(),
        factory,
      });
    });
  }

  fn register_desktop_item_deployer(
    &self,
    name: RString,
    factory: slowshell_plugin_abi::DeployableDesktopItemFactoryBox,
  ) {
    crate::state::PENDING.with(|pending| {
      pending.borrow_mut().push(Pending::DesktopItemDeployer {
        name: name.to_string(),
        factory,
      });
    });
  }

  fn register_compositor(&self, name: RString, factory: CompositorFactoryBox) {
    crate::state::PENDING.with(|pending| {
      pending.borrow_mut().push(Pending::Compositor {
        name: name.to_string(),
        factory,
      });
    });
  }

  fn register_spotlight(&self, name: RString, factory: SpotlightFactoryBox) {
    crate::state::PENDING.with(|pending| {
      pending.borrow_mut().push(Pending::Spotlight {
        name: name.to_string(),
        factory,
      });
    });
  }

  fn register_quick_settings(&self, name: RString, factory: QuickSettingsFactoryBox) {
    crate::state::PENDING.with(|pending| {
      pending.borrow_mut().push(Pending::QuickSettings {
        name: name.to_string(),
        factory,
      });
    });
  }

  fn register_config_parser(&self, name: RString, parser: ConfigParserBox) {
    crate::state::PENDING.with(|pending| {
      pending.borrow_mut().push(Pending::ConfigParser {
        name: name.to_string(),
        parser,
      });
    });
  }

  fn register_service(&self, name: RString, factory: ServiceFactoryBox, autostart: bool) {
    crate::state::PENDING.with(|pending| {
      pending.borrow_mut().push(Pending::Service {
        name: name.to_string(),
        factory,
        autostart,
      });
    });
  }

  fn service_start(&self, name: RStr<'_>) -> bool {
    crate::service::start(name.as_str())
  }

  fn service_stop(&self, name: RStr<'_>) -> bool {
    crate::service::stop(name.as_str())
  }

  fn service_running(&self, name: RStr<'_>) -> bool {
    crate::service::is_running(name.as_str())
  }

  fn service_send(&self, name: RStr<'_>, message: RStr<'_>) -> bool {
    crate::service::send(name.as_str(), message.as_str())
  }

  fn service_notify(&self) -> bool {
    crate::service::notify_current()
  }

  fn service_sleep(&self, millis: u64) -> bool {
    crate::service::sleep(millis)
  }

  fn service_stopping(&self) -> bool {
    crate::service::stopping()
  }

  fn service_recv(&self, timeout_millis: u32) -> ROption<RString> {
    crate::service::recv(timeout_millis).map_or(ROption::RNone, |message| {
      ROption::RSome(RString::from(message))
    })
  }

  fn register_command(&self, name: RString, title: RString, description: RString) {
    crate::state::PENDING.with(|pending| {
      pending.borrow_mut().push(Pending::Command {
        name: name.to_string(),
        title: Some(title.to_string()),
        description: if description.is_empty() {
          None
        } else {
          Some(description.to_string())
        },
      });
    });
  }

  fn register_payload_parser(&self, commands: RVec<RString>, parser: PayloadParserBox) {
    crate::state::PENDING.with(|pending| {
      pending.borrow_mut().push(Pending::PayloadParser {
        commands: commands.iter().map(|command| command.to_string()).collect(),
        parser,
      });
    });
  }

  fn store_touch(&self, key: TypeKey) -> bool {
    crate::store::notify(key)
  }

  fn canvas(&self, width: u32, height: u32) -> ROption<slowshell_plugin_abi::CanvasRef> {
    match crate::state::canvas_alloc(width, height) {
      Some((ptr, len, id)) => ROption::RSome(slowshell_plugin_abi::CanvasRef { ptr, len, id }),
      None => ROption::RNone,
    }
  }

  fn dispatch_payload(&self, name: RStr<'_>, payload: PayloadBox) -> bool {
    let payload = slowshell_core::types::PayloadBox::new(payload);
    let Some(store) = current_store() else {
      return crate::service::current_dispatcher()
        .is_some_and(|dispatcher| dispatcher.dispatch_payload(name.as_str(), Some(payload)));
    };
    let Some(dispatcher) = store.borrow::<slowshell_core::ActionDispatcher>() else {
      return false;
    };
    dispatcher.dispatch_payload(name.as_str(), Some(payload))
  }
}
