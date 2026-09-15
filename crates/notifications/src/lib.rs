use std::{any::TypeId, cell::RefCell, collections::HashMap, sync::atomic::Ordering};

use iced::{
  Alignment, Color, Element, Length,
  widget::{Space, column, container, row, text, text_input},
};
use iced_layershell::reexport::{
  Anchor, IcedId, KeyboardInteractivity, Layer, NewLayerShellSettings,
};
use slowshell_commons::notifications::{
  NOTIF_CHANGED, NotificationCmd, NotificationItem, NotificationPayload, SharedNotificationState,
  reply_action_key,
};
use slowshell_config::{Config, ConfigParser, style::Style, style::Theme};
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  types::{PayloadBox, ToUstr},
};
use slowshell_desktop::{
  DesktopItem, EventFilter, ItemEffect, ItemMessage, MonitorScope, UpdateWhen, Visibility,
};
use slowshell_widgets::{Icon, clickable, notification_icon};

pub struct NotificationManager {
  max_visible: usize,
  position: Anchor,
  last_popups_count: usize,
  cache: RefCell<Cache>,
}

#[derive(Default)]
struct Cache {
  revision: u64,
  by_id: HashMap<u32, CachedMessages>,
}

#[derive(Clone)]
struct CachedMessages {
  dismiss: ItemMessage,
  invoke: HashMap<String, ItemMessage>,
}

fn build_cached(item: &NotificationItem, id: IcedId) -> CachedMessages {
  let notif_id = item.id;
  let dismiss = ItemMessage::EffectAction(
    id,
    ItemEffect::Redraw,
    ListenerAction::Payload {
      name: "notifications.dismiss".to_ustr(),
      payload: Some(PayloadBox::new(NotificationPayload {
        id: notif_id,
        ..Default::default()
      })),
    },
  );

  let reply_key = reply_action_key(item);
  let invoke: HashMap<String, ItemMessage> = item
    .actions
    .iter()
    .filter(|(key, _)| reply_key.as_deref() != Some(key.as_str()))
    .map(|(key, _)| {
      let msg = ItemMessage::EffectAction(
        id,
        ItemEffect::Redraw,
        ListenerAction::Payload {
          name: "notifications.invoke".to_ustr(),
          payload: Some(PayloadBox::new(NotificationPayload {
            id: notif_id,
            key: Some(key.clone()),
            ..Default::default()
          })),
        },
      );
      (key.clone(), msg)
    })
    .collect();

  CachedMessages { dismiss, invoke }
}

impl NotificationManager {
  pub fn new(position: Anchor) -> Self {
    Self {
      max_visible: 5,
      position,
      last_popups_count: 0,
      cache: RefCell::new(Cache::default()),
    }
  }
}

impl DesktopItem for NotificationManager {
  fn id(&self) -> &str {
    "notifications"
  }

