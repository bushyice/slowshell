use std::sync::atomic::Ordering;

use iced::{Element, widget::container};
use slowshell_commons::network::{
  NET_CHANGED, NetCmd, NetworkPayload, SharedNetworkState, signal_icon,
};
use slowshell_commons::panels::PanelOrientation;
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{EventFilter, ItemEffect, ItemMessage},
  types::OptionalPayloadBox,
};
use slowshell_services::util::drain_signal_fd;
use slowshell_widgets::Icon;

use crate::{
  Component, ComponentContext, ComponentOptions, MenuConfig, menu_trigger, vertical_text,
};

pub struct Network {
  shared: SharedNetworkState,
  last_revision: u64,
  shared_in_store: bool,
}

impl Network {
  pub fn new() -> Self {
    Self {
      shared: slowshell_services::network::shared(),
      last_revision: 0,
      shared_in_store: false,
    }
  }
}

impl Default for Network {
  fn default() -> Self {
    Self::new()
  }
}

impl Network {
  fn clear_menu(&self) {
    let mut menu = self.shared.menu.lock().unwrap();
    menu.selected_ssid = None;
    menu.password.clear();
    menu.info_ssid = None;
  }
}

impl Component for Network {
  fn events(&self) -> Vec<EventFilter> {
    vec![
      EventFilter::Named(NET_CHANGED.into()),
      EventFilter::Payload("network.connect".into()),
      EventFilter::Payload("network.disconnect".into()),
      EventFilter::Payload("network.wired".into()),
      EventFilter::Payload("network.toggle".into()),
      EventFilter::Payload("network.scan".into()),
      EventFilter::Payload("network.select".into()),
      EventFilter::Payload("network.password".into()),
      EventFilter::Payload("network.info".into()),
      EventFilter::Payload("network.info.back".into()),
      EventFilter::Payload("network.edit".into()),
    ]
  }

  fn watch(&mut self, store: &mut Store, _options: Option<&ComponentOptions>) {
    self.shared = slowshell_services::network::ensure(store);
    self.shared_in_store = true;
  }

  fn update(
    &mut self,
    _config: &Config,
    store: &mut Store,
    event: &ListenerAction,
    _options: Option<&ComponentOptions>,
  ) -> miette::Result<ItemEffect> {
    if !self.shared_in_store {
      self.shared_in_store = true;
      store.insert(self.shared.clone());
    }

    match event {
      ListenerAction::Signal { name, fd } if &**name == NET_CHANGED => {
        drain_signal_fd(*fd);
        let revision = self.shared.revision.load(Ordering::Relaxed);
        if revision != self.last_revision {
          self.last_revision = revision;
          return Ok(ItemEffect::Redraw);
        }
        Ok(ItemEffect::None)
      }
      ListenerAction::Payload { name, payload } if name.as_ref() == "network.connect" => {
        if let Some(payload) = payload.transform::<NetworkPayload>() {
          let _ = self.shared.cmd_tx.send(NetCmd::Connect {
            ssid: payload.ssid.clone(),
            password: payload.password.clone(),
          });
        }
        Ok(ItemEffect::Redraw)
      }
      ListenerAction::Payload { name, .. } if name.as_ref() == "network.disconnect" => {
        let _ = self.shared.cmd_tx.send(NetCmd::DisconnectWifi);
        self.clear_menu();
        Ok(ItemEffect::Redraw)
      }
      ListenerAction::Payload { name, .. } if name.as_ref() == "network.wired" => {
        let _ = self.shared.cmd_tx.send(NetCmd::WiredConnect);
        self.clear_menu();
        Ok(ItemEffect::Redraw)
      }
      ListenerAction::Payload { name, .. } if name.as_ref() == "network.toggle" => {
        let _ = self.shared.cmd_tx.send(NetCmd::ToggleWifi);
        Ok(ItemEffect::Redraw)
      }
      ListenerAction::Payload { name, .. } if name.as_ref() == "network.scan" => {
        let _ = self.shared.cmd_tx.send(NetCmd::Scan);
        Ok(ItemEffect::Redraw)
      }
      ListenerAction::Payload { name, payload } if name.as_ref() == "network.info" => {
        if let Some(payload) = payload.transform::<NetworkPayload>() {
          let mut menu = self.shared.menu.lock().unwrap();
          menu.info_ssid = Some(payload.ssid.clone());
          menu.selected_ssid = None;
          menu.password.clear();
        }
        Ok(ItemEffect::Redraw)
      }
      ListenerAction::Payload { name, .. } if name.as_ref() == "network.info.back" => {
        self.shared.menu.lock().unwrap().info_ssid = None;
        Ok(ItemEffect::Redraw)
      }
      ListenerAction::Payload { name, .. } if name.as_ref() == "network.edit" => {
        match std::process::Command::new("nm-connection-editor").spawn() {
          Ok(_) => {}
          Err(e) => eprintln!("[network] failed to launch nm-connection-editor: {e}"),
        }
        Ok(ItemEffect::Redraw)
      }
      ListenerAction::Payload { name, payload } if name.as_ref() == "network.select" => {
        if let Some(payload) = payload.transform::<NetworkPayload>() {
          let mut menu = self.shared.menu.lock().unwrap();
          menu.selected_ssid = Some(payload.ssid.clone());
          menu.password.clear();
        }
        Ok(ItemEffect::Redraw)
      }
      ListenerAction::Payload { name, payload } if name.as_ref() == "network.password" => {
        if let Some(payload) = payload
          .transform::<NetworkPayload>()
          .and_then(|x| x.value.clone())
        {
          self.shared.menu.lock().unwrap().password = payload;
        }
        Ok(ItemEffect::Redraw)
      }
      _ => Ok(ItemEffect::None),
    }
  }

