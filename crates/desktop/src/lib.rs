pub mod manager;
pub mod wallpaper;

use iced::Element;
use iced_layershell::reexport::{IcedId, Layer, NewLayerShellSettings};
use slowshell_config::Config;
use slowshell_core::{Store, listeners::ListenerAction, types::Ustr};

pub use manager::DesktopItems;
pub use slowshell_core::message::{EventFilter, ItemEffect, ItemMessage};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DesktopLayer {
  Background,
  Bottom,
  Top,
  Overlay,
}

impl DesktopLayer {
  pub fn to_wayland(&self) -> Layer {
    match self {
      DesktopLayer::Background => Layer::Background,
      DesktopLayer::Bottom => Layer::Bottom,
      DesktopLayer::Top => Layer::Top,
      DesktopLayer::Overlay => Layer::Overlay,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpdateWhen {
  EveryFrame,
  OnEvent,
  OnDemand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Visibility {
  Visible,
  Toggleable(bool),
  Transient,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MonitorScope {
  Single(Option<Ustr>),
  PerMonitor,
  OnMonitors(Vec<Ustr>),
}

impl Default for MonitorScope {
  fn default() -> Self {
    MonitorScope::Single(None)
  }
}

pub trait DesktopItem: Send {
  fn id(&self) -> &str;

  fn layer(&self, _config: &Config, monitor: &str) -> NewLayerShellSettings;

  fn visibility(&self) -> Visibility;

  fn update_strategy(&self) -> UpdateWhen;

  fn monitor(&self, _config: &Config) -> MonitorScope {
    MonitorScope::default()
  }

  // fn clone_box(&self) -> Box<dyn DesktopItem>;

  fn init_events(&self) -> Vec<EventFilter>;

  fn update(&mut self, store: &Store, event: &ListenerAction) -> anyhow::Result<ItemEffect>;

  fn view(&self, store: &Store, id: IcedId) -> Element<'_, ItemMessage>;

  fn handle_message(&mut self, message: &ItemMessage) -> ItemEffect {
    let _ = message;
    ItemEffect::None
  }
}
