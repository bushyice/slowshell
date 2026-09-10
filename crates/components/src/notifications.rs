use std::{
  os::fd::AsRawFd,
  sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
  },
};

use iced::{
  Element, Event, mouse,
  widget::{container, row, text},
};
use nix::{
  fcntl::{FcntlArg, OFlag, fcntl},
  sys::epoll::EpollFlags,
};
use slowshell_commons::notifications::{
  NOTIF_CHANGED, NotificationCmd, NotificationPayload, NotificationState, SharedNotificationState,
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::{FdHandle, ListenerAction},
  message::{EventFilter, ItemEffect, ItemMessage},
  types::OptionalPayloadBox,
};
use slowshell_services::util::drain_signal_fd;
use slowshell_widgets::{EventWrapper, Icon};

use crate::{
  Component, ComponentContext, ComponentOptions, MenuConfig, popup_open_action, spaced_component,
};

pub struct Notifications {
  signal_fd: Option<i32>,
  started: bool,
  shared: SharedNotificationState,
  last_revision: u64,
  shared_in_store: bool,
  cmd_rx: Option<tokio::sync::mpsc::UnboundedReceiver<NotificationCmd>>,
}

impl Notifications {
  pub fn new() -> Self {
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    Self {
      signal_fd: None,
      started: false,
      shared: Arc::new(NotificationState {
        dnd: AtomicBool::new(false),
        state: Mutex::new(Default::default()),
        ui: Mutex::new(Default::default()),
        revision: AtomicU64::new(0),
        cmd_tx,
      }),
      last_revision: 0,
      shared_in_store: false,
      cmd_rx: Some(cmd_rx),
    }
  }
}

impl Default for Notifications {
  fn default() -> Self {
    Self::new()
  }
}

impl Component for Notifications {
  fn events(&self) -> Vec<EventFilter> {
    vec![
      EventFilter::Named(NOTIF_CHANGED.into()),
      EventFilter::Payload("notifications.toggle_dnd".into()),
      EventFilter::Payload("notifications.clear_all".into()),
      EventFilter::Payload("notifications.dismiss".into()),
      EventFilter::Payload("notifications.invoke".into()),
      EventFilter::Payload("notifications.reply".into()),
      EventFilter::Payload("notifications.set_reply".into()),
    ]
  }

  fn watch(&mut self, store: &mut Store, _options: Option<&ComponentOptions>) {
    store.insert(self.shared.clone());
    self.shared_in_store = true;

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
          name: NOTIF_CHANGED.into(),
          fd: raw,
        },
      );
      self.signal_fd = Some(raw);

      Some(write_fd)
    });

    let shared = self.shared.clone();
    let cmd_rx = self.cmd_rx.take();
    if let Some(cmd_rx) = cmd_rx {
      slowshell_services::notifications::run(shared, cmd_rx, notify);
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
  ) -> miette::Result<ItemEffect> {
    if !self.shared_in_store {
      self.shared_in_store = true;
      store.insert(self.shared.clone());
    }

    match event {
      ListenerAction::Signal { name, fd } if &**name == NOTIF_CHANGED => {
        drain_signal_fd(*fd);
        let rev = self.shared.revision.load(Ordering::SeqCst);
        if rev != self.last_revision {
          self.last_revision = rev;
          Ok(ItemEffect::Redraw)
        } else {
          Ok(ItemEffect::None)
        }
      }

      ListenerAction::Named(name) if &**name == NOTIF_CHANGED => {
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

      ListenerAction::Payload { name, .. } if &**name == "notifications.toggle_dnd" => {
        let _ = self.shared.cmd_tx.send(NotificationCmd::ToggleDnd);
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, .. } if &**name == "notifications.clear_all" => {
        let _ = self.shared.cmd_tx.send(NotificationCmd::ClearAll);
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, payload } if &**name == "notifications.dismiss" => {
        if let Some(payload) = payload.transform::<NotificationPayload>() {
          let _ = self
            .shared
            .cmd_tx
            .send(NotificationCmd::DismissHistory(payload.id));
          let mut ui = self.shared.ui.lock().unwrap();
          ui.replies.remove(&payload.id);
        }
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, payload } if &**name == "notifications.invoke" => {
        if let Some((id, key)) = payload
          .transform::<NotificationPayload>()
          .and_then(|p| Some((p.id, p.key.clone()?)))
        {
          let _ = self
            .shared
            .cmd_tx
            .send(NotificationCmd::InvokeAction(id, key));
          let mut ui = self.shared.ui.lock().unwrap();
          ui.replies.remove(&id);
        }
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, payload } if &**name == "notifications.reply" => {
        if let Some((id, key, text)) = payload
          .transform::<NotificationPayload>()
          .and_then(|p| Some((p.id, p.key.clone()?, p.text.clone()?)))
        {
          let _ = self
            .shared
            .cmd_tx
            .send(NotificationCmd::InvokeActionWithText(id, key, text));
          let mut ui = self.shared.ui.lock().unwrap();
          ui.replies.remove(&id);
        }
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, payload } if &**name == "notifications.set_reply" => {
        if let Some((id, text)) = payload
          .transform::<NotificationPayload>()
          .and_then(|p| Some((p.id, p.text.clone()?)))
        {
          self.shared.ui.lock().unwrap().replies.insert(id, text);
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
    _options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let style = config.style("component");
    let theme = &config.theme;

    let icon_size = style.number("icon.size").unwrap_or(14.0) as u16;
    let font_size = style.number("font.size").unwrap_or(11.0);

    let dnd = self.shared.dnd.load(Ordering::Relaxed);
    let unread = {
      let state = self.shared.state.lock().unwrap();
      state.unread_count
    };

    let icon_name = if dnd {
      "notifications-disabled-symbolic"
    } else {
      "notification-symbolic"
    };

    let icon_color = if dnd {
      style.color(theme, "color.faded", theme.subtext)
    } else if unread > 0 {
      style.color(theme, "color.primary", theme.primary)
    } else {
      style.color(theme, "color", theme.text)
    };

    let icon: Element<'a, ItemMessage> = Icon::new(icon_name)
      .size(icon_size)
      .color(icon_color)
      .into();

    let mut row_items = vec![icon];

    if unread > 0 && !dnd {
      row_items.push(
        text(format!("{unread}"))
          .size(font_size)
          .color(icon_color)
          .into(),
      );
    }

    let content = container(row(row_items).spacing(4).align_y(iced::Alignment::Center));

    let menu_config = MenuConfig {
      content: "menus/notifications".into(),
      panel_name: None,
      position: ctx.position,
    };

    let action =
      move |event: &Event, layout: iced::advanced::layout::Layout<'_>, cursor: mouse::Cursor| {
        if !cursor.is_over(layout.bounds()) {
          return None;
        }

        match event {
          Event::Mouse(mouse::Event::ButtonPressed(
            mouse::Button::Middle | mouse::Button::Right,
          )) => Some(ItemMessage::Action(ListenerAction::Payload {
            name: "notifications.toggle_dnd".into(),
            payload: None,
          })),
          Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
            let point = cursor.position()?;
            Some(ItemMessage::Action(popup_open_action(&menu_config, point)))
          }
          _ => None,
        }
      };

    EventWrapper::new(spaced_component(config, ctx, content.into()), action).into()
  }
}
