use std::sync::atomic::{AtomicU64, Ordering};

use iced::Element;
use iced_exwlshell::reexport::{
  Anchor, BlurOption, IcedId, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{EventFilter, ItemEffect, ItemMessage, layer_size},
  types::Ustr,
};
use slowshell_desktop::{
  DeployDesktopItemAction, DeployableDesktopItem, DesktopItem, MonitorScope, UpdateWhen, Visibility,
};
use slowshell_plugin_abi::{
  Anchor as AbiAnchor, BlurOption as AbiBlurOpyion, Context, DeployResult,
  DeployableDesktopItemObjectBox, DesktopItemObjectBox, DesktopSettings, Event as AbiEvent,
  ItemEffect as AbiEffect, KeyboardInteractivity as AbiKeyboardInteractivity, Layer as AbiLayer,
  UpdateWhen as AbiUpdateWhen, Visibility as AbiVisibility,
};

use crate::{
  events::{effect_from_abi, event_from_action, filters_to_core, message_to_abi},
  host::host,
  render::{RenderCtx, render_nodes},
  renderable::{apply_plugin_calls, clear_plugin_calls},
  state::{
    PluginScope, ScopedConfig, compositor_scope, notifications_scope, store_scope, store_scope_mut,
    with_plugin_calls,
  },
};

static NEXT_INSTANCE_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) struct PluginDesktopItem {
  id: Ustr,
  plugin_id: String,
  inner: DesktopItemObjectBox,
  event_name: Ustr,
  timers: Vec<(i32, u32)>,
  fds: Vec<(i32, u32)>,
}

impl PluginDesktopItem {
  pub(crate) fn new(id: Ustr, plugin_id: String, inner: DesktopItemObjectBox) -> Self {
    let instance = NEXT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed);
    Self {
      id,
      plugin_id,
      inner,
      event_name: format!("component/plugin.{instance}").into(),
      timers: Vec::new(),
      fds: Vec::new(),
    }
  }

  fn settings(&self) -> DesktopSettings {
    self.inner.settings()
  }
}

impl DesktopItem for PluginDesktopItem {
  fn id(&self) -> &str {
    &self.id
  }

  fn layer(&self, _config: &Config, monitor: &str) -> NewLayerShellSettings {
    let settings = self.settings();

    let namespace = if settings.namespace.is_empty() {
      self.id.to_string()
    } else {
      settings.namespace.to_string()
    };

    NewLayerShellSettings {
      layer: match settings.layer {
        AbiLayer::Background => Layer::Background,
        AbiLayer::Bottom => Layer::Bottom,
        AbiLayer::Top => Layer::Top,
        AbiLayer::Overlay => Layer::Overlay,
      },
      anchor: anchor_from_bits(settings.anchor.0),
      exclusive_zone: Some(settings.exclusive_zone),
      size: layer_size(settings.width, settings.height),
      margin: Some((
        settings.margin[0],
        settings.margin[1],
        settings.margin[2],
        settings.margin[3],
      )),
      keyboard_interactivity: match settings.keyboard_interactivity {
        AbiKeyboardInteractivity::OnDemand => KeyboardInteractivity::OnDemand,
        AbiKeyboardInteractivity::Exclusive => KeyboardInteractivity::Exclusive,
        AbiKeyboardInteractivity::None => KeyboardInteractivity::None,
      },
      events_transparent: settings.events_transparent,
      namespace: Some(namespace),
      output_option: if monitor.is_empty() {
        OutputOption::Active
      } else {
        OutputOption::OutputName(monitor.to_owned())
      },
      blur_option: match settings.blur {
        AbiBlurOpyion::None => BlurOption::None,
        AbiBlurOpyion::Full => BlurOption::FullRegion,
      },
    }
  }

  fn visibility(&self) -> Visibility {
    match self.settings().visibility {
      AbiVisibility::Transient => Visibility::Transient,
      AbiVisibility::Toggleable(visible) => Visibility::Toggleable(visible),
      AbiVisibility::Visible => Visibility::Visible,
    }
  }

  fn update_strategy(&self) -> UpdateWhen {
    match self.settings().update_when {
      AbiUpdateWhen::EveryFrame => UpdateWhen::EveryFrame,
      AbiUpdateWhen::OnEvent => UpdateWhen::OnEvent,
      AbiUpdateWhen::OnDemand => UpdateWhen::OnDemand,
    }
  }

  fn monitor(&self, _config: &Config) -> MonitorScope {
    let settings = self.settings();
    if settings.per_monitor {
      MonitorScope::PerMonitor
    } else if settings.monitor.is_empty() {
      MonitorScope::Single(None)
    } else {
      MonitorScope::Single(Some(settings.monitor.to_string().into()))
    }
  }

  fn init_events(&self) -> Vec<EventFilter> {
    filters_to_core(&self.inner.events(), &self.event_name)
  }

