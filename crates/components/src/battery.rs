use std::{fs, time::Duration};

use iced::{
  Color, Element,
  widget::{container, row, text},
};
use slowshell_core::{
  Store,
  listeners::{FdHandle, ListenerAction},
  message::{EventFilter, ItemEffect, ItemMessage},
};

use crate::{Component, ComponentContext, MenuConfig, menu_trigger};

const TICK: &str = "component/battery.tick";

pub struct Battery {
  timer_fd: Option<i32>,
  percent: Option<u8>,
  charging: bool,
}

impl Battery {
  pub fn new() -> Self {
    let mut battery = Self {
      timer_fd: None,
      percent: None,
      charging: false,
    };
    battery.refresh();
    battery
  }

  fn battery_base() -> Option<String> {
    fs::read_dir("/sys/class/power_supply")
      .ok()?
      .find_map(|entry| {
        let name = entry.ok()?.file_name().to_string_lossy().into_owned();
        if name.starts_with("BAT") {
          Some(name)
        } else {
          None
        }
      })
  }

  fn refresh(&mut self) {
    let Some(base) = Self::battery_base() else {
      self.percent = None;
      self.charging = false;
      return;
    };
    let dir = format!("/sys/class/power_supply/{base}");
    self.percent = fs::read_to_string(format!("{dir}/capacity"))
      .ok()
      .and_then(|s| s.trim().parse().ok());
    self.charging = fs::read_to_string(format!("{dir}/status"))
      .ok()
      .map(|s| s.trim() == "Charging")
      .unwrap_or(false);
  }

  fn icon(&self) -> &'static str {
    match (self.charging, self.percent) {
      (true, _) => "󰂄",
      (_, Some(p)) => match p {
        80..=100 => "󰁹",
        60..=79 => "󰂂",
        40..=59 => "󰁿",
        20..=39 => "󰁽",
        _ => "󰁻",
      },
      (_, None) => "󰁻",
    }
  }
}

impl Default for Battery {
  fn default() -> Self {
    Self::new()
  }
}

impl Component for Battery {
  fn events(&self) -> Vec<EventFilter> {
    vec![EventFilter::Named(TICK.into())]
  }

  fn watch(&mut self, store: &Store) {
    if self.timer_fd.is_some() {
      return;
    }
    let Some(handle) = store.borrow::<FdHandle>() else {
      return;
    };
    if let Ok(fd) = handle.set_interval(Duration::from_secs(5), ListenerAction::Named(TICK.into()))
    {
      self.timer_fd = Some(fd);
    }
  }

  fn stop(&mut self, store: &Store) {
    if let (Some(fd), Some(handle)) = (self.timer_fd.take(), store.borrow::<FdHandle>()) {
      handle.stop_timer(fd);
    }
  }

  fn update(&mut self, _store: &mut Store, event: &ListenerAction) -> anyhow::Result<ItemEffect> {
    if matches!(event, ListenerAction::Named(n) if &**n == TICK) {
      self.refresh();
      Ok(ItemEffect::Redraw)
    } else {
      Ok(ItemEffect::None)
    }
  }

  fn view<'a>(&self, _store: &Store, ctx: &ComponentContext) -> Element<'a, ItemMessage> {
    let icon = text(self.icon()).size(15).color(Color::WHITE);
    let percent = self
      .percent
      .map(|p| format!("{p}%"))
      .unwrap_or_else(|| "N/A".into());
    let label = text(percent)
      .size(12)
      .color(Color::from_rgba(1.0, 1.0, 1.0, 0.8));

    let content = container(row![icon, label].spacing(4)).padding([4, 8]);
    menu_trigger(
      content.into(),
      MenuConfig {
        content: "battery".into(),
        panel_name: None,
        position: ctx.position,
      },
    )
  }
}
