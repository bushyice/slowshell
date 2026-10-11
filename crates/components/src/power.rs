use std::sync::atomic::Ordering;

use iced::{
  Alignment, Element, Event, mouse,
  widget::{container, row},
};
use slowshell_commons::power::{
  POWER_CHANGED, PowerCmd, SharedPowerState, battery_icon, brightness_icon,
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{EventFilter, ItemEffect, ItemMessage},
  types::{OptionalPayloadBox, PayloadBox, ToUstr, Ustr},
};
use slowshell_services::util::drain_signal_fd;
use slowshell_widgets::{EventWrapper, Icon};

use crate::{
  Component, ComponentContext, ComponentOptions, MenuConfig, popup_open_action, spaced_component,
};

pub struct Power {
  shared: SharedPowerState,
  last_revision: u64,
  shared_in_store: bool,
}

impl Power {
  pub fn new() -> Self {
    Self {
      shared: slowshell_services::power::shared(),
      last_revision: 0,
      shared_in_store: false,
    }
  }

  pub fn with_shared(shared: SharedPowerState) -> Self {
    Self {
      shared,
      last_revision: 0,
      shared_in_store: false,
    }
  }
}

impl Default for Power {
  fn default() -> Self {
    Self::new()
  }
}

impl Component for Power {
  fn events(&self) -> Vec<EventFilter> {
    vec![
      EventFilter::Named(POWER_CHANGED.into()),
      EventFilter::Payload("power.set_brightness".into()),
      EventFilter::Payload("power.step_brightness".into()),
      EventFilter::Payload("power.set_profile".into()),
    ]
  }

  fn watch(&mut self, store: &mut Store, _options: Option<&ComponentOptions>) {
    self.shared = slowshell_services::power::ensure(store);
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
      if let Some(existing) = store.borrow::<SharedPowerState>() {
        self.shared = existing.clone();
      } else {
        store.insert(self.shared.clone());
      }
      self.shared_in_store = true;
    }

    match event {
      ListenerAction::Signal { name, fd } if &**name == POWER_CHANGED => {
        drain_signal_fd(*fd);
        let rev = self.shared.revision.load(Ordering::SeqCst);
        if rev != self.last_revision {
          self.last_revision = rev;
          Ok(ItemEffect::Redraw)
        } else {
          Ok(ItemEffect::None)
        }
      }

      ListenerAction::Named(name) if &**name == POWER_CHANGED => {
        let rev = self.shared.revision.load(Ordering::SeqCst);
        if rev != self.last_revision {
          self.last_revision = rev;
          Ok(ItemEffect::Redraw)
        } else {
          Ok(ItemEffect::None)
        }
      }

      ListenerAction::Payload { name, payload } if &**name == "power.set_brightness" => {
        if let Some(pct) = payload.transform::<u8>() {
          let _ = self.shared.cmd_tx.send(PowerCmd::SetBrightness(*pct));
        }
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, payload } if &**name == "power.step_brightness" => {
        if let Some(delta) = payload.transform::<i32>() {
          let _ = self.shared.cmd_tx.send(PowerCmd::StepBrightness(*delta));
        }
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, payload } if &**name == "power.set_profile" => {
        if let Some(prof_str) = payload.transform::<Ustr>() {
          let prof = slowshell_commons::power::PowerProfile::from_str(&**prof_str);
          let _ = self.shared.cmd_tx.send(PowerCmd::SetProfile(prof));
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
    let style = config.computed(&["component"]);
    let theme = config.palette();

    let icon_size = style.length("icon-size").unwrap_or(14.0) as u16;
    let color = style
      .color("color")
      .map(|c| c.to_iced())
      .unwrap_or(theme.text);
    let faded = style
      .color("color-faded")
      .map(|c| c.to_iced())
      .unwrap_or(theme.subtext);
    let sec_color = style
      .color("color-secondary")
      .map(|c| c.to_iced())
      .unwrap_or(theme.yellow);
    let primary = style
      .color("color-primary")
      .map(|c| c.to_iced())
      .unwrap_or(theme.primary);
    let danger = style
      .color("color-danger")
      .map(|c| c.to_iced())
      .unwrap_or(theme.red);

    let opt_battery = options.and_then(|o| o.bool("battery"));
    let opt_brightness = options.and_then(|o| o.bool("brightness"));

    let (show_battery, show_brightness) = match (opt_battery, opt_brightness) {
      (Some(b), Some(br)) => (b, br),
      (Some(b), None) => (b, false),
      (None, Some(br)) => (false, br),
      (None, None) => (true, true),
    };

    let data = self.shared.data.lock().unwrap();

    let mut row_items = Vec::new();

    if show_brightness {
      let b_icon = Icon::new(brightness_icon(data.brightness_percent))
        .size(icon_size)
        .color(sec_color);
      let b_label = config
        .text(format!("{}%", data.brightness_percent), &["component"])
        .color(color);
      row_items.push(b_icon.into());
      row_items.push(b_label.into());
    }

    if show_battery {
      let icon_name = battery_icon(data.charging, data.percent);
      let icon_color = if data.charging {
        primary
      } else if let Some(p) = data.percent {
        if p <= 20 { danger } else { color }
      } else {
        color
      };

      let bat_icon = Icon::new(icon_name).size(icon_size).color(icon_color);
      let pct_str = data
        .percent
        .map(|p| format!("{p}%"))
        .unwrap_or_else(|| "AC".into());
      let bat_label = config.text(pct_str, &["component"]).color(faded);

      row_items.push(bat_icon.into());
      row_items.push(bat_label.into());
    }

    drop(data);

    let content = container(
      row(row_items)
        .spacing(config.sizing(&["component"]).gap)
        .align_y(Alignment::Center)
        .wrap(),
    );

    let menu_config = MenuConfig {
      content: "menus/power".into(),
      panel_name: None,
      position: ctx.position,
    };

    let menu_suppressed = ctx.menu_suppressed;

    let action =
      move |event: &Event, layout: iced::advanced::layout::Layout<'_>, cursor: mouse::Cursor| {
        if !cursor.is_over(layout.bounds()) {
          return None;
        }

        // TODO: Fix brightness scroll and slider
        match event {
          Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
            let y = match delta {
              mouse::ScrollDelta::Lines { y, .. } => *y,
              mouse::ScrollDelta::Pixels { y, .. } => *y / 20.0,
            };
            let step: i32 = if y > 0.0 { 5 } else { -5 };
            Some(ItemMessage::Action(ListenerAction::Payload {
              name: "power.step_brightness".to_ustr(),
              payload: Some(PayloadBox::new(step)),
            }))
          }
          Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if !menu_suppressed => {
            let point = cursor.position()?;
            Some(ItemMessage::Action(popup_open_action(&menu_config, point)))
          }
          _ => None,
        }
      };

    EventWrapper::new(spaced_component(config, ctx, content.into(), true), action).into()
  }
}
