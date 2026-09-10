use std::{
  os::fd::AsRawFd,
  sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
  },
};

use iced::{Element, widget::container};
use nix::{
  fcntl::{FcntlArg, OFlag, fcntl},
  sys::epoll::EpollFlags,
};
use slowshell_commons::network::{
  NET_CHANGED, NetCmd, NetworkPayload, NetworkState, SharedNetworkState, signal_icon,
};
use slowshell_commons::panels::PanelOrientation;
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::{FdHandle, ListenerAction},
  message::{EventFilter, ItemEffect, ItemMessage},
  types::OptionalPayloadBox,
};
use slowshell_services::util::drain_signal_fd;
use slowshell_widgets::Icon;

use crate::{
  Component, ComponentContext, ComponentOptions, MenuConfig, menu_trigger, vertical_text,
};

pub struct Network {
  signal_fd: Option<i32>,
  started: bool,
  shared: SharedNetworkState,
  last_revision: u64,
  shared_in_store: bool,
  cmd_rx: Option<tokio::sync::mpsc::UnboundedReceiver<NetCmd>>,
}

impl Network {
  pub fn new() -> Self {
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    Self {
      signal_fd: None,
      started: false,
      shared: Arc::new(NetworkState {
        wifi_enabled: AtomicBool::new(true),
        wifi_hardware_enabled: AtomicBool::new(true),
        scanning: AtomicBool::new(false),
        busy: AtomicBool::new(false),
        state: Mutex::new(Default::default()),
        menu: Mutex::new(Default::default()),
        saved_ssids: Mutex::new(None),
        revision: AtomicU64::new(0),
        cmd_tx,
      }),
      last_revision: 0,
      shared_in_store: false,
      cmd_rx: Some(cmd_rx),
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
    store.insert(self.shared.clone());
    self.shared_in_store = true;

    if self.started {
      return;
    }
    self.started = true;

    let notify = store.borrow::<FdHandle>().and_then(|handle| {
      let (read_fd, write_fd) = nix::unistd::pipe().ok()?;
      fcntl(&read_fd, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).ok()?;

      let raw = read_fd.as_raw_fd();
      handle.watch_with_flags(
        read_fd,
        EpollFlags::EPOLLIN | EpollFlags::EPOLLET,
        ListenerAction::Signal {
          name: NET_CHANGED.into(),
          fd: raw,
        },
      );
      self.signal_fd = Some(raw);

      Some(write_fd)
    });

    let shared = self.shared.clone();
    let cmd_rx = self.cmd_rx.take();
    if let Some(cmd_rx) = cmd_rx {
      slowshell_services::network::run(shared, cmd_rx, notify);
    }
  }

  fn stop(&mut self, store: &Store, _options: Option<&ComponentOptions>) {
    if let (Some(handle), Some(fd)) = (store.borrow::<FdHandle>(), self.signal_fd.take()) {
      handle.stop_timer(fd);
    }
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
      ListenerAction::Signal { name, fd }
        if &**name == NET_CHANGED && Some(*fd) == self.signal_fd =>
      {
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
    &self,
    config: &Config,
    _store: &Store,
    ctx: &ComponentContext,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let style = ctx.resolve_style(config);
    let theme = &config.theme;
    let icon_size = style.number("icon.size").unwrap_or(14.0) as u16;
    let font_size = style.number("font.size").unwrap_or(12.0);

    let inner = self.shared.state.lock().unwrap().clone();
    let wifi_enabled = self.shared.wifi_enabled.load(Ordering::Relaxed);

    let (icon, color) = if let Some(wifi) = &inner.wifi_connected {
      (
        signal_icon(wifi.signal),
        style.color(theme, "color", theme.text),
      )
    } else if inner.ethernet.as_ref().is_some_and(|e| e.connected) {
      (
        "network-wired-symbolic",
        style.color(theme, "color", theme.text),
      )
    } else if !wifi_enabled {
      (
        "network-wireless-disabled-symbolic",
        style
          .get_color(theme, "color.offline")
          .unwrap_or(theme.subtext),
      )
    } else {
      (
        "network-wireless-offline-symbolic",
        style
          .get_color(theme, "color.offline")
          .unwrap_or(theme.subtext),
      )
    };

    let show_icon = options.and_then(|o| o.bool("icon")).unwrap_or(true);
    let show_ssid = options.and_then(|o| o.bool("ssid")).unwrap_or(false);

    let label = match (&inner.wifi_connected, &inner.ethernet) {
      (Some(wifi), _) if !wifi.ssid.is_empty() => Some(wifi.ssid.clone()),
      (Some(_), _) => Some("Network".to_string()),
      (None, Some(eth)) if eth.connected => Some("Ethernet".to_string()),
      (None, _) if show_ssid => Some("Network".to_string()),
      _ => None,
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
      if let Some(label) = label {
        items.push(vertical_text(&label, font_size, color));
      }
      iced::widget::column(items)
        .spacing(4)
        .align_x(iced::Alignment::Center)
        .into()
    } else {
      match (show_icon, show_ssid, label) {
        (true, true, Some(label)) => container(
          iced::widget::row![
            Icon::new(icon).color(color).size(icon_size),
            iced::widget::text(label).size(font_size).color(color)
          ]
          .spacing(4)
          .align_y(iced::Alignment::Center),
        )
        .into(),
        (_, true, Some(label)) => {
          container(iced::widget::text(label).size(font_size).color(color)).into()
        }
        (true, ..) => container(Icon::new(icon).color(color).size(icon_size)).into(),
        _ => container(Icon::new(icon).color(color).size(icon_size)).into(),
      }
    };

    menu_trigger(
      content,
      MenuConfig {
        content: "menus/network".into(),
        panel_name: None,
        position: ctx.position,
      },
    )
  }
}
