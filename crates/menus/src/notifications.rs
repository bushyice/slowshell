use std::time::Instant;

use iced::{
  Alignment, Color, Element, Length,
  widget::{Space, column, container, row, scrollable, text_input},
};
use iced_exwlshell::reexport::IcedId;
use slowshell_commons::notifications::{
  NOTIF_CHANGED, NotificationItem, NotificationPayload, SharedNotificationState, reply_action_key,
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{EventFilter, ItemEffect, ItemMessage},
  types::{PayloadBox, ToUstr},
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{
  Backdrop, Icon, Renderable, SizedPopup, clickable, hover_row, notification_icon,
};

pub struct NotificationsRenderable;

#[derive(Debug, Clone)]
enum NotifAction {
  ToggleDnd,
  ClearAll,
  Dismiss(u32),
  Invoke(u32, String),
  Reply(u32, String, String),
  SetReply(u32, String),
}

impl NotifAction {
  fn message(self, id: IcedId) -> ItemMessage {
    let (name, payload) = match self {
      NotifAction::ToggleDnd => ("notifications.toggle_dnd", None),
      NotifAction::ClearAll => ("notifications.clear_all", None),
      NotifAction::Dismiss(notif_id) => (
        "notifications.dismiss",
        Some(PayloadBox::new(NotificationPayload {
          id: notif_id,
          ..Default::default()
        })),
      ),
      NotifAction::Invoke(notif_id, key) => (
        "notifications.invoke",
        Some(PayloadBox::new(NotificationPayload {
          id: notif_id,
          key: Some(key),
          ..Default::default()
        })),
      ),
      NotifAction::Reply(notif_id, key, text) => (
        "notifications.reply",
        Some(PayloadBox::new(NotificationPayload {
          id: notif_id,
          key: Some(key),
          text: Some(text),
        })),
      ),
      NotifAction::SetReply(notif_id, text) => (
        "notifications.set_reply",
        Some(PayloadBox::new(NotificationPayload {
          id: notif_id,
          text: Some(text),
          ..Default::default()
        })),
      ),
    };

    ItemMessage::EffectAction(
      id,
      ItemEffect::Redraw,
      ListenerAction::Payload {
        name: name.to_ustr(),
        payload,
      },
    )
  }
}

impl Renderable for NotificationsRenderable {
  fn events(&self, _store: &Store) -> Vec<EventFilter> {
    vec![EventFilter::Named(NOTIF_CHANGED.into())]
  }

  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    data: &'a dyn std::any::Any,
  ) -> Element<'a, ItemMessage> {
    let data = data.downcast_ref::<PopupSettings>().unwrap();

    let mut children: Vec<Element<'a, ItemMessage>> = Vec::new();

    if let Some(shared) = store.borrow::<SharedNotificationState>() {
      let is_dnd = shared.dnd.load(std::sync::atomic::Ordering::Relaxed);
      let (state, replies) = {
        let mut state = shared.state.lock().unwrap();
        std::sync::Arc::make_mut(&mut state).unread_count = 0;
        let replies = shared.ui.lock().unwrap().clone();
        (state.clone(), replies)
      };
      let history = &state.history;

      children.push(render_header(
        config,
        id,
        is_dnd,
        history.is_empty(),
        history.len(),
      ));

      if history.is_empty() {
        children.push(
          container(config.text("No notifications", &["menu", "text-name", "text-faded"]))
            .padding(16)
            .align_x(Alignment::Center)
            .width(Length::Fill)
            .into(),
        );
      } else {
        let mut notif_items = Vec::new();
        for item in history.iter() {
          let reply = replies.replies.get(&item.id).cloned().unwrap_or_default();
          notif_items.push(render_notification_item(config, item, reply, id));
        }

        children.push(
          scrollable(column(notif_items).spacing(config.sizing(&["menu", "list"]).gap))
            .height(Length::Fixed(400.))
            .into(),
        );
      }
    } else {
      children.push(
        config
          .text(
            "Notifications service unavailable",
            &["menu", "text-status", "text-faded"],
          )
          .into(),
      );
    }

    let list_style = config.computed(&["menu", "list"]);
    let list_spacing = config.sizing(&["menu", "list"]).gap;
    let list_width = list_style.length("list-width").unwrap_or(220.0);
    let list = column(children)
      .spacing(list_spacing)
      .width(Length::Fixed(list_width));

    let inner_padding = config.sizing(&["menu", "inner"]).padding();
    let popup_style = config.container_cached(&["menu"]);

    SizedPopup::new(container(
      container(list).padding(inner_padding).style(popup_style),
    ))
    .with_width(120.)
    .with_backdrop(Backdrop::transparent().with_close_on_click(true))
    .with_position(data.position.0.get(store), data.position.1.get(store))
    .with_panel_edge(data.panel_edge)
    .with_panel_insets(store, data.monitor.as_deref())
    .with_on_close(ItemMessage::Effect(id, ItemEffect::Hide))
    .into()
  }
}