  fn layer(&self, _: &Config, _: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
      layer: Layer::Overlay,
      anchor: self.position,
      exclusive_zone: Some(0),
      size: Some((360, 600)),
      margin: Some((12, 12, 12, 12)),
      keyboard_interactivity: KeyboardInteractivity::None,
      events_transparent: false,
      namespace: Some("slowshell-notifications".into()),
      ..Default::default()
    }
  }

  fn visibility(&self) -> Visibility {
    Visibility::Transient
  }

  fn update_strategy(&self) -> UpdateWhen {
    UpdateWhen::OnDemand
  }

  fn monitor(&self, _: &Config) -> MonitorScope {
    MonitorScope::Single(None)
  }

  fn init_events(&self) -> Vec<EventFilter> {
    vec![
      EventFilter::Named(NOTIF_CHANGED.into()),
      EventFilter::Payload("notifications.dismiss".into()),
      EventFilter::Payload("notifications.invoke".into()),
      EventFilter::Payload("notifications.reply".into()),
      EventFilter::Payload("notifications.set_reply".into()),
    ]
  }

  fn update(
    &mut self,
    _config: &Config,
    store: &mut Store,
    event: &ListenerAction,
  ) -> miette::Result<ItemEffect> {
    match event {
      ListenerAction::Signal { name, .. } if &**name == NOTIF_CHANGED => {
        if let Some(shared) = store.borrow::<SharedNotificationState>() {
          let popups_len = {
            let state = shared.state.lock().unwrap();
            state.popups.len()
          };

          let effect = if popups_len > 0 && self.last_popups_count == 0 {
            ItemEffect::Show
          } else if popups_len == 0 && self.last_popups_count > 0 {
            ItemEffect::Hide
          } else {
            ItemEffect::Redraw
          };

          self.last_popups_count = popups_len;
          Ok(effect)
        } else {
          Ok(ItemEffect::None)
        }
      }

      ListenerAction::Named(name) if &**name == NOTIF_CHANGED => {
        if let Some(shared) = store.borrow::<SharedNotificationState>() {
          let popups_len = {
            let state = shared.state.lock().unwrap();
            state.popups.len()
          };

          let effect = if popups_len > 0 && self.last_popups_count == 0 {
            ItemEffect::Show
          } else if popups_len == 0 && self.last_popups_count > 0 {
            ItemEffect::Hide
          } else {
            ItemEffect::Redraw
          };

          self.last_popups_count = popups_len;
          Ok(effect)
        } else {
          Ok(ItemEffect::None)
        }
      }

      ListenerAction::Payload { name, payload } if &**name == "notifications.dismiss" => {
        if let (Some(shared), Some(payload)) = (store.borrow::<SharedNotificationState>(), payload)
        {
          if let Some(notification) = payload.as_this::<NotificationPayload>() {
            let _ = shared
              .cmd_tx
              .send(NotificationCmd::DismissPopup(notification.id));
          }
        }
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, payload } if &**name == "notifications.invoke" => {
        if let (Some(shared), Some(payload)) = (store.borrow::<SharedNotificationState>(), payload)
        {
          if let Some((id, key)) = payload
            .as_this::<NotificationPayload>()
            .and_then(|p| Some((p.id, p.key.clone()?)))
          {
            let _ = shared.cmd_tx.send(NotificationCmd::InvokeAction(id, key));
          }
        }
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, payload } if &**name == "notifications.reply" => {
        if let (Some(shared), Some(payload)) = (store.borrow::<SharedNotificationState>(), payload)
        {
          if let Some((id, key, text)) = payload
            .as_this::<NotificationPayload>()
            .and_then(|p| Some((p.id, p.key.clone()?, p.text.clone()?)))
          {
            let _ = shared
              .cmd_tx
              .send(NotificationCmd::InvokeActionWithText(id, key, text));
          }
        }
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, payload } if &**name == "notifications.set_reply" => {
        if let (Some(shared), Some(payload)) = (store.borrow::<SharedNotificationState>(), payload)
        {
          if let Some((id, text)) = payload
            .as_this::<NotificationPayload>()
            .and_then(|p| Some((p.id, p.text.clone()?)))
          {
            shared.ui.lock().unwrap().replies.insert(id, text);
          }
        }
        Ok(ItemEffect::Redraw)
      }

      _ => Ok(ItemEffect::None),
    }
  }

  fn view<'a>(
    &'a self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    _monitor: &str,
  ) -> Element<'a, ItemMessage> {
    let style = config.style("notifications");
    let theme = &config.theme;

    let mut cache = self.cache.borrow_mut();

    let notif_elements: Vec<Element<'a, ItemMessage>> =
      if let Some(shared) = store.borrow::<SharedNotificationState>() {
        let state = shared.state.lock().unwrap();
        let ui = shared.ui.lock().unwrap();

        let revision = shared.revision.load(Ordering::Relaxed);
        if revision != cache.revision {
          cache.revision = revision;
          cache.by_id.clear();
        }

        state
          .popups
          .iter()
          .take(self.max_visible)
          .map(|n| {
            let cached = cache
              .by_id
              .entry(n.id)
              .or_insert_with(|| build_cached(n, id));
            let reply_text = if reply_action_key(n).is_some() {
              ui.replies.get(&n.id).cloned()
            } else {
              None
            };
            render_popup_item(
              n,
              reply_text.as_deref().unwrap_or(""),
              cached,
              &style,
              theme,
              id,
            )
          })
          .collect()
      } else {
        Vec::new()
      };

    container(column(notif_elements).spacing(style.number("list.spacing").unwrap_or(8.0)))
      .style(container::transparent)
      .into()
  }
}

