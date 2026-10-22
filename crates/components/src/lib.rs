#[cfg(feature = "audio")]
pub mod audio;
#[cfg(feature = "bluetooth")]
pub mod bluetooth;
pub mod clock;
pub mod icon;
#[cfg(feature = "network")]
pub mod network;
#[cfg(feature = "notch")]
pub mod notch;
#[cfg(feature = "notifications")]
pub mod notifications;
#[cfg(feature = "audio")]
pub mod nowplaying;
#[cfg(feature = "power")]
pub mod power;
pub mod separator;
pub mod sysmon;
#[cfg(feature = "tray")]
pub mod tray;
pub mod window;
pub mod windows;
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
  commands::CommandEntry,
  listeners::ListenerAction,
  message::{EventFilter, ItemEffect, ItemMessage},
  types::{PayloadBox, ToUstr, Ustr},
};
use slowshell_widgets::{EventWrapper, HoverBackground};

pub use slowshell_widgets::ComponentOptions;

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
  ) -> miette::Result<ItemEffect> {
    let _ = (config, store, event, options);
    Ok(ItemEffect::None)
  }

  fn check_view(&self, store: &Store, options: Option<&ComponentOptions>) -> bool {
    let _ = (store, options);
    true
  }

  fn view<'a>(
    &'a self,
    config: &'a Config,
    store: &'a Store,
    ctx: &ComponentContext,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage>;
}

#[derive(Clone, PartialEq, Debug)]
pub struct ComponentContext<'a> {
  pub panel_name: &'a str,
  pub position: PanelEdge,
  pub orientation: PanelOrientation,
  pub monitor: &'a str,
  pub menu_suppressed: bool,
}

pub use slowshell_commons::notch::{NotchStatus, NotchStatusStore};

impl<'a> ComponentContext<'a> {
  pub fn classes(&self) -> &'static [&'static str] {
    if self.orientation == PanelOrientation::Horizontal {
      &["component"]
    } else {
      &["component", "vertical"]
    }
  }

  pub fn update_status(&self, name: impl Into<String>, status: NotchStatus) {
    slowshell_commons::notch::shared().update_status(name, status);
  }

  pub fn remove_status(&self, name: &str) {
    slowshell_commons::notch::shared().remove_status(name);
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

pub fn popup_open_action(
  config: &MenuConfig,
  cursor: iced::Point,
  monitor: &str,
) -> ListenerAction {
  ListenerAction::Payload {
    name: "popup.toggle".to_ustr(),
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
      insets: true,
      monitor: Some(monitor.to_ustr()),
    })),
  }
}

pub fn fill_cross<'a, Message: 'a>(
  content: Element<'a, Message>,
  orientation: PanelOrientation,
) -> Element<'a, Message> {
  if orientation == PanelOrientation::Vertical {
    iced::widget::container(content)
      .width(iced::Length::Fill)
      .align_x(iced::Alignment::Center)
      .into()
  } else {
    iced::widget::container(content)
      .height(iced::Length::Fill)
      .align_y(iced::Alignment::Center)
      .into()
  }
}

pub fn spaced_component<'a>(
  config: &'a Config,
  ctx: &ComponentContext,
  content: Element<'a, ItemMessage>,
  hoverable: bool,
) -> Element<'a, ItemMessage> {
  let item = config.sizing(&["item"]);
  let spacing = item.gap;
  let padding = if ctx.orientation == PanelOrientation::Vertical {
    [spacing / 2., item.pad_h]
  } else {
    [item.pad_v, spacing / 2.]
  };

  let body = iced::widget::container(fill_cross(content, ctx.orientation)).padding(padding);

  if !hoverable || ctx.menu_suppressed {
    return body.into();
  }

  hover_background(config, ctx, body.into())
}

pub fn hover_background<'a>(
  config: &'a Config,
  ctx: &ComponentContext,
  content: Element<'a, ItemMessage>,
) -> Element<'a, ItemMessage> {
  let component_style = config.computed(ctx.classes());
  let radius = component_style.length("hover-radius").unwrap_or(4.0);

  match component_style
    .color("hover-background")
    .map(|c| c.to_iced())
  {
    Some(color) => HoverBackground::new(content, color, radius).into(),
    None => content,
  }
}

