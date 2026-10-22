use slowshell_core::{Store, types::Ustr};

use crate::panels::{PanelEdge, PanelPositions};

#[derive(Debug, Clone)]
pub enum PopupPosition {
  Fixed(f32),

  Percentage(f32),

  FollowMouse(f32),

  Panel { name: Ustr },
}

impl PopupPosition {
  pub fn get(&self, store: &Store) -> f32 {
    match self {
      PopupPosition::Fixed(f) => *f,
      PopupPosition::Percentage(_) => 0.,
      PopupPosition::FollowMouse(f) => *f,
      PopupPosition::Panel { name } => {
        match store.borrow::<PanelPositions>().and_then(|p| p.get(name)) {
          Some(pos) => pos.get() as f32,
          None => 0.,
        }
      }
    }
  }

  pub fn set_offset(&self, offset: f32) -> Self {
    match self {
      Self::Fixed(_) => Self::Fixed(offset),
      Self::FollowMouse(_) => Self::FollowMouse(offset),
      _ => self.clone(),
    }
  }

  pub fn offset_map(&self, o: impl FnOnce(&f32) -> f32) -> Self {
    match self {
      Self::Fixed(f) => Self::Fixed(o(f)),
      Self::FollowMouse(f) => Self::FollowMouse(o(f)),
      _ => self.clone(),
    }
  }
}

#[derive(Clone)]
pub struct PopupSettings {
  pub position: (PopupPosition, PopupPosition),
  pub content: Ustr,
  pub panel_edge: Option<PanelEdge>,
  pub insets: bool,
  pub monitor: Option<Ustr>,
}

impl PopupPosition {
  pub fn to_payload_string(&self) -> String {
    match self {
      PopupPosition::Fixed(val) => format!("fixed,{val}"),
      PopupPosition::FollowMouse(val) => format!("cursor,{val}"),
      PopupPosition::Panel { name } => format!("panel,{name}"),
      PopupPosition::Percentage(val) => format!("fixed,{val}"),
    }
  }
}
