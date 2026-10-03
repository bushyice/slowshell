use std::sync::{Arc, Mutex};

use slowshell_config::Config;
use slowshell_core::{
  Store,
  message::{EventFilter, ItemMessage},
  types::{PayloadBox, ToUstr},
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TileAction {
  #[default]
  None,
  Command(String),
  Commands(Vec<String>),
  Menu(String),
  Expand,
}

#[derive(Debug, Clone, Default)]
pub struct QuickSettingsTile {
  pub id: String,
  pub icon: String,
  pub title: String,
  pub status: String,
  pub detail: Option<String>,
  pub active: bool,
  pub accent: Option<String>,
  pub on_click: TileAction,
  pub order: i32,
}

impl QuickSettingsTile {
  pub fn new(id: impl Into<String>, icon: impl Into<String>, title: impl Into<String>) -> Self {
    Self {
      id: id.into(),
      icon: icon.into(),
      title: title.into(),
      order: 100,
      ..Default::default()
    }
  }

  pub fn status(mut self, status: impl Into<String>) -> Self {
    self.status = status.into();
    self
  }

  pub fn active(mut self, active: bool) -> Self {
    self.active = active;
    self
  }

  pub fn action(mut self, command: impl Into<String>) -> Self {
    self.on_click = TileAction::Command(command.into());
    self
  }

  pub fn commands<I, S>(mut self, commands: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<String>,
  {
    self.on_click = TileAction::Commands(commands.into_iter().map(Into::into).collect());
    self
  }

  pub fn menu(mut self, menu: impl Into<String>) -> Self {
    self.on_click = TileAction::Menu(menu.into());
    self
  }

  pub fn expandable(mut self) -> Self {
    self.on_click = TileAction::Expand;
    self
  }

  pub fn accent(mut self, key: impl Into<String>) -> Self {
    self.accent = Some(key.into());
    self
  }

  pub fn order(mut self, order: i32) -> Self {
    self.order = order;
    self
  }
}

pub trait QuickSettingsProvider: Send + Sync {
  fn id(&self) -> &str;

  fn events(&self) -> Vec<EventFilter> {
    Vec::new()
  }

  fn tiles(&self, _config: &Config, store: &Store) -> Vec<QuickSettingsTile>;

  fn children(&self, _config: &Config, _store: &Store, _id: &str) -> Vec<QuickSettingsTile> {
    Vec::new()
  }

  fn section<'a>(
    &self,
    _config: &Config,
    _store: &Store,
  ) -> Option<iced::Element<'a, ItemMessage>> {
    None
  }
}

#[derive(Default, Clone)]
pub struct QuickSettingsRegistry {
  providers: Arc<Mutex<Vec<Arc<dyn QuickSettingsProvider>>>>,
}

impl QuickSettingsRegistry {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn register(&self, provider: Arc<dyn QuickSettingsProvider>) {
    if let Ok(mut providers) = self.providers.lock() {
      let id = provider.id().to_string();
      providers.retain(|existing| existing.id() != id);
      providers.push(provider);
    }
  }

  pub fn unregister(&self, id: &str) {
    if let Ok(mut providers) = self.providers.lock() {
      providers.retain(|provider| provider.id() != id);
    }
  }

  pub fn tiles(&self, config: &Config, store: &Store) -> Vec<QuickSettingsTile> {
    let providers: Vec<Arc<dyn QuickSettingsProvider>> = self
      .providers
      .lock()
      .map(|providers| providers.clone())
      .unwrap_or_default();

    let mut tiles: Vec<QuickSettingsTile> = providers
      .iter()
      .flat_map(|provider| provider.tiles(config, store))
      .collect();

    tiles.sort_by_cached_key(|tile| (tile.order, tile.title.to_lowercase()));

    tiles
  }

  pub fn events(&self) -> Vec<EventFilter> {
    let providers: Vec<Arc<dyn QuickSettingsProvider>> = self
      .providers
      .lock()
      .map(|providers| providers.clone())
      .unwrap_or_default();

    let mut events = Vec::new();
    for provider in providers.iter() {
      for event in provider.events() {
        if !events.contains(&event) {
          events.push(event);
        }
      }
    }

    events
  }

  pub fn children(&self, config: &Config, store: &Store, id: &str) -> Vec<QuickSettingsTile> {
    let providers: Vec<Arc<dyn QuickSettingsProvider>> = self
      .providers
      .lock()
      .map(|providers| providers.clone())
      .unwrap_or_default();

    providers
      .iter()
      .flat_map(|provider| provider.children(config, store, id))
      .collect()
  }

  pub fn sections<'a>(
    &self,
    config: &Config,
    store: &Store,
  ) -> Vec<iced::Element<'a, ItemMessage>> {
    let providers: Vec<Arc<dyn QuickSettingsProvider>> = self
      .providers
      .lock()
      .map(|providers| providers.clone())
      .unwrap_or_default();

    providers
      .iter()
      .filter_map(|provider| provider.section(config, store))
      .collect()
  }
}

pub fn ensure_registry(store: &mut Store) -> QuickSettingsRegistry {
  if let Some(registry) = store.borrow::<QuickSettingsRegistry>() {
    return registry.clone();
  }

  let registry = QuickSettingsRegistry::new();
  store.insert(registry.clone());
  registry
}

pub fn tile_message(command: &str) -> ItemMessage {
  ItemMessage::Action(slowshell_core::listeners::ListenerAction::Payload {
    name: command.to_ustr(),
    payload: Some(PayloadBox::new(QuickSettingsAction {
      command: command.to_string(),
    })),
  })
}

#[derive(Debug, Clone)]
pub struct QuickSettingsAction {
  pub command: String,
}
