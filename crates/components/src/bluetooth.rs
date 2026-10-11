use std::sync::atomic::Ordering;

use iced::{Element, widget::container};
use slowshell_commons::bluetooth::{
  BT_CHANGED, BluetoothCmd, SharedBluetoothState, bluetooth_icon,
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{EventFilter, ItemEffect, ItemMessage},
  types::{OptionalPayloadBox, Ustr},
};
use slowshell_services::util::drain_signal_fd;
use slowshell_widgets::Icon;

use crate::{Component, ComponentContext, ComponentOptions, MenuConfig, menu_trigger};

pub struct Bluetooth {
  shared: SharedBluetoothState,
  last_revision: u64,
  shared_in_store: bool,
}

impl Bluetooth {
  pub fn new() -> Self {
    Self {
      shared: slowshell_services::bluetooth::shared(),
      last_revision: 0,
      shared_in_store: false,
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
    self.shared = slowshell_services::bluetooth::ensure(store);
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
    &'a self,
    config: &'a Config,
    _store: &'a Store,
    ctx: &ComponentContext,
    _options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let style = config.computed(&["component"]);
    let theme = config.palette();

    let icon_size = style.length("icon-size").unwrap_or(14.0) as u16;

    let text_color = style
      .color("color")
      .map(|c| c.to_iced())
      .unwrap_or(theme.text);
    let faded = style
      .color("color-faded")
      .map(|c| c.to_iced())
      .unwrap_or(theme.subtext);
    let primary = style
      .color("color-primary")
      .map(|c| c.to_iced())
      .unwrap_or(theme.primary);

    let powered = self.shared.powered.load(Ordering::Relaxed);
    let connected_count = {
      let state = self.shared.state.lock().unwrap();
      state.devices.iter().filter(|d| d.connected).count()
    };

    let icon_name = bluetooth_icon(powered, connected_count);

    let color = if !powered {
      faded
    } else if connected_count > 0 {
      primary
    } else {
      text_color
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
      config,
      ctx,
      true,
    )
  }
}
