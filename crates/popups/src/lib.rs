use std::collections::HashMap;

use iced::widget::space;
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings};
pub use slowshell_commons::{
  panels::PanelEdge,
  popups::{PopupPosition, PopupSettings},
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  types::{PayloadBox, PayloadBuilder, ToUstr},
};
use slowshell_desktop::{
  DesktopItem, EventFilter, ItemEffect, ItemMessage, UpdateWhen, Visibility,
};
use slowshell_widgets::Renderables;

#[derive(Default)]
pub struct Popup {
  current: Option<PopupSettings>,
}

impl Popup {}

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
      EventFilter::Payload("popup.open".into()),
      EventFilter::Payload("popup.close".into()),
      EventFilter::Payload("network.password".into()),
      EventFilter::Payload("popup.redraw".into()),
      EventFilter::Named("component/network.changed".into()),
    ]
  }

  fn update(
    &mut self,
    _config: &Config,
    store: &mut Store,
    event: &ListenerAction,
  ) -> anyhow::Result<ItemEffect> {
    match event {
      ListenerAction::Payload { name, payload } if name.as_ref() == "popup.open" => {
        let Some(payload) = payload else {
          return Ok(ItemEffect::None);
        };

        let _ = payload.enforce::<PopupSettings>();

        let settings = payload.as_this::<PopupSettings>();

        if settings.is_some()
          && store
            .borrow::<Renderables>()
            .is_some_and(|x| x.contains_key(&settings.unwrap().content))
        {
          self.current = settings.cloned();

          Ok(ItemEffect::Show)
        } else {
          Ok(ItemEffect::None)
        }
      }
      ListenerAction::Payload { name, payload } if name.as_ref() == "popup.close" => {
        self.current = None;
        Ok(ItemEffect::Destroy)
      }
      ListenerAction::Signal { name, .. }
        if name.as_ref() == "component/network.changed" && self.current.is_some() =>
      {
        Ok(ItemEffect::Redraw)
      }
      ListenerAction::Payload { name, .. } if name.as_ref() == "popup.redraw" => {
        if self.current.is_some() {
          Ok(ItemEffect::Redraw)
        } else {
          Ok(ItemEffect::None)
        }
      }
      ListenerAction::Payload { name, payload } if name.as_ref() == "network.password" => {
        if self.current.is_some() {
          Ok(ItemEffect::Redraw)
        } else {
          let _ = payload;
          Ok(ItemEffect::None)
        }
      }
      _ => Ok(ItemEffect::None),
    }
  }

  fn handle_message(&mut self, store: Option<&mut Store>, message: &ItemMessage) -> ItemEffect {
    match message {
      ItemMessage::Effect(_, effect) => match effect {
        ItemEffect::Hide | ItemEffect::Destroy => {
          self.current = None;
          ItemEffect::Hide
        }
        ItemEffect::Custom([40, a, b, c]) if let Some(current) = &self.current => {
          if let Some(effect) = store
            .and_then(|s| s.borrow_mut::<Renderables>())
            .and_then(|x| x.get_mut(&current.content))
            .and_then(|r| r.handle_message(message))
          {
            return effect;
          }
          ItemEffect::None
        }
        other => other.clone(),
      },
      _ => ItemEffect::None,
    }
  }

  fn view<'a>(
    &'a self,
    config: &'a Config,
    store: &'a slowshell_core::Store,
    id: iced::window::Id,
    _monitor: &str,
  ) -> iced::Element<'a, slowshell_desktop::ItemMessage> {
    if let Some(current) = &self.current {
      if let Some(renderable) = store
        .borrow::<Renderables>()
        .and_then(|x| x.get(&current.content))
      {
        return renderable.view(config, store, id, current).into();
      }
    }
    space().into()
  }
}

fn parse_pos(pos: &str) -> Option<PopupPosition> {
  match pos.split_once(",")? {
    ("fixed", num) => Some(PopupPosition::Fixed(num.parse().ok()?)),
    ("cursor", num) => Some(PopupPosition::FollowMouse(num.parse().ok()?)),
    ("panel", name) => Some(PopupPosition::Panel { name: name.into() }),
    _ => None,
  }
}

fn settings_from_payload(payload: &HashMap<&str, &str>) -> Option<PopupSettings> {
  let default_pos = "fixed,0.0";
  let x = parse_pos(payload.get("x").unwrap_or(&default_pos))?;
  let y = parse_pos(payload.get("y").unwrap_or(&default_pos))?;

  let panel_edge = match payload.get("edge").map(|e| &**e) {
    Some("left") => Some(PanelEdge::Left { width: 0 }),
    Some("right") => Some(PanelEdge::Right { width: 0 }),
    _ => None,
  };

  Some(PopupSettings {
    position: (x, y),
    content: payload.get("content").map(|s| (*s).to_ustr())?,
    panel_edge,
  })
}

slowshell_registry::register_resources!(
  payload: Unknown(PayloadBuilder {
    commands: &["popup.open"],
    build: |_, args| {
      settings_from_payload(&args.as_map()).map(PayloadBox::new)
    }
  }.into_boxed())
);
