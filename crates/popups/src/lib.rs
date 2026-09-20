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

impl Popup {
  fn base_events() -> Vec<EventFilter> {
    vec![
      EventFilter::Payload("popup.open".into()),
      EventFilter::Payload("popup.close".into()),
    ]
  }

  fn events_for(&self, store: &Store) -> Vec<EventFilter> {
    let mut events = Self::base_events();

    if let Some(current) = &self.current
      && let Some(renderable) = store
        .borrow::<Renderables>()
        .and_then(|renderables| renderables.get(&current.content))
    {
      events.extend(renderable.events());
    }

    events
  }

  fn renderable<'a>(&self, store: &'a Store) -> Option<&'a dyn slowshell_widgets::Renderable> {
    let current = self.current.as_ref()?;
    store
      .borrow::<Renderables>()
      .and_then(|renderables| renderables.get(&current.content))
      .map(|renderable| renderable.as_ref())
  }

  fn with_renderable<R>(
    &self,
    store: &mut Store,
    f: impl FnOnce(&mut Box<dyn slowshell_widgets::Renderable>, &mut Store) -> R,
  ) -> Option<R> {
    let content = self.current.as_ref()?.content.clone();

    let renderable = store
      .borrow_mut::<Renderables>()
      .and_then(|renderables| renderables.remove(&content))?;

    let mut renderable = renderable;
    let result = f(&mut renderable, store);

    if let Some(renderables) = store.borrow_mut::<Renderables>() {
      renderables.insert(content, renderable);
    }

    Some(result)
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
    Self::base_events()
  }

  fn update(
    &mut self,
    config: &Config,
    store: &mut Store,
    event: &ListenerAction,
  ) -> miette::Result<ItemEffect> {
    match event {
      ListenerAction::Payload { name, payload } if name.as_ref() == "popup.open" => {
        let Some(payload) = payload else {
          return Ok(ItemEffect::None);
        };

        let _ = payload.enforce::<PopupSettings>();

        let settings = payload.as_this::<PopupSettings>();

        let Some(settings) = settings else {
          return Ok(ItemEffect::None);
        };

        let available = store
          .borrow::<Renderables>()
          .is_some_and(|x| x.contains_key(&settings.content));

        if available {
          self.current = Some(settings.clone());

          // Let the renderable set itself up (timers, fds, ...).
          self.with_renderable(store, |renderable, store| {
            renderable.initialize(config, store);
          });

          Ok(ItemEffect::ShowAndSubscribe(self.events_for(store)))
        } else {
          eprintln!(
            "[popup] no renderable registered for content '{}'",
            settings.content
          );
          Ok(ItemEffect::None)
        }
      }
      ListenerAction::Payload { name, payload } if name.as_ref() == "popup.close" => {
        let _ = payload;
        self.close_current(store);
        Ok(ItemEffect::Subscribe(Self::base_events()))
      }
      other => {
        if self.current.is_none() {
          return Ok(ItemEffect::None);
        }

        if let Some(Some(effect)) = self.with_renderable(store, |renderable, store| {
          renderable.on_event(config, store, other)
        }) {
          return Ok(effect);
        }

        Ok(ItemEffect::None)
      }
    }
  }

  fn handle_message(&mut self, store: Option<&mut Store>, message: &ItemMessage) -> ItemEffect {
    match message {
      ItemMessage::Effect(_, effect) => match effect {
        ItemEffect::Hide | ItemEffect::Destroy => {
          if let Some(store) = store {
            self.close_current(store);
          } else {
            self.current = None;
          }
          ItemEffect::Hide
        }
        ItemEffect::Custom([tag, ..]) if *tag == 40 || *tag == 42 => {
          if let Some(current) = &self.current
            && let Some(effect) = store
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
    if let Some(renderable) = self.renderable(store)
      && let Some(current) = &self.current
    {
      return renderable.view(config, store, id, current).into();
    }

    space().into()
  }
}

impl Popup {
  fn close_current(&mut self, store: &mut Store) {
    self.with_renderable(store, |renderable, store| {
      renderable.close(store);
    });
    self.current = None;
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
