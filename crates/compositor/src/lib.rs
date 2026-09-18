#[cfg(feature = "niri")]
pub mod niri;
#[cfg(feature = "sway")]
pub mod sway;

use futures_channel::mpsc::UnboundedSender;
use slowshell_config::Config;
use slowshell_core::{
  Monitor, Store, Window, Workspace,
  listeners::Listeners,
  message::Message,
  types::{Ustr, Void},
};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum CompositorCommand {
  FocusWorkspace(i32),
  FocusWindow(u64),
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

  /// Please make the `CompositorState` here
  fn initialize(&mut self, _config: &Config, _listeners: &mut Listeners) -> miette::Result<Void> {
    Ok(Void)
  }
  fn is_active(&self, config: &Config) -> bool;

  fn update_state(
    &mut self,
    _store: Option<&Store>,
    _tx: UnboundedSender<Message>,
  ) -> miette::Result<Void> {
    Ok(())
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
  inner: HashMap<Ustr, Box<dyn Compositor>>,
  active: Ustr,
}

impl CompositorStore {
  pub fn new() -> Self {
    #[allow(unused_mut)]
    let mut comps = Self::default();

    #[cfg(feature = "niri")]
    comps
      .inner
      .insert("niri".into(), Box::new(niri::NiriCompositor::new()));

    #[cfg(feature = "sway")]
    comps
      .inner
      .insert("sway".into(), Box::new(sway::WlrCompositor::new()));

    comps
  }

  pub fn register(&mut self, registration: CompositorRegistration) {
    self
      .inner
      .insert(registration.name, (registration.factory)());
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

  pub fn state(&self) -> miette::Result<&CompositorState> {
    self
      .inner
      .get(&self.active)
      .ok_or_else(|| miette::miette!("Current compositor not found."))
      .map(|c| c.state())
  }

  pub fn update_state(
    &mut self,
    store: Option<&Store>,
    tx: UnboundedSender<Message>,
  ) -> miette::Result<Void> {
    let Some(compositor) = self.inner.get_mut(&self.active) else {
      return Err(miette::miette!("Current compositor not found."));
    };

    compositor.update_state(store, tx)?;

    Ok(())
  }

  pub fn send_cmd(&mut self, cmd: CompositorCommand) -> miette::Result<Void> {
    let Some(compositor) = self.inner.get_mut(&self.active) else {
      return Err(miette::miette!("Current compositor not found."));
    };

    compositor.send_cmd(cmd)
  }
}
