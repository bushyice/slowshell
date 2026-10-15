use std::sync::atomic::Ordering;

use iced::{
  Color, Element, Length,
  widget::{Row, container, image},
};
use slowshell_commons::tray::{SharedTrayState, TrayCmd, TrayItem, TrayPayload};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{EventFilter, ItemEffect, ItemMessage},
  types::{OptionalPayloadBox, ToUstr},
};
use slowshell_services::{tray::TRAY_TICK, util::drain_signal_fd};
use slowshell_widgets::{Icon, clickable};

use crate::{
  Component, ComponentContext, ComponentOptions, MenuConfig, popup_open_action, spaced_component,
};

pub struct SystemTray {
  last_revision: u64,
}

impl SystemTray {
  pub fn new() -> Self {
    Self { last_revision: 0 }
  }

  fn redraw_if_changed(&mut self) -> ItemEffect {
    let revision = slowshell_services::tray::shared()
      .revision
      .load(Ordering::Relaxed);
    if revision != self.last_revision {
      self.last_revision = revision;
      ItemEffect::Redraw
    } else {
      ItemEffect::None
    }
  }
}

impl Default for SystemTray {
  fn default() -> Self {
    Self::new()
  }
}

impl Component for SystemTray {
  fn events(&self) -> Vec<EventFilter> {
    vec![
      EventFilter::Named(TRAY_TICK.into()),
      EventFilter::Payload("tray.activate".into()),
    ]
  }

  fn watch(&mut self, store: &mut Store, _options: Option<&ComponentOptions>) {
    slowshell_services::tray::ensure(store);
  }

  fn update(
    &mut self,
    _config: &Config,
    store: &mut Store,
    event: &ListenerAction,
    _options: Option<&ComponentOptions>,
  ) -> miette::Result<ItemEffect> {
    if store.borrow::<SharedTrayState>().is_none() {
      store.insert(slowshell_services::tray::shared().clone());
    }

    match event {
      ListenerAction::Named(n) if &**n == TRAY_TICK => Ok(self.redraw_if_changed()),
      ListenerAction::Signal { name, fd } if &**name == TRAY_TICK => {
        drain_signal_fd(*fd);
        Ok(self.redraw_if_changed())
      }
      ListenerAction::Payload { name, payload } if name.as_ref() == "tray.activate" => {
        if let Some(payload) = payload.transform::<TrayPayload>() {
          if let Some(menu_path) = payload.menu_path.clone() {
            let _ = slowshell_services::tray::shared()
              .cmd_tx
              .send(TrayCmd::Activate {
                address: payload.address.to_string(),
                menu_path: menu_path.to_string(),
                submenu_id: payload.submenu_id,
              });
          }
        }
        Ok(ItemEffect::None)
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
    let style = config.computed(&["component"]);
    let color = style
      .color("color")
      .map(|c| c.to_iced())
      .unwrap_or(config.palette().text);
    let icon_size = style.length("icon-size").unwrap_or(16.0) as u16;
    let spacing = options
      .and_then(|o| o.number("spacing"))
      .unwrap_or_else(|| config.sizing(&["component"]).gap);

    let items = slowshell_services::tray::shared()
      .state
      .lock()
      .unwrap()
      .items
      .clone();

    let mut tray = Row::<ItemMessage>::new().spacing(spacing);
    if items.is_empty() {
      tray = tray.push(config.text("󰓄", &["component"]).color(color));
    }

    let shared = slowshell_services::tray::shared().clone();
    let position = ctx.position;
    let menu_suppressed = ctx.menu_suppressed;
    let monitor = ctx.monitor.to_ustr();
    for item in items.iter() {
      if let Some(el) = render_item(item, icon_size, color) {
        let shared = shared.clone();
        let address = item.address.clone();
        let menu_path = item.menu_path.clone();
        let monitor = monitor.clone();
        let has_menu = menu_path.is_some();
        let el = clickable(el, move |_, _, cursor| {
          if !has_menu || menu_suppressed {
            return None;
          }
          let Some(point) = cursor.position() else {
            return None;
          };
          *shared.selected.lock().unwrap() = Some(address.clone());
          shared.reset_nav();
          if let Some(path) = &menu_path {
            let _ = shared.cmd_tx.send(TrayCmd::AboutToShow {
              address: address.clone(),
              menu_path: path.clone(),
              submenu_id: 0,
            });
          }
          let action = popup_open_action(
            &MenuConfig {
              content: "menus/tray".into(),
              panel_name: None,
              position,
            },
            point,
            &monitor,
          );
          Some(ItemMessage::Action(action))
        });
        tray = tray.push(el);
      }
    }

    spaced_component(config, ctx, container(tray).into(), false)
  }
}

fn render_item<'a>(item: &TrayItem, size: u16, color: Color) -> Option<Element<'a, ItemMessage>> {
  if let Some(name) = &item.icon_name {
    let mut icon = Icon::new(name.clone()).size(size);
    if name.contains("symbolic") {
      icon = icon.color(color);
    }
    return Some(icon.into());
  }

  if let Some((w, h, rgba)) = &item.pixmap {
    if *w > 0 && *h > 0 && !rgba.is_empty() {
      let handle = Icon::<ItemMessage>::from_pixmap(&item.address, *w, *h, rgba);
      return Some(
        image(handle)
          .width(Length::Fixed(size as f32))
          .height(Length::Fixed(size as f32))
          .into(),
      );
    }
  }

  None
}
