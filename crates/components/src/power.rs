use std::{
  os::fd::AsRawFd,
  sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
  },
};

use iced::{
  Alignment, Element, Event, mouse,
  widget::{container, row, text},
};
use nix::{
  fcntl::{FcntlArg, OFlag, fcntl},
  sys::epoll::EpollFlags,
};
use slowshell_commons::power::{
  POWER_CHANGED, PowerCmd, PowerState, SharedPowerState, battery_icon, brightness_icon,
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::{FdHandle, ListenerAction},
  message::{EventFilter, ItemEffect, ItemMessage},
  types::{OptionalPayloadBox, PayloadBox, ToUstr, Ustr},
};
use slowshell_services::util::drain_signal_fd;
use slowshell_widgets::{EventWrapper, Icon};

use crate::{Component, ComponentContext, ComponentOptions, MenuConfig, popup_open_action};

pub struct Power {
  signal_fd: Option<i32>,
  started: bool,
  shared: SharedPowerState,
  last_revision: u64,
  shared_in_store: bool,
  cmd_rx: Option<tokio::sync::mpsc::UnboundedReceiver<PowerCmd>>,
}

impl Power {
  pub fn new() -> Self {
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    Self {
      signal_fd: None,
      started: false,
      shared: Arc::new(PowerState {
        data: Mutex::new(Default::default()),
        revision: AtomicU64::new(0),
        cmd_tx,
      }),
      last_revision: 0,
      shared_in_store: false,
      cmd_rx: Some(cmd_rx),
    }
  }

  pub fn with_shared(shared: SharedPowerState) -> Self {
    Self {
      signal_fd: None,
      started: false,
      shared,
      last_revision: 0,
      shared_in_store: false,
      cmd_rx: None,
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
    if let Some(existing) = store.borrow::<SharedPowerState>() {
      self.shared = existing.clone();
      self.shared_in_store = true;
    } else {
      store.insert(self.shared.clone());
      self.shared_in_store = true;
    }

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
          name: POWER_CHANGED.into(),
          fd: raw,
        },
      );
      self.signal_fd = Some(raw);

      Some(write_fd)
    });

    let shared = self.shared.clone();
    let cmd_rx = self.cmd_rx.take();
    let interval = _options.and_then(|o| o.int("interval")).unwrap_or(10) as u64;
    if let Some(cmd_rx) = cmd_rx {
      slowshell_services::power::run(shared, cmd_rx, notify, interval);
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
    &self,
    config: &Config,
    _store: &Store,
    ctx: &ComponentContext,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let style = config.style("component");
    let theme = &config.theme;

    let icon_size = style.number("icon.size").unwrap_or(14.0) as u16;
    let font_size = style.number("font.size").unwrap_or(13.0);
    let color = style.color(theme, "color", theme.text);
    let faded = style.color(theme, "color.faded", theme.subtext);
    let sec_color = style.color(theme, "color.secondary", theme.yellow);

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
      let b_label = text(format!("{}%", data.brightness_percent))
        .size(font_size)
        .color(color);
      row_items.push(b_icon.into());
      row_items.push(b_label.into());
    }

    if show_battery {
      let icon_name = battery_icon(data.charging, data.percent);
      let icon_color = if data.charging {
        style.color(theme, "color.primary", theme.primary)
      } else if let Some(p) = data.percent {
        if p <= 20 {
          style.color(theme, "color.danger", theme.red)
        } else {
          color
        }
      } else {
        color
      };

      let bat_icon = Icon::new(icon_name).size(icon_size).color(icon_color);
      let pct_str = data
        .percent
        .map(|p| format!("{p}%"))
        .unwrap_or_else(|| "AC".into());
      let bat_label = text(pct_str).size(font_size).color(faded);

      row_items.push(bat_icon.into());
      row_items.push(bat_label.into());
    }

    drop(data);

    let content = container(
      row(row_items)
        .spacing(style.number("spacing").unwrap_or(4.0))
        .align_y(Alignment::Center)
        .wrap(),
    );

    let menu_config = MenuConfig {
      content: "menus/power".into(),
      panel_name: None,
      position: ctx.position,
    };

    let action =
      move |event: &Event, layout: iced::advanced::layout::Layout<'_>, cursor: mouse::Cursor| {
        if !cursor.is_over(layout.bounds()) {
          return None;
        }

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
          Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
            let point = cursor.position()?;
            Some(ItemMessage::Action(popup_open_action(&menu_config, point)))
          }
          _ => None,
        }
      };

    EventWrapper::new(content.into(), action).into()
  }
}
