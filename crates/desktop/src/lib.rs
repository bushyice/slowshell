pub mod manager;

use iced::Element;
use iced_layershell::reexport::{IcedId, Layer, NewLayerShellSettings};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  types::{Ustr, Void},
};

pub use manager::DesktopItems;
pub use slowshell_core::message::{EventFilter, ItemEffect, ItemMessage, WindowSettings};

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

  fn initialize(&mut self, store: &mut Store) -> miette::Result<Void> {
    let _ = store;
    Ok(())
  }

  fn update(
    &mut self,
    config: &Config,
    store: &mut Store,
    event: &ListenerAction,
  ) -> miette::Result<ItemEffect>;

  fn view<'a>(
    &'a self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    monitor: &str,
  ) -> Element<'a, ItemMessage>;

  fn handle_message(&mut self, store: Option<&mut Store>, message: &ItemMessage) -> ItemEffect {
    let _ = (store, message);
    ItemEffect::None
  }
}

pub enum DeployDesktopItemAction {
  Deploy(Box<dyn DesktopItem>),
  Destroy(Ustr),
  Many(Vec<DeployDesktopItemAction>),
}

impl DeployDesktopItemAction {
  pub fn as_many(self) -> Vec<DeployDesktopItemAction> {
    match self {
      DeployDesktopItemAction::Many(items) => items,
      _ => vec![self],
    }
  }
}

pub trait DeployableDesktopItem: Send {
  fn id(&self) -> &str;

  fn deploys_on(&self) -> Vec<EventFilter>;

  fn deploy(
    &mut self,
    config: &Config,
    store: &mut Store,
    action: &ListenerAction,
  ) -> miette::Result<Option<DeployDesktopItemAction>>;
}
