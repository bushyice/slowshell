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
use slowshell_commons::bluetooth::{
  BT_CHANGED, BluetoothCmd, BluetoothState, SharedBluetoothState, bluetooth_icon,
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::{FdHandle, ListenerAction},
  message::{EventFilter, ItemEffect, ItemMessage},
  types::{OptionalPayloadBox, Ustr},
};
use slowshell_services::util::drain_signal_fd;
use slowshell_widgets::Icon;

use crate::{Component, ComponentContext, ComponentOptions, MenuConfig, menu_trigger};

pub struct Bluetooth {
  signal_fd: Option<i32>,
  started: bool,
  shared: SharedBluetoothState,
  last_revision: u64,
  shared_in_store: bool,
  cmd_rx: Option<tokio::sync::mpsc::UnboundedReceiver<BluetoothCmd>>,
}

impl Bluetooth {
  pub fn new() -> Self {
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    Self {
      signal_fd: None,
      started: false,
      shared: Arc::new(BluetoothState {
        powered: AtomicBool::new(false),
        discovering: AtomicBool::new(false),
        state: Mutex::new(Default::default()),
        menu: Mutex::new(Default::default()),
        revision: AtomicU64::new(0),
        cmd_tx,
      }),
      last_revision: 0,
      shared_in_store: false,
      cmd_rx: Some(cmd_rx),
    }
  }
}

impl Default for Bluetooth {
  fn default() -> Self {
    Self::new()
  }
}

impl Component for Bluetooth {
  fn events(&self) -> Vec<EventFilter> {
    vec![
      EventFilter::Named(BT_CHANGED.into()),
      EventFilter::Payload("bluetooth.toggle".into()),
      EventFilter::Payload("bluetooth.scan".into()),
      EventFilter::Payload("bluetooth.connect".into()),
      EventFilter::Payload("bluetooth.disconnect".into()),
      EventFilter::Payload("bluetooth.pair".into()),
      EventFilter::Payload("bluetooth.remove".into()),
      EventFilter::Payload("bluetooth.confirm_remove".into()),
      EventFilter::Payload("bluetooth.trust".into()),
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
          name: BT_CHANGED.into(),
          fd: raw,
        },
      );
      self.signal_fd = Some(raw);

      Some(write_fd)
    });

    let shared = self.shared.clone();
    let cmd_rx = self.cmd_rx.take();
    if let Some(cmd_rx) = cmd_rx {
      slowshell_services::bluetooth::run(shared, cmd_rx, notify);
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
  ) -> anyhow::Result<ItemEffect> {
    if !self.shared_in_store {
      self.shared_in_store = true;
      store.insert(self.shared.clone());
    }

    match event {
      ListenerAction::Signal { name, fd } if &**name == BT_CHANGED => {
        drain_signal_fd(*fd);
        let rev = self.shared.revision.load(Ordering::SeqCst);
        if rev != self.last_revision {
          self.last_revision = rev;
          Ok(ItemEffect::Redraw)
        } else {
          Ok(ItemEffect::None)
        }
      }

      ListenerAction::Named(name) if &**name == BT_CHANGED => {
        if let Some(fd) = self.signal_fd {
          drain_signal_fd(fd);
        }
        let rev = self.shared.revision.load(Ordering::SeqCst);
        if rev != self.last_revision {
          self.last_revision = rev;
          Ok(ItemEffect::Redraw)
        } else {
          Ok(ItemEffect::None)
        }
      }

      ListenerAction::Payload { name, payload } if &**name == "bluetooth.toggle" => {
        let _ = self.shared.cmd_tx.send(BluetoothCmd::TogglePower);
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, .. } if &**name == "bluetooth.scan" => {
        let discovering = self.shared.discovering.load(Ordering::Relaxed);
        if discovering {
          let _ = self.shared.cmd_tx.send(BluetoothCmd::StopDiscovery);
        } else {
          let _ = self.shared.cmd_tx.send(BluetoothCmd::StartDiscovery);
        }
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, payload } if &**name == "bluetooth.connect" => {
        if let Some(addr) = payload.transform::<Ustr>() {
          let _ = self
            .shared
            .cmd_tx
            .send(BluetoothCmd::Connect(addr.to_string()));
        }
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, payload } if &**name == "bluetooth.disconnect" => {
        if let Some(addr) = payload.transform::<Ustr>() {
          let _ = self
            .shared
            .cmd_tx
            .send(BluetoothCmd::Disconnect(addr.to_string()));
        }
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, payload } if &**name == "bluetooth.pair" => {
        if let Some(addr) = payload.transform::<Ustr>() {
          let _ = self
            .shared
            .cmd_tx
            .send(BluetoothCmd::Pair(addr.to_string()));
        }
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, payload } if &**name == "bluetooth.remove" => {
        if let Some(addr) = payload.transform::<Ustr>() {
          let _ = self
            .shared
            .cmd_tx
            .send(BluetoothCmd::Remove(addr.to_string()));
        }
        let mut menu = self.shared.menu.lock().unwrap();
        menu.confirming_remove = None;
        drop(menu);
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, payload } if &**name == "bluetooth.confirm_remove" => {
        let mut menu = self.shared.menu.lock().unwrap();
        if let Some(addr) = payload.transform::<Ustr>() {
          let addr = addr.to_string();
          menu.confirming_remove = if menu.confirming_remove.as_deref() == Some(&addr) {
            None
          } else {
            Some(addr)
          };
        }
        drop(menu);
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, payload } if &**name == "bluetooth.trust" => {
        if let Some((addr, trust)) = payload.transform::<(Ustr, bool)>() {
          let _ = self
            .shared
            .cmd_tx
            .send(BluetoothCmd::Trust(addr.to_string(), *trust));
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
    _options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let style = config.style("component");
    let theme = &config.theme;

    let icon_size = style.number("icon.size").unwrap_or(14.0) as u16;

    let powered = self.shared.powered.load(Ordering::Relaxed);
    let connected_count = {
      let state = self.shared.state.lock().unwrap();
      state.devices.iter().filter(|d| d.connected).count()
    };

    let icon_name = bluetooth_icon(powered, connected_count);

    let color = if !powered {
      style.color(theme, "color.faded", theme.subtext)
    } else if connected_count > 0 {
      style.color(theme, "color.primary", theme.primary)
    } else {
      style.color(theme, "color", theme.text)
    };

    let icon: Element<'a, ItemMessage> =
      Icon::new(icon_name).size(icon_size - 1).color(color).into();

    let content = container(icon);

    menu_trigger(
      content.into(),
      MenuConfig {
        content: "menus/bluetooth".into(),
        panel_name: None,
        position: ctx.position,
      },
    )
  }
}
