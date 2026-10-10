use std::sync::atomic::{AtomicU64, Ordering};

use iced::Element;
use slowshell_components::{Component, ComponentContext, ComponentOptions, spaced_component};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{EventFilter, ItemEffect as CoreEffect, ItemMessage},
  types::Ustr,
};
use slowshell_plugin_abi::{
  ComponentObjectBox, Context, Event as AbiEvent, ItemEffect as AbiEffect,
};

use crate::{
  events::{effect_from_abi, event_from_action, filters_to_core},
  host::host,
  render::{RenderCtx, render_nodes},
  renderable::{apply_plugin_calls, clear_plugin_calls},
  state::{
    PluginScope, ScopedConfig, ScopedOptions, compositor_scope, notifications_scope,
    store_scope_mut, with_plugin_calls,
  },
};

static NEXT_INSTANCE_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) struct PluginComponent {
  plugin_id: String,
  inner: ComponentObjectBox,
  event_name: Ustr,
  timers: Vec<(i32, u32)>,
  fds: Vec<(i32, u32)>,
  hoverable: bool,
}

impl PluginComponent {
  pub(crate) fn new(plugin_id: String, inner: ComponentObjectBox, hoverable: bool) -> Self {
    let id = NEXT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed);
    Self {
      plugin_id,
      inner,
      event_name: format!("component/plugin.{id}").into(),
      timers: Vec::new(),
      fds: Vec::new(),
      hoverable,
    }
  }
}

impl Component for PluginComponent {
  fn events(&self) -> Vec<EventFilter> {
    filters_to_core(&self.inner.events(), &self.event_name)
  }

  fn check_view(&self, store: &Store, options: Option<&ComponentOptions>) -> bool {
    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = crate::state::store_scope(store);
    let _options_scope = ScopedOptions::current(options);

    let ctx = Context::new(host());
    self.inner.check_view(&ctx)
  }

  fn watch(&mut self, store: &mut Store, options: Option<&ComponentOptions>) {
    clear_plugin_calls(&mut self.timers, &mut self.fds, store);

    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _store_scope = store_scope_mut(store);
    let _options_scope = ScopedOptions::current(options);
    let ctx = Context::new(host());

    let (_, calls) = with_plugin_calls(|| {
      self.inner.watch(&ctx);
    });
    self.apply_calls(store, calls);
  }

  fn stop(&mut self, store: &Store, options: Option<&ComponentOptions>) {
    clear_plugin_calls(&mut self.timers, &mut self.fds, store);

    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _store_scope = crate::state::store_scope(store);
    let _options_scope = ScopedOptions::current(options);
    let ctx = Context::new(host());
    self.inner.stop(&ctx);
  }

  fn update(
    &mut self,
    config: &Config,
    store: &mut Store,
    event: &ListenerAction,
    options: Option<&ComponentOptions>,
  ) -> miette::Result<CoreEffect> {
    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _config_scope = ScopedConfig::current(config);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope_mut(store);
    let _options_scope = ScopedOptions::current(options);

    let targeted = match event {
      ListenerAction::Timer { name, .. } | ListenerAction::Signal { name, .. } => {
        &**name == &*self.event_name
      }
      ListenerAction::Payload { name, .. } => &**name == &*self.event_name,
      _ => false,
    };

    let widget_event = match event {
      ListenerAction::Payload { name, payload } if &**name == &*self.event_name => {
        crate::events::widget_from_payload(payload)
      }
      _ => None,
    };

    let ctx = Context::new(host());
    let mut effect = AbiEffect::None;
    let (_, calls) = with_plugin_calls(|| {
      if let Some(widget_event) = widget_event {
        effect = self.inner.update(&ctx, &AbiEvent::Widget(widget_event));
      } else if let ListenerAction::Payload { name, payload } = event
        && &**name == &*self.event_name
      {
        effect = self
          .inner
          .update(&ctx, &crate::events::payload_event(name, payload));
      } else if targeted {
        let (kind, action, fd) = match event {
          ListenerAction::Signal { fd, .. } => {
            let action = self
              .fds
              .iter()
              .find(|(registered, _)| registered == fd)
              .map(|(_, action)| *action)
              .unwrap_or(0);
            (1u32, action, *fd)
          }
          ListenerAction::Timer { fd, .. } => {
            let tag = self
              .timers
              .iter()
              .find(|(registered, _)| registered == fd)
              .map(|(_, tag)| *tag)
              .unwrap_or(0);
            (0u32, tag, *fd)
          }
          _ => (0u32, 0, -1),
        };

        let abi_event = if kind == 0 {
          AbiEvent::Tick { tag: action, fd }
        } else {
          AbiEvent::Fd { action, fd }
        };
        effect = self.inner.update(&ctx, &abi_event);
      } else {
        effect = self.inner.update(&ctx, &event_from_action(event));
      }
    });

    let redraw = self.apply_calls(store, calls);
    if effect == AbiEffect::Subscribe {
      return Ok(CoreEffect::Subscribe(self.events()));
    }
    let mut effect = effect_from_abi(effect);
    if effect == CoreEffect::None && (targeted || redraw) {
      effect = CoreEffect::Redraw;
    }
    Ok(effect)
  }

  fn view<'a>(
    &'a self,
    config: &'a Config,
    store: &'a Store,
    ctx: &ComponentContext,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _config_scope = ScopedConfig::current(config);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = crate::state::store_scope(store);
    let _options_scope = ScopedOptions::current(options);

    crate::state::reset_canvas_pool();
    let host_ctx = Context::new(host());
    let node = self.inner.view(&host_ctx);

    let render_ctx = RenderCtx {
      id: None,
      widget_event: Some(self.event_name.clone()),
    };
    let rendered = render_nodes(&node, render_ctx);
    spaced_component(config, ctx, rendered, self.hoverable)
  }
}

impl PluginComponent {
  fn apply_calls(&mut self, store: &mut Store, calls: crate::state::PluginCalls) -> bool {
    apply_plugin_calls(
      &self.event_name,
      &mut self.timers,
      &mut self.fds,
      store,
      calls,
    )
  }
}
