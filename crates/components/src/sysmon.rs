use std::{sync::Arc, time::Duration};

use iced::{
  Element, Length,
  widget::{column, container, progress_bar, row, text},
};
use slowshell_commons::panels::PanelOrientation;
use slowshell_commons::system::{SharedSystemState, SystemSnapshot, SystemState};
use slowshell_config::{Config, style::Theme};
use slowshell_core::{
  Store,
  listeners::{FdHandle, ListenerAction},
  message::{EventFilter, ItemEffect, ItemMessage},
};
use slowshell_widgets::Icon;

use crate::{
  Component, ComponentContext, ComponentOptions, MenuConfig, menu_trigger, vertical_text,
};

const TICK: &str = "component/system-mon.tick";

#[derive(Debug, Clone, Copy)]
enum SystemItem {
  Cpu,
  Mem,
  Temp,
  Net,
  Load,
}

impl SystemItem {
  fn parse(value: &str) -> Option<Self> {
    match value.trim() {
      "cpu" => Some(Self::Cpu),
      "mem" => Some(Self::Mem),
      "temp" => Some(Self::Temp),
      "net" => Some(Self::Net),
      "load" => Some(Self::Load),
      _ => None,
    }
  }
}

pub struct SystemMon {
  timer_fd: Option<i32>,
  shared: SharedSystemState,
}

impl SystemMon {
  pub fn new() -> Self {
    Self {
      timer_fd: None,
      shared: Arc::new(SystemState::default()),
    }
  }
}

impl Default for SystemMon {
  fn default() -> Self {
    Self::new()
  }
}

impl Component for SystemMon {
  fn events(&self) -> Vec<EventFilter> {
    vec![EventFilter::Named(TICK.into())]
  }

  fn watch(&mut self, store: &mut Store, options: Option<&ComponentOptions>) {
    store.insert(self.shared.clone());

    if self.timer_fd.is_some() {
      return;
    }

    let Some(handle) = store.borrow::<FdHandle>() else {
      return;
    };

    let update_time = options.and_then(|o| o.int("interval")).unwrap_or(2).max(1) as u64;

    if let Ok(fd) = handle.set_named_interval(Duration::from_secs(update_time), TICK) {
      self.timer_fd = Some(fd);
    }

    let shared = self.shared.clone();
    slowshell_services::system::run(shared, Duration::from_secs(update_time));
  }

  fn stop(&mut self, store: &Store, _options: Option<&ComponentOptions>) {
    if let (Some(fd), Some(handle)) = (self.timer_fd.take(), store.borrow::<FdHandle>()) {
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
    if store.borrow::<SharedSystemState>().is_none() {
      store.insert(self.shared.clone());
    }

    if matches!(
      event,
      ListenerAction::Named(n) if &**n == TICK
    ) || matches!(
      (event, self.timer_fd),
      (ListenerAction::Timer { name, fd }, Some(owned_fd)) if &**name == TICK && *fd == owned_fd
    ) {
      Ok(ItemEffect::Redraw)
    } else {
      Ok(ItemEffect::None)
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

    let icon_names: [Option<&str>; 3] = options
      .and_then(|o| o.strings("icon_names"))
      .map(|names| {
        [
          names.first().copied(),
          names.get(1).copied(),
          names.get(2).copied(),
        ]
      })
      .unwrap_or([None, None, None]);

    let icons = options.and_then(|o| o.bool("icons")).unwrap_or(true);

    let labels = options.and_then(|o| o.bool("labels")).unwrap_or(true);

    let progress = options.and_then(|o| o.bool("progress")).unwrap_or(false);

    let progress_direction = options
      .and_then(|o| o.str("progress-direction"))
      .unwrap_or("horizontal");

    let icon_size = style.number("icon.size").unwrap_or(16.0) as u16;
    let font_size = style.number("font.size").unwrap_or(12.0);
    let color = style.color(theme, "color", theme.text);

    let items = Self::items(options);
    let snapshot = self.shared.snapshot.lock().unwrap().clone();

    let contents = items.into_iter().map(|item| {
      self.view_item(
        item,
        &snapshot,
        icon_names,
        icons,
        labels,
        progress,
        progress_direction,
        font_size,
        icon_size,
        color,
        &theme,
        ctx.orientation != PanelOrientation::Horizontal,
      )
    });

    let content: Element<'_, ItemMessage> = if ctx.orientation == PanelOrientation::Horizontal {
      row(contents).spacing(8).into()
    } else {
      column(contents).spacing(8).into()
    };

    let content = container(content);

    menu_trigger(
      content.into(),
      MenuConfig {
        content: "menus/system".into(),
        panel_name: None,
        position: ctx.position,
      },
      config,
      ctx,
      true,
    )
  }
}

impl SystemMon {
  fn items(options: Option<&ComponentOptions>) -> Vec<SystemItem> {
    options
      .and_then(|o| o.strings("items"))
      .map(|items| items.into_iter().filter_map(SystemItem::parse).collect())
      .unwrap_or_else(|| vec![SystemItem::Cpu, SystemItem::Mem])
  }

