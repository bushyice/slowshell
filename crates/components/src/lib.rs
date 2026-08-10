pub mod battery;
pub mod clock;
pub mod cpu;
pub mod wifi;

use std::collections::HashMap;

use iced::{Element, Event, mouse};
use slowshell_commons::panels::PanelEdge;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{EventFilter, ItemEffect, ItemMessage},
  types::{ToUstr, Ustr},
};
use slowshell_widgets::EventWrapper;

pub trait Component: Send {
  fn events(&self) -> Vec<EventFilter> {
    Vec::new()
  }

  fn watch(&mut self, store: &Store) {
    let _ = store;
  }

  fn stop(&mut self, store: &Store) {
    let _ = store;
  }

  fn update(&mut self, store: &mut Store, event: &ListenerAction) -> anyhow::Result<ItemEffect> {
    let _ = (store, event);
    Ok(ItemEffect::None)
  }

  fn view<'a>(&self, store: &Store, ctx: &ComponentContext) -> Element<'a, ItemMessage>;
}

pub struct ComponentContext<'a> {
  pub panel_name: &'a str,
  pub position: PanelEdge,
}

pub struct MenuConfig {
  pub content: String,
  pub panel_name: Option<String>,
  pub position: PanelEdge,
}

pub fn popup_open_action(config: &MenuConfig, cursor: iced::Point) -> ListenerAction {
  let mut payload: HashMap<Ustr, Ustr> = HashMap::new();
  payload.insert("content".to_ustr(), config.content.to_ustr());

  let (x, y) = match config.position {
    PanelEdge::Top { .. } | PanelEdge::Bottom { .. } => (
      format!("fixed,{}", cursor.x),
      if let Some(panel) = &config.panel_name {
        format!("panel,{}", panel)
      } else {
        format!("fixed,0.0")
      },
    ),
    PanelEdge::Left { .. } | PanelEdge::Right { .. } => (
      if let Some(panel) = &config.panel_name {
        format!("panel,{}", panel)
      } else {
        format!("fixed,0.0")
      },
      format!("fixed,{}", cursor.y),
    ),
  };

  payload.insert("x".to_ustr(), x.to_ustr());
  payload.insert("y".to_ustr(), y.to_ustr());

  ListenerAction::Payload {
    name: "popup.open".to_ustr(),
    payload: Some(payload),
  }
}

pub fn menu_trigger<'a>(
  content: Element<'a, ItemMessage>,
  config: MenuConfig,
) -> Element<'a, ItemMessage> {
  let action =
    move |event: &Event, layout: iced::advanced::layout::Layout<'_>, cursor: mouse::Cursor| {
      if !cursor.is_over(layout.bounds()) {
        return None;
      }
      let Event::Mouse(mouse::Event::ButtonPressed(button)) = event else {
        return None;
      };
      if !matches!(button, mouse::Button::Left | mouse::Button::Right) {
        return None;
      }
      let Some(point) = cursor.position() else {
        return None;
      };
      Some(ItemMessage::Action(popup_open_action(&config, point)))
    };

  EventWrapper::new(content, action).into()
}

pub type ComponentFactory = Box<dyn Fn() -> Box<dyn Component> + Send + Sync>;
pub type Components = HashMap<Ustr, ComponentFactory>;

pub fn register_all(components: &mut Components) {
  components.insert(
    "core/clock".into(),
    Box::new(|| Box::new(clock::Clock::new())),
  );
  components.insert(
    "core/battery".into(),
    Box::new(|| Box::new(battery::Battery::new())),
  );
  components.insert("core/cpu".into(), Box::new(|| Box::new(cpu::CpuMem::new())));
  components.insert("core/wifi".into(), Box::new(|| Box::new(wifi::Wifi::new())));
}
