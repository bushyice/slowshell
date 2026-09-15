use std::time::Instant;

use iced::{
  Alignment, Color, Element, Length,
  widget::{Space, column, container, row, scrollable, text, text_input},
};
use iced_layershell::reexport::IcedId;
use slowshell_commons::notifications::{
  NotificationItem, NotificationPayload, SharedNotificationState, reply_action_key,
};
use slowshell_config::{Config, style::Style, style::Theme};
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{ItemEffect, ItemMessage},
  types::{PayloadBox, ToUstr},
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{Backdrop, Icon, Renderable, SizedPopup, clickable, notification_icon};

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
  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    data: &'a dyn std::any::Any,
  ) -> Element<'a, ItemMessage> {
    let data = data.downcast_ref::<PopupSettings>().unwrap();
    let style = config.style("menu");
    let theme = &config.theme;

    let mut children: Vec<Element<'a, ItemMessage>> = Vec::new();

    if let Some(shared) = store.borrow::<SharedNotificationState>() {
      let is_dnd = shared.dnd.load(std::sync::atomic::Ordering::Relaxed);
      let (history, replies) = {
        let mut state = shared.state.lock().unwrap();
        state.unread_count = 0;
        let history = state.history.clone();
        let replies = shared.ui.lock().unwrap().replies.clone();
        (history, replies)
      };

      children.push(render_header(
        &style,
        theme,
        id,
        is_dnd,
        history.is_empty(),
        history.len(),
      ));

      if history.is_empty() {
        children.push(
          container(
            text("No notifications")
              .size(style.number("name.font.size").unwrap_or(13.0))
              .color(style.color(theme, "color.faded", theme.subtext)),
          )
          .padding(16)
          .align_x(Alignment::Center)
          .width(Length::Fill)
          .into(),
        );
      } else {
        let mut notif_items = Vec::new();
        for item in history {
          let reply = replies.get(&item.id).cloned().unwrap_or_default();
          notif_items.push(render_notification_item(&item, reply, &style, theme, id));
        }

        children.push(
          scrollable(column(notif_items).spacing(style.number("list.spacing").unwrap_or(6.0)))
            .height(Length::Fixed(400.))
            .into(),
        );
      }
    } else {
      children.push(
        text("Notifications service unavailable")
          .size(style.number("status.font.size").unwrap_or(11.0))
          .color(style.color(theme, "color.faded", theme.subtext))
          .into(),
      );
    }

    let list_spacing = style.number("list.spacing").unwrap_or(6.0);
    let list_width = style.number("list.width").unwrap_or(280.0);
    let list = column(children)
      .spacing(list_spacing)
      .width(Length::Fixed(list_width));

    let inner_padding = style.number("inner.padding").unwrap_or(6.0);
    let mut popup_style = style.container_style(theme);
    if let (Some(x), Some(y)) = (
      style.number("shadow.offset.x"),
      style.number("shadow.offset.y"),
    ) {
      popup_style.shadow.offset = iced::Vector::new(x, y);
    }

    SizedPopup::new(container(
      container(list)
        .padding(inner_padding)
        .style(move |_| popup_style),
    ))
    .with_width(120.)
    .with_backdrop(Backdrop::transparent().with_close_on_click(true))
    .with_position(data.position.0.get(store), data.position.1.get(store))
    .with_panel_edge(data.panel_edge)
    .with_panel_insets(store)
    .with_on_close(ItemMessage::Effect(id, ItemEffect::Hide))
    .into()
  }
}

fn render_header<'a>(
  style: &Style,
  theme: &Theme,
  id: IcedId,
  is_dnd: bool,
  is_empty: bool,
  count: usize,
) -> Element<'a, ItemMessage> {
  let title = text("Notifications")
    .size(style.number("header.font.size").unwrap_or(17.0))
    .color(style.color(theme, "color", theme.text));

  let count_badge = if count == 0 {
    return row![
      title,
      Space::new().width(Length::Fill),
      dnd_and_clear_buttons(style, theme, id, is_dnd, is_empty)
    ]
    .align_y(Alignment::Center)
    .into();
  } else {
    let bg_col = style.color(theme, "row.background", theme.mantle);
    let badge = container(
      text(format!("{count}"))
        .size(style.number("status.font.size").unwrap_or(11.0))
        .color(style.color(theme, "color.primary", theme.primary)),
    )
    .padding([2.0, 8.0])
    .style(move |_| container::Style {
      background: Some(bg_col.into()),
      border: iced::Border {
        radius: 8.0.into(),
        ..Default::default()
      },
      ..Default::default()
    });

    row![title, badge].spacing(8).align_y(Alignment::Center)
  };

  row![
    count_badge,
    Space::new().width(Length::Fill),
    dnd_and_clear_buttons(style, theme, id, is_dnd, is_empty)
  ]
  .align_y(Alignment::Center)
  .into()
}