fn render_popup_item<'a>(
  item: &NotificationItem,
  reply_text: &str,
  cached: &CachedMessages,
  style: &Style,
  theme: &Theme,
  id: IcedId,
) -> Element<'a, ItemMessage> {
  let app_icon: Element<'a, ItemMessage> = notification_icon(
    item.image.as_ref(),
    item.app_icon.as_deref(),
    style.number("icon.size").unwrap_or(18.0) as u16,
    style.color(theme, "app.color", theme.overlay),
  );

  let app_font = style.number("app.font.size").unwrap_or(10.0);
  let app: Element<'a, ItemMessage> = text(item.app_name.clone())
    .size(app_font)
    .color(style.color(theme, "app.color", theme.overlay))
    .into();

  let notif_id = item.id;
  let close_icon: Element<'a, ItemMessage> = Icon::new("window-close-symbolic")
    .size(12)
    .color(style.color(theme, "app.color", theme.overlay))
    .into();

  let dismiss_msg = cached.dismiss.clone();
  let close_btn = clickable(close_icon, move |_, _, _| Some(dismiss_msg.clone()));

  let mut header_items: Vec<Element<'a, ItemMessage>> = vec![
    app_icon,
    Space::new().width(6).into(),
    app,
    Space::new().width(Length::Fill).into(),
  ];
  if item.urgency >= 2 {
    let critical = style.color(theme, "urgency.color", Color::from_rgb(0.9, 0.3, 0.3));
    header_items.push(text("●").size(10).color(critical).into());
  }
  header_items.push(close_btn);
  let header_row = row(header_items).align_y(Alignment::Center);

  let summary = text(item.summary.clone()).size(style.number("summary.font.size").unwrap_or(13.0));
  let body = text(item.body.clone()).size(style.number("body.font.size").unwrap_or(11.0));

  let mut content_items: Vec<Element<'a, ItemMessage>> =
    vec![header_row.into(), summary.into(), body.into()];

  let reply_key = reply_action_key(item);

  if let Some(actions) = render_popup_actions(item, cached, reply_key.as_deref(), style, theme) {
    content_items.push(actions);
  }

  if let Some(reply_key) = reply_key {
    let reply_string = reply_text.to_string();
    let reply_key_submit = reply_key.clone();
    let input = text_input("Reply…", reply_text)
      .size(style.number("body.font.size").unwrap_or(11.0))
      .width(Length::Fill)
      .padding(6)
      .on_input(move |value| {
        ItemMessage::EffectAction(
          id,
          ItemEffect::Redraw,
          ListenerAction::Payload {
            name: "notifications.set_reply".to_ustr(),
            payload: Some(PayloadBox::new(NotificationPayload {
              id: notif_id,
              text: Some(value),
              ..Default::default()
            })),
          },
        )
      })
      .on_submit(ItemMessage::EffectAction(
        id,
        ItemEffect::Redraw,
        ListenerAction::Payload {
          name: "notifications.reply".to_ustr(),
          payload: Some(PayloadBox::new(NotificationPayload {
            id: notif_id,
            key: Some(reply_key_submit),
            text: Some(reply_string),
          })),
        },
      ));

    let submit_icon: Element<'a, ItemMessage> = Icon::new("mail-send-symbolic")
      .size(12)
      .color(style.color(theme, "app.color", theme.overlay))
      .into();

    let reply_msg = ItemMessage::EffectAction(
      id,
      ItemEffect::Redraw,
      ListenerAction::Payload {
        name: "notifications.reply".to_ustr(),
        payload: Some(PayloadBox::new(NotificationPayload {
          id: notif_id,
          key: Some(reply_key),
          text: Some(reply_text.to_string()),
        })),
      },
    );
    let submit_btn = clickable(submit_icon, move |_, _, _| Some(reply_msg.clone()));

    content_items.push(
      row![input, Space::new().width(6), submit_btn]
        .align_y(Alignment::Center)
        .into(),
    );
  }

  let inner_spacing = style.number("spacing").unwrap_or(4.0);
  let padding = style.number("padding").unwrap_or(10.0);
  let popup_style = style.container_style(theme);

  container(column(content_items).spacing(inner_spacing))
    .width(Length::Fill)
    .padding(padding)
    .style(move |_| popup_style)
    .into()
}

