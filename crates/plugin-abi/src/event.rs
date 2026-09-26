use abi_stable::{
  StableAbi,
  std_types::{RString, RVec},
};

use crate::payload::PayloadBox;

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetEventKind {
  Input,
  Submit,
  Custom,
  Scroll,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, PartialEq)]
pub struct WidgetEvent {
  pub id: RString,
  pub kind: WidgetEventKind,
  pub value: RString,
  pub scroll: [f32; 2],
}
#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompositorEvent(pub u8);

impl CompositorEvent {
  pub const NONE: Self = Self(0);
  pub const WINDOWS: Self = Self(1 << 0);
  pub const FOCUS: Self = Self(1 << 1);
  pub const WORKSPACES: Self = Self(1 << 2);
  pub const MONITORS: Self = Self(1 << 3);
  pub const OVERVIEW: Self = Self(1 << 4);
  pub const ALL: Self = Self(0b1_1111);

  pub const fn bits(self) -> u8 {
    self.0
  }

  pub const fn from_bits(bits: u8) -> Self {
    Self(bits)
  }

  pub const fn is_empty(self) -> bool {
    self.0 == 0
  }

  pub const fn contains(self, other: Self) -> bool {
    self.0 & other.0 == other.0
  }

  pub const fn intersects(self, other: Self) -> bool {
    self.0 & other.0 != 0
  }

  pub fn insert(&mut self, other: Self) {
    self.0 |= other.0;
  }
}

impl core::ops::BitOr for CompositorEvent {
  type Output = Self;

  fn bitor(self, rhs: Self) -> Self {
    Self(self.0 | rhs.0)
  }
}

impl core::ops::BitOrAssign for CompositorEvent {
  fn bitor_assign(&mut self, rhs: Self) {
    self.0 |= rhs.0;
  }
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, PartialEq, Eq)]
pub enum EventFilter {
  Tick,
  Fd,
  Widget,
  ConfigReload,
  CompositorUpdate,
  Compositor(CompositorEvent),
  Frame,
  Named(RString),
  Payload(RString),
  Ipc(RString),
  StartUp,
  All,
}

impl EventFilter {
  pub const fn tick() -> Self {
    Self::Tick
  }

  pub const fn frame() -> Self {
    Self::Frame
  }

  pub const fn compositor() -> Self {
    Self::CompositorUpdate
  }

  pub fn named(name: impl Into<RString>) -> Self {
    Self::Named(name.into())
  }

  pub fn payload(name: impl Into<RString>) -> Self {
    Self::Payload(name.into())
  }

  pub fn ipc(name: impl Into<RString>) -> Self {
    Self::Ipc(name.into())
  }
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub enum Event {
  Tick {
    tag: u32,
    fd: i32,
  },
  Fd {
    action: u32,
    fd: i32,
  },
  ConfigReload,
  CompositorUpdate(CompositorEvent),
  Frame,
  Named(RString),
  Payload {
    name: RString,
    payload: PayloadBox,
  },
  Ipc {
    name: RString,
    command: RString,
    args: RVec<RString>,
  },
  StartUp,
  Widget(WidgetEvent),
  Other(u32),
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemEffect {
  None,
  Redraw,
  Subscribe,
  Hide,
  Show,
  ReallyHide,
  Destroy,
  ReallyDestroy,
  Custom([usize; 4]),
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, PartialEq, Eq)]
pub enum Message {
  Noop,
  Effect(ItemEffect),
  Action(RString),
  EffectAction(ItemEffect, RString),
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, PartialEq, Eq)]
pub struct ArgEntry {
  pub key: RString,
  pub value: RString,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq, Eq)]
pub struct Args {
  entries: RVec<ArgEntry>,
}

impl Args {
  pub fn new(entries: impl IntoIterator<Item = (RString, RString)>) -> Self {
    Self {
      entries: entries
        .into_iter()
        .map(|(key, value)| ArgEntry { key, value })
        .collect(),
    }
  }

  pub fn get(&self, key: &str) -> Option<&str> {
    self
      .entries
      .iter()
      .find(|entry| &*entry.key == key)
      .map(|entry| &*entry.value)
  }

  pub fn get_f64(&self, key: &str) -> Option<f64> {
    self.get(key).and_then(|value| value.parse().ok())
  }

  pub fn get_i64(&self, key: &str) -> Option<i64> {
    self.get(key).and_then(|value| value.parse().ok())
  }

  pub fn get_bool(&self, key: &str) -> Option<bool> {
    self.get(key).and_then(|value| value.parse().ok())
  }

  pub fn contains(&self, key: &str) -> bool {
    self.entries.iter().any(|entry| &*entry.key == key)
  }

  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
    self
      .entries
      .iter()
      .map(|entry| (&*entry.key, &*entry.value))
  }

  pub fn render(&self) -> String {
    self
      .entries
      .iter()
      .map(|entry| format!("{}={}", entry.key, entry.value))
      .collect::<Vec<_>>()
      .join(" ")
  }
}
