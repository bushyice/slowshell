use std::collections::HashMap;

use iced::widget::space;
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings};
use slowshell_commons::panels::PanelPositions;
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  types::{ToUstr, Ustr},
};
use slowshell_desktop::{
  DesktopItem, EventFilter, ItemEffect, ItemMessage, UpdateWhen, Visibility,
};
use slowshell_widgets::Renderables;

#[derive(Debug, Clone)]
pub enum PopupPosition {
  Fixed(f32),

  Percentage(f32),

  FollowMouse(f32),

  Panel { name: Ustr },
}

impl PopupPosition {
  pub fn get(&self, store: &Store) -> f32 {
    match self {
      PopupPosition::Fixed(f) => *f,
      PopupPosition::Percentage(_) => 0.,
      PopupPosition::FollowMouse(f) => *f,
      PopupPosition::Panel { name } => {
        match store.borrow::<PanelPositions>().and_then(|p| p.get(name)) {
          Some(pos) => pos.get() as f32,
          None => 0.,
        }
      }
    }
  }
}

pub struct PopupSettings {
  pub position: (PopupPosition, PopupPosition),
  content: Ustr,
}

#[derive(Default)]
pub struct Popup {
  current: Option<PopupSettings>,
}

impl Popup {
  fn parse_pos(pos: &Ustr) -> Option<PopupPosition> {
    match pos.split_once(",")? {
      ("fixed", num) => Some(PopupPosition::Fixed(num.parse().ok()?)),
      ("cursor", num) => Some(PopupPosition::FollowMouse(num.parse().ok()?)),
      ("panel", name) => Some(PopupPosition::Panel { name: name.into() }),
      _ => None,
    }
  }

  fn settings_from_payload(payload: &HashMap<Ustr, Ustr>) -> Option<PopupSettings> {
    let x = Self::parse_pos(payload.get("x").unwrap_or(&"fixed,0.0".to_ustr()))?;
    let y = Self::parse_pos(payload.get("y").unwrap_or(&"fixed,0.0".to_ustr()))?;

    Some(PopupSettings {
      position: (x, y),
      content: payload.get("content").cloned()?,
    })
  }
}

impl DesktopItem for Popup {
  fn id(&self) -> &str {
    "popup"
  }

  fn layer(&self, _config: &Config, _monitor: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
      layer: Layer::Overlay,
      anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
      exclusive_zone: Some(-1),
      size: None,
      margin: Some((0, 0, 0, 0)),
      keyboard_interactivity: KeyboardInteractivity::OnDemand,
      events_transparent: false,
      namespace: Some(format!("slowshell-popup",)),
      ..Default::default()
    }
  }

  fn visibility(&self) -> Visibility {
    Visibility::Transient
  }

  fn update_strategy(&self) -> UpdateWhen {
    UpdateWhen::OnDemand
  }

  fn init_events(&self) -> Vec<EventFilter> {
    vec![
      EventFilter::Payload {
        name: "popup.open".into(),
        payload: None,
      },
      EventFilter::Payload {
        name: "popup.close".into(),
        payload: None,
      },
    ]
  }

  fn update(&mut self, _store: &mut Store, event: &ListenerAction) -> anyhow::Result<ItemEffect> {
    match event {
      ListenerAction::Payload { name, payload } if name.as_ref() == "popup.open" => {
        let Some(payload) = payload else {
          return Ok(ItemEffect::None);
        };

        let settings = Self::settings_from_payload(payload);

        if settings.is_none() {
          return Ok(ItemEffect::None);
        }

        self.current = settings;

        Ok(ItemEffect::Show)
      }
      ListenerAction::Payload { name, payload } if name.as_ref() == "popup.close" => {
        self.current = None;
        Ok(ItemEffect::Destroy)
      }
      _ => Ok(ItemEffect::None),
    }
  }

  fn handle_message(&mut self, message: &ItemMessage) -> ItemEffect {
    match message {
      ItemMessage::Effect(_, effect) => match effect {
        ItemEffect::Hide | ItemEffect::Destroy => {
          self.current = None;
          ItemEffect::Hide
        }
        other => other.clone(),
      },
      _ => ItemEffect::None,
    }
  }

  fn view(
    &self,
    store: &slowshell_core::Store,
    id: iced::window::Id,
  ) -> iced::Element<'_, slowshell_desktop::ItemMessage> {
    if let Some(current) = &self.current {
      if let Some(renderable) = store
        .borrow::<Renderables>()
        .and_then(|x| x.get(&current.content))
      {
        return renderable.view(store, id, current).into();
      }
    }
    space().into()
  }
}
