use std::fmt::Debug;

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelEdge {
  Top { height: u32 },
  Bottom { height: u32 },
  Left { width: u32 },
  Right { width: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelOrientation {
  Horizontal,
  Vertical,
}

impl PanelEdge {
  pub fn get(&self) -> u32 {
    match self {
      PanelEdge::Bottom { height } => *height,
      PanelEdge::Top { height } => *height,
      PanelEdge::Right { width } => *width,
      PanelEdge::Left { width } => *width,
    }
  }
}

#[derive(Debug, Clone)]
struct PanelEntry {
  edge: PanelEdge,
  monitors: Option<Vec<Ustr>>,
}

#[derive(Debug, Clone, Default)]
pub struct PanelPositions {
  entries: HashMap<Ustr, PanelEntry>,
}

impl PanelPositions {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn set(&mut self, name: impl Into<Ustr>, edge: PanelEdge, monitors: Option<Vec<Ustr>>) {
    self
      .entries
      .insert(name.into(), PanelEntry { edge, monitors });
  }

  pub fn remove(&mut self, name: &str) {
    self.entries.remove(name);
  }

  pub fn get(&self, name: &str) -> Option<&PanelEdge> {
    self.entries.get(name).map(|entry| &entry.edge)
  }

  pub fn keys(&self) -> impl Iterator<Item = &Ustr> + Debug {
    self.entries.keys()
  }

  pub fn for_each(&self, mut f: impl FnMut(&Ustr, &PanelEdge)) {
    for (name, entry) in &self.entries {
      f(name, &entry.edge);
    }
  }

  pub fn insets(&self, monitor: Option<&str>) -> (u32, u32, u32, u32) {
    let mut insets = (0, 0, 0, 0);

    for entry in self.entries.values() {
      if !entry.covers(monitor) {
        continue;
      }

      match entry.edge {
        PanelEdge::Top { height } => insets.0 += height,
        PanelEdge::Right { width } => insets.1 += width,
        PanelEdge::Bottom { height } => insets.2 += height,
        PanelEdge::Left { width } => insets.3 += width,
      }
    }

    insets
  }

  pub fn iter(&self) -> impl Iterator<Item = (&Ustr, &PanelEdge)> {
    self.entries.iter().map(|(name, entry)| (name, &entry.edge))
  }
}

impl PanelEntry {
  fn covers(&self, monitor: Option<&str>) -> bool {
    let Some(monitor) = monitor else {
      return true;
    };

    self
      .monitors
      .as_ref()
      .is_none_or(|monitors| monitors.iter().any(|m| m.as_ref() == monitor))
  }
}
