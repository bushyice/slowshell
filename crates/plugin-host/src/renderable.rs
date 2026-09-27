use std::{
  any::Any,
  cell::RefCell,
  os::fd::{AsRawFd, FromRawFd, OwnedFd},
  sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
  },
  time::Duration,
};

use iced::Element;
use iced_layershell::reexport::IcedId;
use slowshell_commons::popups::PopupSettings;
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::{FdHandle, ListenerAction},
  message::{ItemEffect, ItemMessage},
  types::Ustr,
};
use slowshell_plugin_abi::{Context, RenderableObjectBox, RenderableSettings};
use slowshell_widgets::{Backdrop, ComponentOptions, Renderable, SizedPopup};

use crate::{
  events::{effect_from_abi, event_from_action, filters_to_core, message_to_abi},
  host::host,
  render::{RenderCtx, render_nodes},
  state::{
    PluginCalls, PluginScope, ScopedConfig, compositor_scope, notifications_scope, store_scope,
    store_scope_mut,
  },
};

#[cfg(feature = "panels")]
use crate::state::ScopedOptions;

#[cfg_attr(not(feature = "panels"), allow(dead_code))]
static NEXT_INSTANCE_ID: AtomicU64 = AtomicU64::new(1);

#[cfg_attr(not(feature = "panels"), allow(dead_code))]
pub(crate) struct PluginRenderable {
  plugin_id: String,
  inner: RefCell<RenderableObjectBox>,
  event_name: Ustr,
  options: Mutex<Option<Arc<ComponentOptions>>>,
  #[allow(dead_code)]
  timers: Vec<(i32, u32)>,
  #[allow(dead_code)]
  fds: Vec<(i32, u32)>,
}

#[cfg_attr(not(feature = "panels"), allow(dead_code))]
impl PluginRenderable {
  pub(crate) fn new(plugin_id: String, inner: RenderableObjectBox) -> Self {
    let id = NEXT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed);
    Self {
      plugin_id,
      inner: RefCell::new(inner),
      event_name: format!("component/plugin.{id}").into(),
      options: Mutex::new(None),
      timers: Vec::new(),
      fds: Vec::new(),
    }
  }

  fn settings(&self) -> RenderableSettings {
    self.inner.borrow().settings()
  }

  fn set_options(&self, options: Option<&ComponentOptions>) {
    if let Ok(mut guard) = self.options.lock() {
      *guard = options.map(|options| Arc::new(options.clone()));
    }
  }

  fn take_options(&self) -> Option<Arc<ComponentOptions>> {
    self.options.lock().ok().and_then(|guard| guard.clone())
  }
}

impl Renderable for PluginRenderable {
  fn events(&self) -> Vec<slowshell_core::message::EventFilter> {
    filters_to_core(&self.inner.borrow().events(), &self.event_name)
  }

  fn on_event(
    &mut self,
    config: &Config,
    store: &mut Store,
    event: &ListenerAction,
  ) -> Option<ItemEffect> {
    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _config_scope = ScopedConfig::current(config);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope_mut(store);
    let options = self.take_options();
    #[cfg(feature = "panels")]
    let _options_scope = ScopedOptions::current(options.as_deref());
    let _ = &options;

    let ctx = Context::new(host());
    let effect = self
      .inner
      .borrow_mut()
      .on_event(&ctx, &event_from_action(event));
    Some(effect_from_abi(effect))
  }

  fn initialize_with_options(
    &self,
    config: &Config,
    store: &mut Store,
    options: Option<&ComponentOptions>,
  ) -> Box<dyn Any + Send> {
    self.set_options(options);
    let options = self.take_options();

    #[cfg(feature = "panels")]
    let _options_scope = ScopedOptions::current(options.as_deref());
    let _ = &options;

    self.initialize(config, store)
  }

  fn initialize(&self, config: &Config, store: &mut Store) -> Box<dyn Any + Send> {
    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _config_scope = ScopedConfig::current(config);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope_mut(store);

    let ctx = Context::new(host());
    self.inner.borrow_mut().initialize(&ctx);
    Box::new(())
  }

  fn update(&mut self, config: &Config, store: &mut Store, _data: &dyn Any) -> Option<ItemEffect> {
    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _config_scope = ScopedConfig::current(config);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope_mut(store);
    let options = self.take_options();
    #[cfg(feature = "panels")]
    let _options_scope = ScopedOptions::current(options.as_deref());
    let _ = &options;

    let ctx = Context::new(host());
    let effect = self.inner.borrow_mut().update(&ctx);
    Some(effect_from_abi(effect))
  }

  fn handle_message(&mut self, message: &ItemMessage) -> Option<ItemEffect> {
    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let options = self.take_options();
    #[cfg(feature = "panels")]
    let _options_scope = ScopedOptions::current(options.as_deref());
    let _ = &options;
    let ctx = Context::new(host());
    let message = message_to_abi(message);
    let effect = self.inner.borrow_mut().handle_message(&ctx, &message);
    Some(effect_from_abi(effect))
  }

  fn view_with_options<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    data: &'a dyn Any,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    self.set_options(options);
    let options = self.take_options();

    #[cfg(feature = "panels")]
    let _options_scope = ScopedOptions::current(options.as_deref());
    let _ = &options;

    self.view(config, store, id, data)
  }

  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    data: &'a dyn Any,
  ) -> Element<'a, ItemMessage> {
    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _config_scope = ScopedConfig::current(config);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope(store);

    crate::state::reset_canvas_pool();
    let ctx = Context::new(host());
    let node = self.inner.borrow().view(&ctx);
    let rendered = render_nodes(
      &node,
      RenderCtx {
        id: None,
        widget_event: Some(self.event_name.clone()),
      },
    );
    let content = iced::widget::container(rendered).padding(12.0);

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

pub(crate) fn apply_plugin_calls(
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

pub(crate) fn clear_plugin_calls(
  timers: &mut Vec<(i32, u32)>,
  fds: &mut Vec<(i32, u32)>,
  store: &Store,
) {
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