  fn view<'a>(
    &'a self,
    config: &'a Config,
    _store: &'a Store,
    ctx: &ComponentContext,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let style = config.computed(ctx.classes());
    let theme = config.palette();
    let icon_size = style.length("icon-size").unwrap_or(14.0) as u16;
    let font_size = config.sizing(ctx.classes()).font_size;

    let color = style
      .color("color")
      .map(|c| c.to_iced())
      .unwrap_or(theme.text);
    let offline = style
      .color("color-offline")
      .map(|c| c.to_iced())
      .unwrap_or(theme.subtext);

    let inner = self.shared.state.lock().unwrap().clone();
    let wifi_enabled = self.shared.wifi_enabled.load(Ordering::Relaxed);

    let (icon, color) = if let Some(wifi) = &inner.wifi_connected {
      (signal_icon(wifi.signal), color)
    } else if inner.ethernet.as_ref().is_some_and(|e| e.connected) {
      ("network-wired-symbolic", color)
    } else if !wifi_enabled {
      ("network-wireless-disabled-symbolic", offline)
    } else {
      ("network-wireless-offline-symbolic", offline)
    };

    let show_icon = options.and_then(|o| o.bool("icon")).unwrap_or(true);
    let show_ssid = options.and_then(|o| o.bool("ssid")).unwrap_or(false);

    let label = match (&inner.wifi_connected, &inner.ethernet) {
      (Some(wifi), _) if !wifi.ssid.is_empty() => Some(wifi.ssid.clone()),
      (Some(_), _) => Some("Network".to_string()),
      (None, Some(eth)) if eth.connected => Some("Ethernet".to_string()),
      (None, _) => Some("Network".to_string()),
    };

    let icon_el: Option<Element<'a, ItemMessage>> = if show_icon {
      Some(Icon::new(icon).color(color).size(icon_size).into())
    } else {
      None
    };

    let vertical = ctx.orientation == PanelOrientation::Vertical;

    let content: Element<'a, ItemMessage> = if vertical {
      let mut items: Vec<Element<'a, ItemMessage>> = Vec::new();
      if let Some(icon) = icon_el {
        items.push(icon);
      }
      if show_ssid && let Some(label) = label {
        items.push(vertical_text(&label, font_size, color));
      }
      iced::widget::column(items)
        .spacing(4)
        .align_x(iced::Alignment::Center)
        .into()
    } else {
      let mut items: Vec<Element<'a, ItemMessage>> = Vec::new();
      if let Some(icon) = icon_el {
        items.push(icon);
      }
      if show_ssid && let Some(label) = label {
        items.push(config.text(label, ctx.classes()).color(color).into());
      }
      if items.is_empty() {
        container(iced::widget::Space::new()).into()
      } else {
        container(
          iced::widget::row(items)
            .spacing(4)
            .align_y(iced::Alignment::Center),
        )
        .into()
      }
    };

    menu_trigger(
      content,
      MenuConfig {
        content: "menus/network".into(),
        panel_name: None,
        position: ctx.position,
      },
      config,
      ctx,
      true,
    )
  }
}
