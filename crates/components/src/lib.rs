pub mod audio;
pub mod bluetooth;
pub mod clock;
pub mod icon;
pub mod network;
pub mod notifications;
pub mod nowplaying;
pub mod power;
pub mod separator;
pub mod sysmon;
pub mod tray;
pub mod window;
pub mod workspaces;

use std::collections::HashMap;

use iced::{Element, Event, mouse};
use slowshell_commons::{
  panels::{PanelEdge, PanelOrientation},
  popups::{PopupPosition, PopupSettings},
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{EventFilter, ItemEffect, ItemMessage},
  types::{PayloadBox, ToUstr, Ustr},
};
use slowshell_widgets::EventWrapper;

pub trait Component: Send {
  fn events(&self) -> Vec<EventFilter> {
    Vec::new()
  }

  fn watch(&mut self, store: &mut Store, options: Option<&ComponentOptions>) {
    let _ = (store, options);
  }

  fn stop(&mut self, store: &Store, options: Option<&ComponentOptions>) {
    let _ = (store, options);
  }

  fn update(
    &mut self,
    config: &Config,
    store: &mut Store,
    event: &ListenerAction,
    options: Option<&ComponentOptions>,
  ) -> anyhow::Result<ItemEffect> {
    let _ = (config, store, event, options);
    Ok(ItemEffect::None)
  }

  fn check_view(&self, store: &Store, options: Option<&ComponentOptions>) -> bool {
    let _ = (store, options);
    true
  }

  fn view<'a>(
    &self,
    config: &Config,
    store: &Store,
    ctx: &ComponentContext,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage>;
}

#[derive(Clone, PartialEq, Debug)]
pub struct ComponentOptions {
  inner: HashMap<Ustr, slowshell_config::Value>,
}

impl ComponentOptions {
  pub fn number(&self, id: impl Into<Ustr>) -> Option<f32> {
    self
      .inner
      .get(&id.into())
      .and_then(|v| v.as_float().map(|x| x as f32))
  }

  pub fn int(&self, id: impl Into<Ustr>) -> Option<i32> {
    self
      .inner
      .get(&id.into())
      .and_then(|v| v.as_int().map(|x| x as i32))
  }

  pub fn str(&self, id: impl Into<Ustr>) -> Option<&str> {
    self.inner.get(&id.into()).and_then(|v| v.as_str())
  }

  pub fn bool(&self, id: impl Into<Ustr>) -> Option<bool> {
    self.inner.get(&id.into()).and_then(|v| v.as_bool())
  }

  pub fn table(&self, id: impl Into<Ustr>) -> Option<&[(String, slowshell_config::Value)]> {
    self.inner.get(&id.into()).and_then(|v| v.as_table())
  }

  pub fn strings(&self, id: impl Into<Ustr>) -> Option<Vec<&str>> {
    self
      .inner
      .get(&id.into())
      .and_then(|v| v.as_array())
      .map(|array| array.iter().filter_map(|v| v.as_str()).collect())
  }
}

impl From<HashMap<Ustr, slowshell_config::Value>> for ComponentOptions {
  fn from(value: HashMap<Ustr, slowshell_config::Value>) -> Self {
    ComponentOptions { inner: value }
  }
}

pub struct ComponentContext<'a> {
  pub panel_name: &'a str,
  pub position: PanelEdge,
  pub orientation: PanelOrientation,
  pub monitor: &'a str,
}

impl<'a> ComponentContext<'a> {
  pub fn resolve_style(&self, config: &slowshell_config::Config) -> slowshell_config::style::Style {
    if self.orientation == PanelOrientation::Horizontal {
      config.style("component")
    } else {
      config.style_any(&["component.vertical", "component"])
    }
  }
}

pub fn vertical_text<'a, Message: 'a>(
  content: &str,
  font_size: f32,
  color: iced::Color,
) -> Element<'a, Message> {
  use iced::widget::{column, text};
  let items: Vec<Element<'a, Message>> = content
    .chars()
    .map(|ch| text(ch.to_string()).size(font_size).color(color).into())
    .collect();
  column(items)
    .spacing(0)
    .align_x(iced::Alignment::Center)
    .into()
}

pub struct MenuConfig {
  pub content: Ustr,
  pub panel_name: Option<Ustr>,
  pub position: PanelEdge,
}

