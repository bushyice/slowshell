use std::{collections::HashMap, hash::Hash};

use crate::{listeners::IpcCommand, types::Ustr};

use super::listeners::ListenerAction;
use iced_layershell::{
  reexport::{IcedId, NewLayerShellSettings, Task},
  to_layer_message,
};

#[derive(Debug, Clone, Eq)]
pub enum EventFilter {
  UpdateCompositor,
  Named(Ustr),
  Payload {
    name: Ustr,
    payload: Option<HashMap<Ustr, Ustr>>,
  },
  Ipc(IpcCommand),
  Tick,
  All,
  StartUp,
}

impl PartialEq for EventFilter {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::UpdateCompositor, Self::UpdateCompositor) => true,
      (Self::Named(a), Self::Named(b)) => a == b,
      (Self::Payload { name: a, .. }, Self::Payload { name: b, .. }) => a == b,
      (Self::Tick, Self::Tick) => true,
      (Self::Ipc(cmd1), Self::Ipc(cmd2)) => cmd1.name == cmd2.name,
      (Self::All, Self::All) => true,
      _ => false,
    }
  }
}

impl std::hash::Hash for EventFilter {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    use std::mem::discriminant;

    discriminant(self).hash(state);

    match self {
      EventFilter::UpdateCompositor => {}
      EventFilter::Named(name) => {
        name.hash(state);
      }
      EventFilter::Payload { name, .. } => {
        name.hash(state);
      }
      EventFilter::Ipc(cmd) => {
        cmd.name.hash(state);
      }
      EventFilter::Tick => {}
      EventFilter::All => {}
      EventFilter::StartUp => {}
    }
  }
}

impl EventFilter {
  pub fn matches(&self, action: &ListenerAction) -> bool {
    match self {
      EventFilter::UpdateCompositor => matches!(action, ListenerAction::UpdateCompositor),
      EventFilter::Named(name) => matches!(action, ListenerAction::Named(n) if n == name),
      EventFilter::Payload { name, .. } => {
        matches!(action, ListenerAction::Payload { name: n, ..} if n == name)
      }
      EventFilter::Ipc(cmd1) => {
        matches!(action, ListenerAction::Ipc(cmd2) if cmd2.name == cmd1.name)
      }
      EventFilter::Tick => false,
      EventFilter::All => true,
      EventFilter::StartUp => matches!(action, ListenerAction::StartUp),
    }
  }
}

impl From<EventFilter> for ListenerAction {
  fn from(val: EventFilter) -> Self {
    match val {
      EventFilter::All => ListenerAction::UpdateCompositor,
      EventFilter::UpdateCompositor => ListenerAction::UpdateCompositor,
      EventFilter::Tick => ListenerAction::None,
      EventFilter::Named(n) => ListenerAction::Named(n),
      EventFilter::Payload { name, payload } => ListenerAction::Payload { name, payload },
      EventFilter::Ipc(_) => ListenerAction::None,
      EventFilter::StartUp => ListenerAction::None,
    }
  }
}

impl Into<EventFilter> for &ListenerAction {
  fn into(self) -> EventFilter {
    match self {
      ListenerAction::Named(n) => EventFilter::Named(n.clone()),
      ListenerAction::UpdateCompositor => EventFilter::UpdateCompositor,
      ListenerAction::None => EventFilter::Tick,
      ListenerAction::StartUp => EventFilter::StartUp,
      ListenerAction::Ipc(cmd) => EventFilter::Ipc(cmd.clone()),
      ListenerAction::Payload { name, payload } => EventFilter::Payload {
        name: name.clone(),
        payload: payload.clone(),
      },
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ItemEffect {
  None,
  Redraw,
  Show,
  Hide,
  Destroy,
  ReallyDestroy,
  Subscribe(Vec<EventFilter>),
}

#[derive(Debug, Clone)]
pub enum ItemMessage {
  Effect(IcedId, ItemEffect),
  Action(ListenerAction),
  Noop,
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
  Tick,
  FdUpdate(ListenerAction),
  LinkAction(u32, ListenerAction),
  LinkedAction(u32, ListenerAction),
  Item(ItemMessage),
  UpdateMonitors,
  Noop,
}

impl Message {
  pub fn open_layer(settings: NewLayerShellSettings) -> (IcedId, Task<Message>) {
    Self::layershell_open(settings)
  }
}