pub fn menu_trigger<'a>(
  content: Element<'a, ItemMessage>,
  config: MenuConfig,
  app_config: &'a Config,
  ctx: &ComponentContext,
  wrapper: bool,
) -> Element<'a, ItemMessage> {
  let body = if wrapper {
    spaced_component(app_config, ctx, content, true)
  } else {
    fill_cross(content, ctx.orientation)
  };

  if ctx.menu_suppressed {
    return body;
  }

  let monitor = ctx.monitor.to_ustr();
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
      Some(ItemMessage::Action(popup_open_action(
        &config, point, &monitor,
      )))
    };

  EventWrapper::new(body, action).into()
}

pub type ComponentFactory = Box<dyn Fn() -> Box<dyn Component> + Send + Sync>;
pub type Components = HashMap<Ustr, ComponentFactory>;

pub struct ComponentRegistration {
  pub name: Ustr,
  pub factory: ComponentFactory,
}

impl ComponentRegistration {
  pub fn new(name: impl Into<Ustr>, factory: ComponentFactory) -> Self {
    Self {
      name: name.into(),
      factory,
    }
  }
}

pub fn register_all(components: &mut Components) {
  #[cfg(feature = "audio")]
  components.insert(
    "core/audio".into(),
    Box::new(|| Box::new(audio::Audio::new())),
  );
  #[cfg(feature = "audio")]
  components.insert(
    "core/sound".into(),
    Box::new(|| Box::new(audio::Audio::new())),
  );
  components.insert(
    "core/clock".into(),
    Box::new(|| Box::new(clock::Clock::new())),
  );
  #[cfg(feature = "power")]
  components.insert(
    "core/power".into(),
    Box::new(|| Box::new(power::Power::new())),
  );
  components.insert(
    "core/system".into(),
    Box::new(|| Box::new(sysmon::SystemMon::new())),
  );
  #[cfg(feature = "network")]
  components.insert(
    "core/network".into(),
    Box::new(|| Box::new(network::Network::new())),
  );
  #[cfg(feature = "bluetooth")]
  components.insert(
    "core/bluetooth".into(),
    Box::new(|| Box::new(bluetooth::Bluetooth::new())),
  );
  #[cfg(feature = "notifications")]
  components.insert(
    "core/notifications".into(),
    Box::new(|| Box::new(notifications::Notifications::new())),
  );
  components.insert(
    "core/icon".into(),
    Box::new(|| Box::new(icon::IconComp::default())),
  );
  #[cfg(feature = "tray")]
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
    "core/windows".into(),
    Box::new(|| Box::new(windows::Windows::default())),
  );
  #[cfg(feature = "audio")]
  components.insert(
    "core/nowplaying".into(),
    Box::new(|| Box::new(nowplaying::NowPlaying::new())),
  );
  #[cfg(feature = "notch")]
  components.insert(
    "core/notch".into(),
    Box::new(|| Box::new(notch::Notch::new())),
  );
}

slowshell_registry::register_resources!(
  component: Style(include_str!("./styles/components.icss").to_string()),
  app: Custom(|store| {
    slowshell_commons::desktop::DesktopEntries::initialize_unless(store);
  }),
  audio: Command(
    CommandEntry::payload("audio.toggle_mute").title("Toggle Mute"),
  ),
  audio: Command(
    CommandEntry::payload("audio.play_pause").title("Play / Pause"),
  ),
  audio: Command(
    CommandEntry::payload("audio.next").title("Next Track"),
  ),
  audio: Command(
    CommandEntry::payload("audio.previous").title("Previous Track"),
  ),
);

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
    let action = popup_open_action(&menu_config, iced::Point::ORIGIN, "DP-1");
    if let ListenerAction::Payload { name, payload } = action {
      assert_eq!(name.as_ref(), "popup.toggle");
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
      let settings = settings.unwrap();
      assert!(settings.insets);
      assert_eq!(settings.content.as_ref(), "menus/sound");
      assert_eq!(settings.monitor.as_deref(), Some("DP-1"));
    } else {
      panic!("Expected ListenerAction::Payload");
    }
  }
}