pub fn popup_open_action(config: &MenuConfig, cursor: iced::Point) -> ListenerAction {
  ListenerAction::Payload {
    name: "popup.open".to_ustr(),
    payload: Some(PayloadBox::new(PopupSettings {
      content: config.content.clone(),
      panel_edge: Some(config.position),
      position: match config.position {
        PanelEdge::Top { .. } | PanelEdge::Bottom { .. } => (
          PopupPosition::Fixed(cursor.x),
          if let Some(panel) = &config.panel_name {
            PopupPosition::Panel {
              name: panel.clone(),
            }
          } else {
            PopupPosition::Fixed(0.0)
          },
        ),
        PanelEdge::Left { width } => (
          PopupPosition::Fixed(width as f32),
          PopupPosition::Fixed(cursor.y),
        ),
        PanelEdge::Right { .. } => (PopupPosition::Fixed(0.0), PopupPosition::Fixed(cursor.y)),
      },
    })),
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
    "core/audio".into(),
    Box::new(|| Box::new(audio::Audio::new())),
  );
  components.insert(
    "core/sound".into(),
    Box::new(|| Box::new(audio::Audio::new())),
  );
  components.insert(
    "core/clock".into(),
    Box::new(|| Box::new(clock::Clock::new())),
  );
  components.insert(
    "core/power".into(),
    Box::new(|| Box::new(power::Power::new())),
  );
  components.insert(
    "core/system".into(),
    Box::new(|| Box::new(sysmon::SystemMon::new())),
  );
  components.insert(
    "core/network".into(),
    Box::new(|| Box::new(network::Network::new())),
  );
  components.insert(
    "core/bluetooth".into(),
    Box::new(|| Box::new(bluetooth::Bluetooth::new())),
  );
  components.insert(
    "core/notifications".into(),
    Box::new(|| Box::new(notifications::Notifications::new())),
  );
  components.insert(
    "core/icon".into(),
    Box::new(|| Box::new(icon::IconComp::default())),
  );
  components.insert(
    "core/tray".into(),
    Box::new(|| Box::new(tray::SystemTray::new())),
  );
  components.insert(
    "core/separator".into(),
    Box::new(|| Box::new(separator::Separator)),
  );
  components.insert(
    "core/workspaces".into(),
    Box::new(|| Box::new(workspaces::Workspaces::default())),
  );
  components.insert("core/window".into(), Box::new(|| Box::new(window::Window)));
  components.insert(
    "core/nowplaying".into(),
    Box::new(|| Box::new(nowplaying::NowPlaying::new())),
  );
}

slowshell_registry::register_resources!(
  component: Style(
    slowshell_config::style! {
      "color" => "text",
      "color.faded" => "subtext",
      "color.invert" => "base",
      "color.primary" => "primary",
      "font.size" => 12,
      "icon.size" => 14,
      "spacing" => 5,
      "padding.x" => 9,
      "padding.y" => 5,
    }
  )
);

pub fn register_vertical_style() {
  slowshell_config::style::new_default_style(
    "component.vertical".into(),
    slowshell_config::style! {
      "color" => "text",
      "color.faded" => "subtext",
      "color.invert" => "base",
      "color.primary" => "primary",
      "font.size" => 15,
      "icon.size" => 18,
      "spacing" => 8,
      "padding.x" => 5,
      "padding.y" => 4,
    },
  );
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_popup_open_action_downcast() {
    let menu_config = MenuConfig {
      content: "menus/sound".into(),
      panel_name: None,
      position: PanelEdge::Top { height: 32 },
    };
    let action = popup_open_action(&menu_config, iced::Point::ORIGIN);
    if let ListenerAction::Payload { name, payload } = action {
      assert_eq!(name.as_ref(), "popup.open");
      let payload = payload.expect("payload should be Some");
      println!(
        "target TypeId: {:?}",
        std::any::TypeId::of::<PopupSettings>()
      );
      println!(
        "settings is_some: {:?}",
        payload.as_this::<PopupSettings>().is_some()
      );
      let settings = payload.as_this::<PopupSettings>();
      assert!(
        settings.is_some(),
        "PopupSettings should downcast successfully"
      );
      assert_eq!(settings.unwrap().content.as_ref(), "menus/sound");
    } else {
      panic!("Expected ListenerAction::Payload");
    }
  }
}