fn render_header<'a>(
  config: &'a Config,
  id: IcedId,
  is_dnd: bool,
  is_empty: bool,
  count: usize,
) -> Element<'a, ItemMessage> {
  let title = config.text("Notifications", &["menu", "text-header"]);

  let count_badge = if count == 0 {
    return row![
      title,
      Space::new().width(Length::Fill),
      dnd_and_clear_buttons(config, id, is_dnd, is_empty)
    ]
    .align_y(Alignment::Center)
    .into();
  } else {
    let badge =
      container(config.text(format!("{count}"), &["menu", "text-status", "text-primary"]))
        .padding(config.sizing(&["menu", "badge"]).padding())
        .style(config.container_cached(&["menu", "badge"]));

    row![title, badge].spacing(8).align_y(Alignment::Center)
  };

  row![
    count_badge,
    Space::new().width(Length::Fill),
    dnd_and_clear_buttons(config, id, is_dnd, is_empty)
  ]
  .align_y(Alignment::Center)
  .into()
}

fn dnd_and_clear_buttons<'a>(
  config: &'a Config,
  id: IcedId,
  is_dnd: bool,
  is_empty: bool,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();

  let dnd_icon_name = if is_dnd {
    "notifications-disabled-symbolic"
  } else {
    "notification-symbolic"
  };

  let dnd_color = if is_dnd { theme.primary } else { theme.subtext };

  let dnd_icon: Element<'a, ItemMessage> =
    Icon::new(dnd_icon_name).size(14).color(dnd_color).into();

  let dnd_btn = clickable(dnd_icon, move |_, _, _| {
    Some(NotifAction::ToggleDnd.message(id))
  });

  let clear_color = if is_empty { theme.subtext } else { theme.text };

  let clear_icon: Element<'a, ItemMessage> = Icon::new("edit-clear-all-symbolic")
    .size(14)
    .color(clear_color)
    .into();

  let clear_btn = clickable(clear_icon, move |_, _, _| {
    Some(NotifAction::ClearAll.message(id))
  });

  row![dnd_btn, Space::new().width(8), clear_btn,]
    .align_y(Alignment::Center)
    .into()
}

fn relative_time(created: Instant) -> String {
  let secs = created.elapsed().as_secs();
  if secs < 60 {
    "just now".to_string()
  } else if secs < 3600 {
    format!("{}m ago", secs / 60)
  } else if secs < 86400 {
    format!("{}h ago", secs / 3600)
  } else {
    format!("{}d ago", secs / 86400)
  }
}

