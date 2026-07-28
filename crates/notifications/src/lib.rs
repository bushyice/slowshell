use std::{
  collections::HashMap,
  time::{Duration, Instant},
};

use iced::{
  Color, Element, Length,
  widget::{column, container, text},
};
use iced_layershell::reexport::{
  Anchor, IcedId, KeyboardInteractivity, Layer, NewLayerShellSettings,
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::{FdHandle, ListenerAction},
  types::Ustr,
};
use slowshell_desktop::{
  DesktopItem, EventFilter, ItemEffect, ItemMessage, MonitorScope, UpdateWhen, Visibility,
};

#[derive(Debug, Clone)]
pub struct Notification {
  pub id: u32,
  pub app_name: Ustr,
  pub summary: String,
  pub body: String,
  pub created_at: Instant,
  pub timeout: Option<Duration>,
}

pub struct NotificationManager {
  notifications: HashMap<u32, Notification>,
  max_visible: usize,
  next_id: u32,
  position: Anchor,
}

impl NotificationManager {
  pub fn new(position: Anchor) -> Self {
    Self {
      notifications: HashMap::new(),
      max_visible: 5,
      next_id: 0,
      position,
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
      size: Some((400, 800)),
      margin: Some((8, 8, 8, 8)),
      keyboard_interactivity: KeyboardInteractivity::None,
      events_transparent: true,
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

  // fn clone_box(&self) -> Box<dyn DesktopItem> {
  //   Box::new(NotificationManager {
  //     notifications: HashMap::new(),
  //     max_visible: self.max_visible,
  //     next_id: 0,
  //     position: self.position,
  //   })
  // }

  fn init_events(&self) -> Vec<EventFilter> {
    vec![
      EventFilter::Named("notification.new".into()),
      EventFilter::Named("notification.dismiss".into()),
      EventFilter::Payload {
        name: "notification.dismiss".into(),
        payload: 0,
      },
      EventFilter::Tick,
    ]
  }

  fn update(&mut self, store: &Store, event: &ListenerAction) -> anyhow::Result<ItemEffect> {
    Ok(match event {
      ListenerAction::Named(name) if name.as_ref() == "notification.new" => {
        let notif = Notification {
          id: self.next_id,
          app_name: "Some title".into(),
          summary: "A notif".into(),
          body: "New notification.".into(),
          created_at: Instant::now(),
          timeout: Some(Duration::from_secs(5)),
        };
        self.next_id += 1;
        if let Some(handle) = store.borrow::<FdHandle>() {
          handle.set_timer(
            Duration::from_secs(5),
            ListenerAction::Payload {
              name: "notification.dismiss".into(),
              payload: notif.id as isize,
            },
          )?;
        }
        self.notifications.insert(notif.id, notif);

        if self.notifications.len() == 1 {
          ItemEffect::Show
        } else {
          ItemEffect::Redraw
        }
      }
      ListenerAction::Payload { name, payload } if name.as_ref() == "notification.dismiss" => {
        if !self.notifications.is_empty() {
          self.notifications.remove(&(*payload as u32));
          if self.notifications.is_empty() {
            ItemEffect::Hide
          } else {
            ItemEffect::Redraw
          }
        } else {
          ItemEffect::None
        }
      }
      _ => ItemEffect::None,
    })
  }

  fn view(&self, _store: &Store, _id: IcedId) -> Element<'_, ItemMessage> {
    let notifs = self
      .notifications
      .iter()
      .take(self.max_visible)
      .map(|(_, n)| {
        let header = text(&n.summary).size(14);
        let body = text(&n.body).size(12);
        let app = text(n.app_name.to_string())
          .size(10)
          .color(Color::from_rgb(0.5, 0.5, 0.5));

        container(column![app, header, body,].spacing(4))
          .width(Length::Fill)
          .padding(12)
          .style(container::rounded_box)
          .into()
      })
      .collect::<Vec<Element<'_, ItemMessage>>>();

    container(column(notifs).spacing(8))
      .style(container::transparent)
      .into()
  }

  // fn tick(&mut self) {
  //   let now = Instant::now();
  //   self.notifications.retain(|n| {
  //     n.timeout
  //       .map_or(true, |t| now.duration_since(n.created_at) < t)
  //   });
  // }
}
