use std::{collections::HashMap, hash::Hash};

use crate::{listeners::IpcCommand, types::Ustr};

use super::listeners::{CompositorEvent, ListenerAction};
use iced_layershell::{
  reexport::{IcedId, NewLayerShellSettings, Task},
  to_layer_message,
};

#[derive(Debug, Clone, Eq)]
pub enum EventFilter {
  UpdateCompositor,
  Compositor(CompositorEvent),
  Named(Ustr),
  Payload(Ustr),
  Ipc(IpcCommand),
  Tick,
  Frame,
  All,
  StartUp,
}

impl PartialEq for EventFilter {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::UpdateCompositor, Self::UpdateCompositor) => true,
      (Self::Compositor(a), Self::Compositor(b)) => a == b,
      (Self::Named(a), Self::Named(b)) => a == b,
      (Self::Payload(a), Self::Payload(b)) => a == b,
      (Self::Tick, Self::Tick) => true,
      (Self::Frame, Self::Frame) => true,
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
      EventFilter::Compositor(events) => {
        events.hash(state);
      }
      EventFilter::Named(name) => {
        name.hash(state);
      }
      EventFilter::Payload(name) => {
        name.hash(state);
      }
      EventFilter::Ipc(cmd) => {
        cmd.name.hash(state);
      }
      EventFilter::Tick => {}
      EventFilter::Frame => {}
      EventFilter::All => {}
      EventFilter::StartUp => {}
    }
  }
}

impl EventFilter {
  pub fn matches(&self, action: &ListenerAction) -> bool {
    match self {
      EventFilter::UpdateCompositor => matches!(action, ListenerAction::UpdateCompositor(_)),
      EventFilter::Compositor(events) => {
        matches!(action, ListenerAction::UpdateCompositor(changed) if changed.intersects(*events))
      }
      EventFilter::Named(name) => {
        matches!(
          action,
          ListenerAction::Named(n)
            | ListenerAction::Timer { name: n, .. }
            | ListenerAction::Signal { name: n, .. }
            if n == name
        ) || matches!(
          action,
          ListenerAction::Payload { name: n, payload: None } if n == name
        )
      }
      EventFilter::Payload(name) => {
        matches!(action, ListenerAction::Payload { name: n, ..} if n == name)
      }
      EventFilter::Ipc(cmd1) => {
        matches!(action, ListenerAction::Ipc(cmd2) if cmd2.name == cmd1.name)
      }
      EventFilter::Tick => false,
      EventFilter::Frame => matches!(action, ListenerAction::Frame),
      EventFilter::All => true,
      EventFilter::StartUp => matches!(action, ListenerAction::StartUp),
    }
  }
}

impl From<EventFilter> for ListenerAction {
  fn from(val: EventFilter) -> Self {
    match val {
      EventFilter::All => ListenerAction::UpdateCompositor(CompositorEvent::ALL),
      EventFilter::UpdateCompositor => ListenerAction::UpdateCompositor(CompositorEvent::ALL),
      EventFilter::Compositor(events) => ListenerAction::UpdateCompositor(events),
      EventFilter::Tick => ListenerAction::None,
      EventFilter::Frame => ListenerAction::Frame,
      EventFilter::Named(n) => ListenerAction::Named(n),
      EventFilter::Payload(name) => ListenerAction::Payload {
        name,
        payload: None,
      },
      EventFilter::Ipc(_) => ListenerAction::None,
      EventFilter::StartUp => ListenerAction::None,
    }
  }
}

