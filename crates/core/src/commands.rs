use std::sync::{Mutex, OnceLock};

use crate::{listeners::ListenerAction, types::Ustr};

#[derive(Clone, Debug)]
pub struct CommandEntry {
  pub name: Ustr,
  pub title: Option<String>,
  pub description: Option<String>,
  pub icon: Option<String>,
  pub action: ListenerAction,
}

impl CommandEntry {
  pub fn named(name: impl Into<Ustr>) -> Self {
    let name = name.into();
    Self {
      action: ListenerAction::Named(name.clone()),
      name,
      title: None,
      description: None,
      icon: None,
    }
  }

  pub fn payload(name: impl Into<Ustr>) -> Self {
    let name = name.into();
    Self {
      action: ListenerAction::Payload {
        name: name.clone(),
        payload: None,
      },
      name,
      title: None,
      description: None,
      icon: None,
    }
  }

  pub fn title(mut self, title: impl Into<String>) -> Self {
    self.title = Some(title.into());
    self
  }

  pub fn description(mut self, description: impl Into<String>) -> Self {
    self.description = Some(description.into());
    self
  }

  pub fn icon(mut self, icon: impl Into<String>) -> Self {
    self.icon = Some(icon.into());
    self
  }

  pub fn action(mut self, action: ListenerAction) -> Self {
    self.action = action;
    self
  }

  pub fn display(&self) -> String {
    self.title.clone().unwrap_or_else(|| self.name.to_string())
  }
}

#[derive(Default)]
pub struct CommandRegistry {
  entries: Mutex<Vec<CommandEntry>>,
}

impl CommandRegistry {
  pub fn register(&self, entry: CommandEntry) {
    let mut entries = self.entries.lock().unwrap();

    if let Some(existing) = entries
      .iter_mut()
      .find(|existing| existing.name == entry.name)
    {
      *existing = entry;
      return;
    }

    entries.push(entry);
  }

  pub fn register_default(&self, entry: CommandEntry) {
    let mut entries = self.entries.lock().unwrap();

    if entries.iter().any(|existing| existing.name == entry.name) {
      return;
    }

    entries.push(entry);
  }

  pub fn unregister(&self, name: &str) -> bool {
    let mut entries = self.entries.lock().unwrap();
    let before = entries.len();
    entries.retain(|entry| &*entry.name != name);
    entries.len() != before
  }

  pub fn get(&self, name: &str) -> Option<CommandEntry> {
    self
      .entries
      .lock()
      .unwrap()
      .iter()
      .find(|entry| &*entry.name == name)
      .cloned()
  }

  pub fn contains(&self, name: &str) -> bool {
    self
      .entries
      .lock()
      .unwrap()
      .iter()
      .any(|entry| &*entry.name == name)
  }

  pub fn list(&self) -> Vec<CommandEntry> {
    self.entries.lock().unwrap().clone()
  }

  pub fn len(&self) -> usize {
    self.entries.lock().unwrap().len()
  }

  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }
}

static REGISTRY: OnceLock<CommandRegistry> = OnceLock::new();

pub fn commands() -> &'static CommandRegistry {
  REGISTRY.get_or_init(CommandRegistry::default)
}

pub fn register(entry: CommandEntry) {
  commands().register(entry);
}

pub fn register_default(entry: CommandEntry) {
  commands().register_default(entry);
}
