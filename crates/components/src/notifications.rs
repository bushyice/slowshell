use std::sync::atomic::Ordering;

use iced::{
  Element, Event, mouse,
  widget::{container, row},
};
use slowshell_commons::notifications::{
  NOTIF_CHANGED, NotificationCmd, NotificationPayload, SharedNotificationState,
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{EventFilter, ItemEffect, ItemMessage},
  types::{OptionalPayloadBox, ToUstr},
};
use slowshell_services::util::drain_signal_fd;
use slowshell_widgets::{EventWrapper, Icon};

use crate::{
  Component, ComponentContext, ComponentOptions, MenuConfig, popup_open_action, spaced_component,
};

pub struct Notifications {
  shared: SharedNotificationState,
  last_revision: u64,
  shared_in_store: bool,
}

impl Notifications {
  pub fn new() -> Self {
    Self {
      shared: slowshell_services::notifications::shared(),
      last_revision: 0,
      shared_in_store: false,
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
    self.shared = slowshell_services::notifications::ensure(store);
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
          std::sync::Arc::make_mut(&mut ui)
            .replies
            .remove(&payload.id);
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
          std::sync::Arc::make_mut(&mut ui).replies.remove(&id);
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
          std::sync::Arc::make_mut(&mut ui).replies.remove(&id);
        }
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, payload } if &**name == "notifications.set_reply" => {
        if let Some((id, text)) = payload
          .transform::<NotificationPayload>()
          .and_then(|p| Some((p.id, p.text.clone()?)))
        {
          let mut ui = self.shared.ui.lock().unwrap();
          std::sync::Arc::make_mut(&mut ui).replies.insert(id, text);
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
      faded
    } else if unread > 0 {
      primary
    } else {
      text_color
    };

    let icon: Element<'a, ItemMessage> = Icon::new(icon_name)
      .size(icon_size)
      .color(icon_color)
      .into();

    let mut row_items = vec![icon];

    if unread > 0 && !dnd {
      row_items.push(
        config
          .text(format!("{unread}"), &["component"])
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

    let menu_suppressed = ctx.menu_suppressed;
    let monitor = ctx.monitor.to_ustr();

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
          Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if !menu_suppressed => {
            let point = cursor.position()?;
            Some(ItemMessage::Action(popup_open_action(
              &menu_config,
              point,
              &monitor,
            )))
          }
          _ => None,
        }
      };

    EventWrapper::new(spaced_component(config, ctx, content.into(), true), action).into()
  }
}
