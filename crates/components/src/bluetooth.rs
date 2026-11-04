use std::sync::atomic::Ordering;

use iced::{Element, widget::container};
use slowshell_commons::bluetooth::{BT_CHANGED, SharedBluetoothState, bluetooth_icon};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{EventFilter, ItemEffect, ItemMessage},
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
    vec![EventFilter::Named(BT_CHANGED.into())]
  }

  fn foreground_services(&self) -> Vec<String> {
    vec!["bluetooth".into()]
  }

  fn watch(&mut self, store: &mut Store, _options: Option<&ComponentOptions>) {
    self.shared = slowshell_services::bluetooth::ensure(store);
    self.shared_in_store = true;
    slowshell_services::start_foreground(store, "bluetooth");
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
