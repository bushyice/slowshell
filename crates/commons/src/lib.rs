use std::collections::HashMap;

use slowshell_core::types::Ustr;

pub mod panels {
  use super::*;

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum PanelEdge {
    Top { height: u32 },
    Bottom { height: u32 },
    Left { width: u32 },
    Right { width: u32 },
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

  #[derive(Debug, Clone, Default)]
  pub struct PanelPositions {
    entries: HashMap<Ustr, PanelEdge>,
  }

  impl PanelPositions {
    pub fn new() -> Self {
      Self::default()
    }

    pub fn set(&mut self, name: impl Into<Ustr>, edge: PanelEdge) {
      self.entries.insert(name.into(), edge);
    }

    pub fn remove(&mut self, name: &str) {
      self.entries.remove(name);
    }

    pub fn get(&self, name: &str) -> Option<&PanelEdge> {
      self.entries.get(name)
    }

    pub fn for_each(&self, mut f: impl FnMut(&Ustr, &PanelEdge)) {
      for (name, edge) in &self.entries {
        f(name, edge);
      }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Ustr, &PanelEdge)> {
      self.entries.iter()
    }
  }
}