fn dnd_and_clear_buttons<'a>(
  style: &Style,
  theme: &Theme,
  id: IcedId,
  is_dnd: bool,
  is_empty: bool,
) -> Element<'a, ItemMessage> {
  let dnd_icon_name = if is_dnd {
    "notifications-disabled-symbolic"
  } else {
    "notification-symbolic"
  };

  let dnd_color = if is_dnd {
    style.color(theme, "color.primary", theme.primary)
  } else {
    style.color(theme, "color.faded", theme.subtext)
  };

  let dnd_icon: Element<'a, ItemMessage> =
    Icon::new(dnd_icon_name).size(14).color(dnd_color).into();

  let dnd_btn = clickable(dnd_icon, move |_, _, _| {
    Some(NotifAction::ToggleDnd.message(id))
  });

  let clear_color = if is_empty {
    style.color(theme, "color.faded", theme.subtext)
  } else {
    style.color(theme, "color", theme.text)
  };

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
  item: &NotificationItem,
  reply: String,
  style: &Style,
  theme: &Theme,
  id: IcedId,
) -> Element<'a, ItemMessage> {
  let app_icon: Element<'a, ItemMessage> = notification_icon(
    item.image.as_ref(),
    item.app_icon.as_deref(),
    style.number("icon.size").unwrap_or(13.0) as u16,
    style.color(theme, "color.faded", theme.subtext),
  );

  let app_font = style.number("app.font.size").unwrap_or(11.0);
  let mut top_row_items: Vec<Element<'a, ItemMessage>> = vec![
    app_icon,
    Space::new().width(6).into(),
    text(item.app_name.clone())
      .size(app_font)
      .color(style.color(theme, "color.faded", theme.subtext))
      .into(),
    Space::new().width(Length::Fill).into(),
  ];
  if item.urgency >= 2 {
    let critical = style.color(theme, "urgency.color", Color::from_rgb(0.9, 0.3, 0.3));
    top_row_items.push(text("●").size(10).color(critical).into());
  }
  top_row_items.push(
    text(relative_time(item.created_at))
      .size(style.number("app.font.size").unwrap_or(10.0))
      .color(style.color(theme, "color.faded", theme.subtext))
      .into(),
  );
  top_row_items.push(Space::new().width(8).into());
  let top_row = row(top_row_items).align_y(Alignment::Center);

  let notif_id = item.id;
  let close_icon: Element<'a, ItemMessage> = Icon::new("window-close-symbolic")
    .size(12)
    .color(style.color(theme, "color.faded", theme.subtext))
    .into();

  let close_btn = clickable(close_icon, move |_, _, _| {
    Some(NotifAction::Dismiss(notif_id).message(id))
  });

  let summary = text(item.summary.clone())
    .size(style.number("name.font.size").unwrap_or(14.0))
    .color(style.color(theme, "color", theme.text));

  let body = text(item.body.clone())
    .size(style.number("status.font.size").unwrap_or(12.0))
    .color(style.color(theme, "color.faded", theme.subtext));

  let mut content_items: Vec<Element<'a, ItemMessage>> = vec![
    row![top_row, close_btn].align_y(Alignment::Center).into(),
    summary.into(),
    body.into(),
  ];

  let reply_key = reply_action_key(item);

  if let Some(actions) = render_action_buttons(item, reply_key.as_deref(), style, theme, id) {
    content_items.push(actions);
  }

  if let Some(reply_key) = reply_key {
    let input = text_input("Reply…", &reply)
      .size(style.number("status.font.size").unwrap_or(12.0))
      .width(Length::Fill)
      .padding(6)
      .on_input(move |value| NotifAction::SetReply(notif_id, value).message(id))
      .on_submit(NotifAction::Reply(notif_id, reply_key.clone(), reply.clone()).message(id));

    let submit_icon: Element<'a, ItemMessage> = Icon::new("mail-send-symbolic")
      .size(12)
      .color(style.color(theme, "color.faded", theme.subtext))
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

  let bg = style.color(theme, "row.background", Color::TRANSPARENT);
  let row_radius = style.number("row.radius").unwrap_or(10.0);

  container(column(content_items).spacing(6))
    .padding(style.padding([10.0, 12.0]))
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(bg.into()),
      border: iced::Border {
        radius: row_radius.into(),
        ..Default::default()
      },
      ..Default::default()
    })
    .into()
}

fn render_action_buttons<'a>(
  item: &NotificationItem,
  reply_key: Option<&str>,
  style: &Style,
  theme: &Theme,
  id: IcedId,
) -> Option<Element<'a, ItemMessage>> {
  let button_font = style.number("status.font.size").unwrap_or(11.0);
  let mut buttons: Vec<Element<'a, ItemMessage>> = Vec::new();
  let from_plugin = item.plugin.is_some();

  for (key, label) in &item.actions {
    if Some(key.as_str()) == reply_key {
      continue;
    }

    let notif_id = item.id;
    let key = key.clone();
    let label = label.clone();
    let bg = style.color(theme, "row.background", theme.mantle);

    let pill = container(
      text(label)
        .size(button_font)
        .color(style.color(theme, "color", theme.text)),
    )
    .padding([4.0, 10.0])
    .style(move |_| container::Style {
      background: Some(bg.into()),
      border: iced::Border {
        radius: 10.0.into(),
        ..Default::default()
      },
      ..Default::default()
    });

    buttons.push(
      clickable(pill.into(), move |_, _, _| {
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
