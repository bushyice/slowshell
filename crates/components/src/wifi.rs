use std::fs;

use iced::{
  Color, Element,
  widget::{container, text},
};
use slowshell_core::{Store, message::ItemMessage};

use crate::{Component, ComponentContext, MenuConfig, menu_trigger};

pub struct Wifi {
  connected: bool,
}

impl Wifi {
  pub fn new() -> Self {
    let connected = fs::read_dir("/sys/class/net")
      .map(|entries| {
        entries.filter_map(|e| e.ok()).any(|entry| {
          let name = entry.file_name().to_string_lossy().into_owned();
          (name.starts_with("wlan") || name.starts_with("wlp"))
            && fs::read_to_string(entry.path().join("operstate"))
              .map(|s| s.trim() == "up")
              .unwrap_or(false)
        })
      })
      .unwrap_or(false);
    Self { connected }
  }
}

impl Default for Wifi {
  fn default() -> Self {
    Self::new()
  }
}

impl Component for Wifi {
  fn view<'a>(&self, _store: &Store, ctx: &ComponentContext) -> Element<'a, ItemMessage> {
    let icon = if self.connected { "󰤨" } else { "󰤭" };
    let color = if self.connected {
      Color::WHITE
    } else {
      Color::from_rgba(1.0, 1.0, 1.0, 0.4)
    };
    let content = container(text(icon).size(15).color(color)).padding([4, 8]);
    menu_trigger(
      content.into(),
      MenuConfig {
        content: "wifi".into(),
        panel_name: None,
        position: ctx.position,
      },
    )
  }
}
