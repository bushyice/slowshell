use std::time::Duration;

use chrono::Local;
use iced::{
  Alignment, Element,
  widget::{Space, column, container, text},
};
use slowshell_commons::panels::PanelOrientation;
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::{FdHandle, ListenerAction},
  message::{EventFilter, ItemEffect, ItemMessage},
};

use crate::{Component, ComponentContext, ComponentOptions, MenuConfig, menu_trigger};

const TICK: &str = "component/clock.tick";

pub struct Clock {
  timer_fd: Option<i32>,
  time: String,
  parts: Vec<String>,
}

impl Clock {
  pub fn new() -> Self {
    let mut clock = Self {
      timer_fd: None,
      time: String::new(),
      parts: Vec::new(),
    };
    clock.update_time(None);
    clock
  }

  fn update_time(&mut self, options: Option<&ComponentOptions>) {
    let format = options.and_then(|x| x.str("format")).unwrap_or("%H:%M");
    let now = Local::now();

    self.time = now.format(format).to_string();
    self.parts = self
      .time
      .split(|c: char| c == ':' || c.is_whitespace())
      .filter(|part| !part.is_empty())
      .map(str::to_owned)
      .collect();
  }
}

impl Default for Clock {
  fn default() -> Self {
    Self::new()
  }
}

impl Component for Clock {
  fn events(&self) -> Vec<EventFilter> {
    vec![EventFilter::Named(TICK.into())]
  }

  fn watch(&mut self, store: &mut Store, options: Option<&ComponentOptions>) {
    if self.timer_fd.is_some() {
      return;
    }

    self.update_time(options);

    let Some(handle) = store.borrow::<FdHandle>() else {
      return;
    };

    if let Ok(fd) = handle.set_named_interval(
      Duration::from_secs(
        options
          .and_then(|o| o.int("update-time").map(|x| x as u64))
          .unwrap_or(60),
      ),
      TICK,
    ) {
      self.timer_fd = Some(fd);
    }
  }

  fn stop(&mut self, store: &Store, _options: Option<&ComponentOptions>) {
    if let (Some(fd), Some(handle)) = (self.timer_fd.take(), store.borrow::<FdHandle>()) {
      handle.stop_timer(fd);
    }
  }

  fn update(
    &mut self,
    _config: &Config,
    _store: &mut Store,
    event: &ListenerAction,
    options: Option<&ComponentOptions>,
  ) -> miette::Result<ItemEffect> {
    if matches!(
      event,
      ListenerAction::Named(n) if &**n == TICK
    ) || matches!(
      (event, self.timer_fd),
      (ListenerAction::Timer { name, fd }, Some(owned_fd)) if &**name == TICK && *fd == owned_fd
    ) {
      self.update_time(options);
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
    _options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let vertical = ctx.orientation == PanelOrientation::Vertical;
    let style = ctx.resolve_style(config);
    let theme = &config.theme;

    let content = if vertical {
      let font_size = style.number("font.size").unwrap_or(15.0);
      let color = style.color(theme, "color", theme.text);
      let mut items: Vec<Element<'a, ItemMessage>> = Vec::new();

      for (index, part) in self.parts.iter().enumerate() {
        if index > 0 {
          items.push(Space::new().height(2).into());
        }
        items.push(text(part.clone()).size(font_size).color(color).into());
      }

      if items.is_empty() {
        items.push(text(self.time.clone()).size(font_size).color(color).into());
      }

      container(column(items).align_x(Alignment::Center))
    } else {
      container(
        text(self.time.clone())
          .size(style.number("font.size").unwrap_or(13.0))
          .color(style.color(theme, "color", theme.text)),
      )
    };

    menu_trigger(
      content.into(),
      MenuConfig {
        content: "clock".into(),
        panel_name: None,
        position: ctx.position,
      },
      config,
      ctx,
      true,
    )
  }
}