fn render_notification_item<'a>(
  config: &'a Config,
  item: &NotificationItem,
  reply: String,
  id: IcedId,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();

  let icon_size = config
    .computed(&["menu", "icon"])
    .length("icon-size")
    .unwrap_or(16.0) as u16;

  let app_icon: Element<'a, ItemMessage> = notification_icon(
    item.image.as_ref(),
    item.app_icon.as_deref(),
    icon_size,
    theme.subtext,
  );

  let mut top_row_items: Vec<Element<'a, ItemMessage>> = vec![
    app_icon,
    Space::new().width(6).into(),
    config
      .text(
        item.app_name.clone(),
        &["menu", "text-status", "text-faded"],
      )
      .into(),
    Space::new().width(Length::Fill).into(),
  ];
  if item.urgency >= 2 {
    top_row_items.push(
      config
        .text("●", &["menu", "text-status"])
        .color(Color::from_rgb(0.9, 0.3, 0.3))
        .into(),
    );
  }
  top_row_items.push(
    config
      .text(
        relative_time(item.created_at),
        &["menu", "text-status", "text-faded"],
      )
      .into(),
  );
  top_row_items.push(Space::new().width(8).into());
  let top_row = row(top_row_items).align_y(Alignment::Center);

  let notif_id = item.id;
  let close_icon: Element<'a, ItemMessage> = Icon::new("window-close-symbolic")
    .size(12)
    .color(theme.subtext)
    .into();

  let close_btn = clickable(close_icon, move |_, _, _| {
    Some(NotifAction::Dismiss(notif_id).message(id))
  });

  let summary = config.text(item.summary.clone(), &["menu", "text-name"]);

  let body = config.text(item.body.clone(), &["menu", "text-status", "text-faded"]);

  let mut content_items: Vec<Element<'a, ItemMessage>> = vec![
    row![top_row, close_btn].align_y(Alignment::Center).into(),
    summary.into(),
    body.into(),
  ];

  let reply_key = reply_action_key(item);

  if let Some(actions) = render_action_buttons(config, item, reply_key.as_deref(), id) {
    content_items.push(actions);
  }

  if let Some(reply_key) = reply_key {
    let input_size = config.sizing(&["menu", "text-status"]).font_size;
    let input = text_input("Reply…", &reply)
      .size(input_size)
      .width(Length::Fill)
      .padding(6)
      .on_input(move |value| NotifAction::SetReply(notif_id, value).message(id))
      .on_submit(NotifAction::Reply(notif_id, reply_key.clone(), reply.clone()).message(id));

    let submit_icon: Element<'a, ItemMessage> = Icon::new("mail-send-symbolic")
      .size(12)
      .color(theme.subtext)
      .into();
    let submit_btn = clickable(submit_icon, move |_, _, _| {
      Some(NotifAction::Reply(notif_id, reply_key.clone(), reply.clone()).message(id))
    });

    content_items.push(
      row![input, Space::new().width(6), submit_btn]
        .align_y(Alignment::Center)
        .into(),
    );
  }

  hover_row(
    config,
    &["menu", "row"],
    container(column(content_items).spacing(6))
      .padding(config.sizing(&["menu", "row"]).padding())
      .width(Length::Fill),
  )
}

fn render_action_buttons<'a>(
  config: &'a Config,
  item: &NotificationItem,
  reply_key: Option<&str>,
  id: IcedId,
) -> Option<Element<'a, ItemMessage>> {
  let mut buttons: Vec<Element<'a, ItemMessage>> = Vec::new();
  let from_plugin = item.plugin.is_some();

  for (key, label) in &item.actions {
    if Some(key.as_str()) == reply_key {
      continue;
    }

    let notif_id = item.id;
    let key = key.clone();
    let label = label.clone();

    let pill = hover_row(
      config,
      &["menu", "chip"],
      container(config.text(label, &["menu", "text-status"]))
        .padding(config.sizing(&["menu", "chip"]).padding()),
    );

    buttons.push(
      clickable(pill, move |_, _, _| {
        Some(if from_plugin {
          ItemMessage::EffectAction(
            id,
            ItemEffect::Redraw,
            ListenerAction::Payload {
              name: key.clone().to_ustr(),
              payload: None,
            },
          )
        } else {
          NotifAction::Invoke(notif_id, key.clone()).message(id)
        })
      })
      .into(),
    );
  }

  if buttons.is_empty() {
    None
  } else {
    Some(row(buttons).spacing(6).into())
  }
}