fn render_popup_actions<'a>(
  item: &NotificationItem,
  cached: &CachedMessages,
  reply_key: Option<&str>,
  style: &Style,
  theme: &Theme,
) -> Option<Element<'a, ItemMessage>> {
  let button_font = style.number("body.font.size").unwrap_or(11.0);
  let border_width = style.number("action.border.width").unwrap_or(0.0);
  let border_color = style.color(theme, "action.border.color", theme.overlay);
  let bg = style.color(theme, "action.background", theme.overlay);

  let actions = item
    .actions
    .iter()
    .filter(|(key, _)| Some(key.as_str()) != reply_key && cached.invoke.contains_key(key));

  let mut rows = Vec::new();
  let mut pending = None::<Element<'_, _>>;

  for (key, label) in actions {
    let invoke_msg = cached.invoke.get(key).unwrap().clone();
    let label = label.clone();

    let pill = container(text(label).size(button_font))
      .width(Length::Fill)
      .padding([7.0, 10.0])
      .style(move |_| container::Style {
        background: Some(bg.into()),
        border: iced::Border {
          radius: 8.0.into(),
          color: border_color,
          width: border_width,
        },
        ..Default::default()
      })
      .width(Length::Fill);

    let button = clickable(pill.into(), move |_, _, _| Some(invoke_msg.clone()));

    if let Some(first) = pending.take() {
      rows.push(row![first, button].spacing(6).width(Length::Fill).into());
    } else {
      pending = Some(button.into());
    }
  }

  if let Some(button) = pending {
    rows.push(row![button].width(Length::Fill).into());
  }

  if rows.is_empty() {
    None
  } else {
    Some(column(rows).spacing(6).width(Length::Fill).into())
  }
}

struct NotificationConfig {
  enabled: bool,
}

slowshell_registry::register_resources!(
  config: Unknown(Box::new(ConfigParser {
    type_id: TypeId::of::<NotificationConfig>(),
    de: |nodes| {
      let Some(node) = slowshell_config::find_node(nodes, "notifications") else {
        return Ok(None);
      };
      let enabled = slowshell_config::child_bool(node, "enabled").ok_or_else(|| {
        slowshell_config::ConfigError::at_node(
          node,
          "missing `enabled` in `notifications` block",
          Some("put `enabled true` or `enabled false` inside `notifications`"),
        )
      })?;
      Ok(Some(Box::new(NotificationConfig { enabled })))
    }
  })),
  app: Item(|config, _| {
    if let Some(notif_mgr) = config.typed::<NotificationConfig>().and_then(|conf| {
      if !conf.enabled {
        return None;
      }
      Some(Box::new(NotificationManager::new(Anchor::Top | Anchor::Right)) as Box<dyn DesktopItem + Send>)
    }) {
      return Ok(vec![notif_mgr]);
    }
    Ok(vec![])
  }),
  notifications: Style(
    slowshell_config::style! {
      "background" => "crust",
      "background.opacity" => 0.9,
      "radius" => 10,
      "border.width" => 1,
      "border.color" => "overlay",
      "border.color.opacity" => 0.15,
      "padding" => 12,
      "spacing" => 4,
      "list.spacing" => 8,
      "summary.font.size" => 13,
      "body.font.size" => 11,
      "app.font.size" => 10,
      "app.color" => "overlay",
      "icon.size" => 20,
      "action.border.width" => 0,
      "action.background" => "mantle"
    }
  )
);
