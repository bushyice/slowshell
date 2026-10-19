#[cfg(feature = "fallback")]
pub mod fallback;
#[cfg(feature = "hyprland")]
pub mod hyprland;
#[cfg(feature = "niri")]
pub mod niri;
#[cfg(feature = "sway")]
pub mod sway;

use slowshell_config::Config;
use slowshell_core::{
  Monitor, Store, Window, Workspace,
  listeners::{CompositorEvent, Listeners},
  types::{PayloadBox, Ustr, Void},
};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum CompositorCommand {
  FocusWorkspace(i32),
  FocusWindow(u64),
  CloseWindow(u64),
  KillWindow(u64),
  FocusMonitor(String),
  Custom(PayloadBox),
}

#[derive(Debug, Default)]
pub struct CompositorState {
  pub monitors: HashMap<Ustr, Monitor>,
  pub active_window: Option<Window>,
  pub active_windows: Vec<Window>,
  pub workspaces: Vec<Workspace>,
  pub overview_active: bool,
}

pub trait Compositor: Send + Sync {
  fn state(&self) -> &CompositorState;
  fn send_cmd(&mut self, cmd: CompositorCommand) -> miette::Result<Void>;

  fn initialize(&mut self, _config: &Config, _listeners: &mut Listeners) -> miette::Result<Void> {
    Ok(Void)
  }

  fn is_active(&self, config: &Config) -> bool;

  fn update_state(&mut self, _store: Option<&Store>) -> miette::Result<CompositorEvent> {
    Ok(CompositorEvent::ALL)
  }
}

pub type CompositorFactory = Box<dyn Fn() -> Box<dyn Compositor> + Send + Sync>;

pub struct CompositorRegistration {
  pub name: Ustr,
  pub factory: CompositorFactory,
}

impl CompositorRegistration {
  pub fn new(name: impl Into<Ustr>, factory: CompositorFactory) -> Self {
    Self {
      name: name.into(),
      factory,
    }
  }
}

#[derive(Default)]
pub struct CompositorStore {
  inner: Vec<(Ustr, Box<dyn Compositor>)>,
  active: Ustr,
}

impl CompositorStore {
  pub fn new() -> Self {
    #[allow(unused_mut)]
    let mut comps = Self::default();

    #[cfg(feature = "niri")]
    comps
      .inner
      .push(("niri".into(), Box::new(niri::NiriCompositor::new())));

    #[cfg(feature = "sway")]
    comps
      .inner
      .push(("sway".into(), Box::new(sway::WlrCompositor::new())));

    #[cfg(feature = "hyprland")]
    comps.inner.push((
      "hyprland".into(),
      Box::new(hyprland::HyprlandCompositor::new()),
    ));

    #[cfg(feature = "fallback")]
    comps.inner.push((
      "fallback".into(),
      Box::new(fallback::FallbackCompositor::new()),
    ));

    comps
  }

  pub fn register(&mut self, registration: CompositorRegistration) {
    let entry = (registration.name, (registration.factory)());
    if let Some(existing) = self.inner.iter_mut().find(|(name, _)| *name == entry.0) {
      existing.1 = entry.1;
    } else {
      self.inner.push(entry);
    }
  }

  pub fn initialize(&mut self, config: &Config, listeners: &mut Listeners) -> miette::Result<Void> {
    self.detect_compositor(config, listeners)
  }

  pub fn detect_compositor(
    &mut self,
    config: &Config,
    listeners: &mut Listeners,
  ) -> miette::Result<Void> {
    for (name, compositor) in &mut self.inner {
      if compositor.is_active(config) {
        compositor.initialize(config, listeners)?;
        self.active = name.clone();
        return Ok(Void);
      }
    }

    Err(miette::miette!("Compositor was not detected"))
  }

  pub fn active(&self) -> &Ustr {
    &self.active
  }

  pub fn state(&self) -> miette::Result<&CompositorState> {
    self
      .inner
      .iter()
      .find(|(name, _)| *name == self.active)
      .ok_or_else(|| miette::miette!("Current compositor not found."))
      .map(|(_, compositor)| compositor.state())
  }

  pub fn update_state(&mut self, store: Option<&Store>) -> miette::Result<CompositorEvent> {
    let Some((_, compositor)) = self.inner.iter_mut().find(|(name, _)| *name == self.active) else {
      return Err(miette::miette!("Current compositor not found."));
    };

    compositor.update_state(store)
  }

  pub fn send_cmd(&mut self, cmd: CompositorCommand) -> miette::Result<Void> {
    let Some((_, compositor)) = self.inner.iter_mut().find(|(name, _)| *name == self.active) else {
      return Err(miette::miette!("Current compositor not found."));
    };

    compositor.send_cmd(cmd)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  struct FakeCompositor {
    state: CompositorState,
    active: bool,
  }

  impl Compositor for FakeCompositor {
    fn state(&self) -> &CompositorState {
      &self.state
    }

    fn send_cmd(&mut self, _cmd: CompositorCommand) -> miette::Result<Void> {
      Ok(Void)
    }

    fn is_active(&self, _config: &Config) -> bool {
      self.active
    }

    fn update_state(&mut self, _store: Option<&Store>) -> miette::Result<CompositorEvent> {
      Ok(CompositorEvent::ALL)
    }
  }

  #[test]
  fn plugin_compositors_are_registered_after_builtins() {
    let mut store = CompositorStore::new();
    let builtin_count = store.inner.len();

    store.register(CompositorRegistration::new(
      "plugin",
      Box::new(|| {
        Box::new(FakeCompositor {
          state: CompositorState::default(),
          active: true,
        })
      }),
    ));

    assert_eq!(store.inner.len(), builtin_count + 1);
    assert_eq!(
      store.inner.last().map(|(name, _)| name.to_string()),
      Some("plugin".to_owned())
    );
    if builtin_count > 0 {
      assert_ne!(
        store.inner.first().map(|(name, _)| name.to_string()),
        Some("plugin".to_owned())
      );
    }
  }
}