  fn initialize(&mut self, store: &mut Store) -> miette::Result<()> {
    clear_plugin_calls(&mut self.timers, &mut self.fds, store);

    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _store_scope = store_scope_mut(store);

    let ctx = Context::new(host());
    let (_, calls) = with_plugin_calls(|| {
      self.inner.initialize(&ctx);
    });
    apply_plugin_calls(
      &self.event_name,
      &mut self.timers,
      &mut self.fds,
      store,
      calls,
    );
    Ok(())
  }

  fn update(
    &mut self,
    config: &Config,
    store: &mut Store,
    event: &ListenerAction,
  ) -> miette::Result<ItemEffect> {
    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _config_scope = ScopedConfig::current(config);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope_mut(store);

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

    let redraw = apply_plugin_calls(
      &self.event_name,
      &mut self.timers,
      &mut self.fds,
      store,
      calls,
    );

    if effect == AbiEffect::Subscribe {
      return Ok(ItemEffect::Subscribe(self.init_events()));
    }
    let mut effect = effect_from_abi(effect);
    if effect == ItemEffect::None && (targeted || redraw) {
      effect = ItemEffect::Redraw;
    }
    Ok(effect)
  }

  fn handle_message(&mut self, store: Option<&mut Store>, message: &ItemMessage) -> ItemEffect {
    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _compositor_scope = store.as_deref().map(compositor_scope);
    let _notifications_scope = store.as_deref().map(notifications_scope);
    let _store_scope = store.as_deref().map(store_scope);

    let ctx = Context::new(host());
    let message = message_to_abi(message);
    effect_from_abi(self.inner.handle_message(&ctx, &message))
  }

  fn view<'a>(
    &'a self,
    config: &'a Config,
    store: &'a Store,
    _id: IcedId,
    _monitor: &str,
  ) -> Element<'a, ItemMessage> {
    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _config_scope = ScopedConfig::current(config);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope(store);

    crate::state::reset_canvas_pool();
    let ctx = Context::new(host());
    let node = self.inner.view(&ctx);
    render_nodes(
      &node,
      RenderCtx {
        id: None,
        widget_event: Some(self.event_name.clone()),
      },
    )
  }
}

pub(crate) struct PluginDeployable {
  id: Ustr,
  plugin_id: String,
  inner: DeployableDesktopItemObjectBox,
  event_name: Ustr,
}

impl PluginDeployable {
  pub(crate) fn new(id: Ustr, plugin_id: String, inner: DeployableDesktopItemObjectBox) -> Self {
    let instance = NEXT_INSTANCE_ID.fetch_add(1, Ordering::Relaxed);
    Self {
      id,
      plugin_id,
      inner,
      event_name: format!("component/plugin.{instance}").into(),
    }
  }
}

impl DeployableDesktopItem for PluginDeployable {
  fn id(&self) -> &str {
    &self.id
  }

  fn deploys_on(&self) -> Vec<EventFilter> {
    filters_to_core(&self.inner.events(), &self.event_name)
  }

  fn deploy(
    &mut self,
    config: &Config,
    store: &mut Store,
    action: &ListenerAction,
  ) -> miette::Result<Option<DeployDesktopItemAction>> {
    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _config_scope = ScopedConfig::current(config);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope_mut(store);

    let ctx = Context::new(host());
    let result = with_plugin_calls(|| self.inner.deploy(&ctx, &event_from_action(action))).0;

    Ok(deploy_action(result, &self.plugin_id))
  }
}

fn deploy_action(result: DeployResult, plugin_id: &str) -> Option<DeployDesktopItemAction> {
  let mut actions = Vec::new();
  collect_deploy_actions(result, plugin_id, &mut actions);

  if actions.is_empty() {
    None
  } else {
    Some(DeployDesktopItemAction::Many(actions))
  }
}

fn collect_deploy_actions(
  result: DeployResult,
  plugin_id: &str,
  actions: &mut Vec<DeployDesktopItemAction>,
) {
  match result {
    DeployResult::None => {}
    DeployResult::Deploy(items) => {
      for item in items {
        actions.push(DeployDesktopItemAction::Deploy(
          Box::new(PluginDesktopItem::new(
            item.id.to_string().into(),
            plugin_id.to_owned(),
            item.item,
          )) as Box<dyn DesktopItem>,
        ));
      }
    }
    DeployResult::Destroy(id) => {
      actions.push(DeployDesktopItemAction::Destroy(id.to_string().into()));
    }
    DeployResult::Many(results) => {
      for result in results {
        collect_deploy_actions(result, plugin_id, actions);
      }
    }
  }
}

fn anchor_from_bits(bits: u32) -> Anchor {
  let mut anchor = Anchor::empty();
  if bits & AbiAnchor::TOP.0 != 0 {
    anchor |= Anchor::Top;
  }
  if bits & AbiAnchor::BOTTOM.0 != 0 {
    anchor |= Anchor::Bottom;
  }
  if bits & AbiAnchor::LEFT.0 != 0 {
    anchor |= Anchor::Left;
  }
  if bits & AbiAnchor::RIGHT.0 != 0 {
    anchor |= Anchor::Right;
  }
  anchor
}
