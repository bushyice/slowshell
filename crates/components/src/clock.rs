use std::time::Duration;

use chrono::Local;
use iced::{
  Color, Element,
  widget::{container, text},
};
use slowshell_core::{
  Store,
  listeners::{FdHandle, ListenerAction},
  message::{EventFilter, ItemEffect, ItemMessage},
};

use crate::{Component, ComponentContext, MenuConfig, menu_trigger};

const TICK: &str = "component/clock.tick";

pub struct Clock {
  timer_fd: Option<i32>,
  time: String,
}

impl Clock {
  pub fn new() -> Self {
    Self {
      timer_fd: None,
      time: Local::now().format("%H:%M").to_string(),
    }
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

  fn watch(&mut self, store: &Store) {
    println!("Watching clock");
    if self.timer_fd.is_some() {
      return;
    }
    let Some(handle) = store.borrow::<FdHandle>() else {
      println!("Handle not found");
      return;
    };
    if let Ok(fd) = handle.set_interval(Duration::from_secs(1), ListenerAction::Named(TICK.into()))
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
      self.time = Local::now().format("%H:%M").to_string();
      Ok(ItemEffect::Redraw)
    } else {
      Ok(ItemEffect::None)
    }
  }

  fn view<'a>(&self, _store: &Store, ctx: &ComponentContext) -> Element<'a, ItemMessage> {
    let time = self.time.clone();
    let content = container(text(time).size(13).color(Color::WHITE)).padding([4, 8]);
    menu_trigger(
      content.into(),
      MenuConfig {
        content: "clock".into(),
        panel_name: None,
        position: ctx.position,
      },
    )
  }
}