impl Into<EventFilter> for &ListenerAction {
  fn into(self) -> EventFilter {
    match self {
      ListenerAction::Named(n) => EventFilter::Named(n.clone()),
      ListenerAction::Timer { name, .. } => EventFilter::Named(name.clone()),
      ListenerAction::Signal { name, .. } => EventFilter::Named(name.clone()),
      ListenerAction::UpdateCompositor(_) => EventFilter::UpdateCompositor,
      ListenerAction::Frame => EventFilter::Frame,
      ListenerAction::None => EventFilter::Tick,
      ListenerAction::StartUp => EventFilter::StartUp,
      ListenerAction::FocusWorkspace(_) => EventFilter::Tick,
      ListenerAction::FocusWindow(_) => EventFilter::Tick,
      ListenerAction::Ipc(cmd) => EventFilter::Ipc(cmd.clone()),
      ListenerAction::Payload { name, .. } => EventFilter::Payload(name.clone()),
    }
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct WindowSettings {
  pub margin: Option<(i32, i32, i32, i32)>,
  pub size: Option<(u32, u32)>,
  pub exclusive_zone: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ItemEffect {
  None,
  Redraw,
  Show,
  ShowAndSubscribe(Vec<EventFilter>),
  Hide,
  ReallyHide,
  Destroy,
  ReallyDestroy,
  Subscribe(Vec<EventFilter>),
  UpdateWindow(WindowSettings),
  Custom([usize; 4]),
}

#[derive(Clone)]
pub enum ItemMessage {
  Effect(IcedId, ItemEffect),
  Action(ListenerAction),
  EffectAction(IcedId, ItemEffect, ListenerAction),
  Noop,
  Task(std::sync::Arc<dyn Fn() -> Task<Message> + Send + Sync>),
  Payload(IcedId, HashMap<Ustr, Ustr>),
}

impl std::fmt::Debug for ItemMessage {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Effect(id, eff) => f.debug_tuple("Effect").field(id).field(eff).finish(),
      Self::Action(act) => f.debug_tuple("Action").field(act).finish(),
      Self::EffectAction(id, eff, act) => f
        .debug_tuple("EffectAction")
        .field(id)
        .field(eff)
        .field(act)
        .finish(),
      Self::Noop => write!(f, "Noop"),
      Self::Task(_) => write!(f, "Task(Arc<dyn Fn() -> Task<Message>>)"),
      Self::Payload(id, map) => f.debug_tuple("Payload").field(id).field(map).finish(),
    }
  }
}

#[to_layer_message(multi)]
#[derive(Debug, Clone)]
pub enum Message {
  Tick,
  FdUpdate(ListenerAction),
  FdBatch(Vec<ListenerAction>),
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::listeners::{CompositorEvent, ListenerAction};

  #[test]
  fn compositor_event_masks_compose() {
    let mut mask = CompositorEvent::NONE;
    assert!(mask.is_empty());

    mask.insert(CompositorEvent::WINDOWS);
    mask |= CompositorEvent::FOCUS;
    assert!(mask.contains(CompositorEvent::WINDOWS));
    assert!(mask.contains(CompositorEvent::WINDOWS | CompositorEvent::FOCUS));
    assert!(!mask.contains(CompositorEvent::WORKSPACES));
    assert!(mask.intersects(CompositorEvent::FOCUS | CompositorEvent::MONITORS));
    assert!(!mask.intersects(CompositorEvent::MONITORS));
    assert!(CompositorEvent::ALL.contains(mask));
  }

  #[test]
  fn compositor_filters_match_changed_areas() {
    let any = EventFilter::UpdateCompositor;
    let workspaces = EventFilter::Compositor(CompositorEvent::WORKSPACES);

    assert!(any.matches(&ListenerAction::UpdateCompositor(CompositorEvent::FOCUS)));
    assert!(workspaces.matches(&ListenerAction::UpdateCompositor(
      CompositorEvent::WORKSPACES
    )));
    assert!(workspaces.matches(&ListenerAction::UpdateCompositor(
      CompositorEvent::WORKSPACES | CompositorEvent::MONITORS
    )));
    assert!(!workspaces.matches(&ListenerAction::UpdateCompositor(CompositorEvent::MONITORS)));
    assert!(!workspaces.matches(&ListenerAction::Named("other".into())));
  }

  #[test]
  fn named_filters_match_payload_actions_without_a_payload() {
    let bare = ListenerAction::Payload {
      name: "fixture.toggle".into(),
      payload: None,
    };
    let with_payload = ListenerAction::Payload {
      name: "fixture.toggle".into(),
      payload: Some(crate::types::PayloadBox::new("value")),
    };

    assert!(EventFilter::Named("fixture.toggle".into()).matches(&bare));
    assert!(!EventFilter::Named("fixture.toggle".into()).matches(&with_payload));
    assert!(!EventFilter::Named("other".into()).matches(&bare));
  }

  #[test]
  fn compositor_actions_map_back_to_the_catch_all_filter() {
    let action = ListenerAction::UpdateCompositor(CompositorEvent::ALL);
    let filter: EventFilter = (&action).into();
    assert_eq!(filter, EventFilter::UpdateCompositor);
  }
}