  fn view_item<'a>(
    &self,
    item: SystemItem,
    snapshot: &SystemSnapshot,
    icon_names: [Option<&str>; 3],
    icons: bool,
    labels: bool,
    progress: bool,
    progress_direction: &str,
    font_size: f32,
    icon_size: u16,
    color: iced::Color,
    theme: &Theme,
    vertical: bool,
  ) -> Element<'a, ItemMessage> {
    let (icon, label, value, c) = match item {
      SystemItem::Cpu => (
        [
          if let Some(icon) = icon_names[0] {
            icon
          } else {
            "cpu-symbolic"
          },
          "applications-system-symbolic",
        ],
        "CPU",
        format!("{:.0}%", snapshot.cpu_usage),
        match snapshot.cpu_usage {
          70.0..100. => Some(theme.red),
          50.0..70. => Some(theme.yellow),
          _ => None,
        },
      ),

      SystemItem::Mem => (
        [
          if let Some(icon) = icon_names[1] {
            icon
          } else {
            "memory-symbolic"
          },
          "applications-system-symbolic",
        ],
        "MEM",
        format!("{:.0}%", snapshot.mem_usage),
        match snapshot.mem_usage {
          70.0..100. => Some(theme.red),
          50.0..70. => Some(theme.yellow),
          _ => None,
        },
      ),

      SystemItem::Temp => (
        ["temperature-symbolic", "applications-system-symbolic"],
        "TEMP",
        snapshot
          .temperature
          .map(|temp| format!("{temp:.0}°C"))
          .unwrap_or_else(|| "--".into()),
        match snapshot.mem_usage {
          60.0..70. => Some(theme.yellow),
          _ if snapshot.mem_usage > 70. => Some(theme.red),
          _ => None,
        },
      ),

      SystemItem::Net => (
        [
          if snapshot.network_state == 2 {
            "network-wired"
          } else if snapshot.network_state == 1 {
            "network-wireless"
          } else {
            "network-offline"
          },
          "applications-system-symbolic",
        ],
        "NET",
        format!(
          "↓ {} ↑ {}",
          format_bytes(snapshot.network_rx),
          format_bytes(snapshot.network_tx),
        ),
        None,
      ),

      SystemItem::Load => (
        [
          if let Some(icon) = icon_names[2] {
            icon
          } else {
            "drive-harddisk-symbolic"
          },
          "applications-system-symbolic",
        ],
        "LOAD",
        format!(
          "{:.2} {:.2} {:.2}",
          snapshot.load[0], snapshot.load[1], snapshot.load[2],
        ),
        None,
      ),
    };

    let color = c.unwrap_or(color);

    let value_num = match item {
      SystemItem::Cpu => snapshot.cpu_usage,
      SystemItem::Mem => snapshot.mem_usage,
      SystemItem::Temp => snapshot.temperature.unwrap_or(0.0),
      SystemItem::Load => (snapshot.load[0] * 100.0).min(100.0),
      SystemItem::Net => 0.0,
    };

    let pct_item = matches!(item, SystemItem::Cpu | SystemItem::Mem | SystemItem::Temp);

    let (glyph_label, inline_value): (Option<String>, Option<String>) = if vertical && pct_item {
      if labels {
        (Some(label.to_string()), Some(value.clone()))
      } else if !icons {
        (None, Some(value.clone()))
      } else {
        (None, None)
      }
    } else if labels {
      (Some(format!("{label} {value}")), None)
    } else if !icons {
      (Some(value.clone()), None)
    } else {
      (None, None)
    };

    let mut col: Vec<Element<'_, ItemMessage>> = vec![];

    if progress {
      let progress_color = c.unwrap_or(theme.blue);
      let background = theme.crust;

      match progress_direction {
        "vertical" => col.push(
          progress_bar(0.0..=100.0, value_num)
            .vertical()
            .length(Length::Fill)
            .girth(Length::Fixed(4.0))
            .style(move |t| iced::widget::progress_bar::Style {
              background: background.into(),
              bar: progress_color.into(),
              ..iced::widget::progress_bar::primary(t)
            })
            .into(),
        ),

        _ => col.push(
          progress_bar(0.0..=100.0, value_num)
            .length(Length::Fixed(16.0))
            .girth(Length::Fixed(4.0))
            .style(move |t| iced::widget::progress_bar::Style {
              background: background.into(),
              bar: progress_color.into(),
              ..iced::widget::progress_bar::primary(t)
            })
            .into(),
        ),
      }
    }

    if let Some(label) = glyph_label {
      if vertical {
        col.insert(0, vertical_text(&label, font_size, color));
      } else {
        col.insert(0, text(label).size(font_size).color(color).into());
      }
    }

    if let Some(value) = inline_value {
      col.insert(0, text(value).size(font_size).color(color).into());
    }

    if icons {
      col.insert(
        0,
        Icon::any(icon).size(icon_size).color(color).into_element(),
      );
    }

    if vertical {
      column(col)
        .spacing(2)
        .align_x(iced::Alignment::Center)
        .into()
    } else {
      row(col).spacing(4).into()
    }
  }
}

fn format_bytes(bytes: u64) -> String {
  const UNITS: [&str; 4] = ["B", "K", "M", "G"];

  let mut value = bytes as f64;
  let mut unit = 0;

  while value >= 1024.0 && unit < UNITS.len() - 1 {
    value /= 1024.0;
    unit += 1;
  }

  if unit == 0 {
    format!("{value:.0}{}", UNITS[unit])
  } else {
    format!("{value:.1}{}", UNITS[unit])
  }
}
