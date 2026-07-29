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
  types::{ToUstr, Ustr},
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
      EventFilter::Payload {
        name: "notification.new".into(),
        payload: None,
      },
      EventFilter::Payload {
        name: "notification.dismiss".into(),
        payload: None,
      },
      EventFilter::Tick,
    ]
  }

  fn update(&mut self, store: &Store, event: &ListenerAction) -> anyhow::Result<ItemEffect> {
    Ok(match event {
      ListenerAction::Payload { name, payload } if name.as_ref() == "notification.new" => {
        println!("notif {payload:?}");

        let Some(payload) = payload else {
          return Err(anyhow::anyhow!("Squanch this, mofo"));
        };

        let notif = Notification {
          id: self.next_id,
          app_name: payload
            .get(&Ustr::from("app"))
            .cloned()
            .ok_or(anyhow::anyhow!("App name not provided for notification"))?,
          summary: payload
            .get(&Ustr::from("summary"))
            .map(|x| x.to_string())
            .ok_or(anyhow::anyhow!("Summary not provided for notification"))?,
          body: payload
            .get(&Ustr::from("body"))
            .map(|x| x.to_string())
            .ok_or(anyhow::anyhow!("Body not provided for notification"))?,
          created_at: Instant::now(),
        };
        self.next_id += 1;
        if let Some(handle) = store.borrow::<FdHandle>() {
          handle.set_timer(
            Duration::from_secs(5),
            ListenerAction::Payload {
              name: "notification.dismiss".into(),
              payload: Some([("id".to_ustr(), notif.id.to_string().to_ustr())].into()),
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
        if !self.notifications.is_empty()
          && let Some(id) = payload.as_ref().and_then(|x| {
            x.get::<Ustr>(&"id".into())
              .and_then(|x| x.parse::<u32>().ok())
          })
        {
          self.notifications.remove(&id);
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
