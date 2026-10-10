use slowshell_compositor::{Compositor, CompositorCommand, CompositorState};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::{CompositorEvent, Listeners},
  types::Void,
};
use slowshell_plugin_abi::{CompositorObjectBox, Context};

use crate::{
  host::host,
  state::{PluginScope, ScopedCompositorState, ScopedConfig, ScopedListeners, store_scope},
};

const COMPOSITOR_FOCUS_WORKSPACE: u32 = 0;
const COMPOSITOR_FOCUS_WINDOW: u32 = 1;

pub(crate) struct PluginCompositor {
  plugin_id: String,
  inner: CompositorObjectBox,
  state: CompositorState,
}

unsafe impl Sync for PluginCompositor {}

impl PluginCompositor {
  pub(crate) fn new(plugin_id: String, inner: CompositorObjectBox) -> Self {
    Self {
      plugin_id,
      inner,
      state: CompositorState::default(),
    }
  }
}

impl Compositor for PluginCompositor {
  fn state(&self) -> &CompositorState {
    &self.state
  }

  fn send_cmd(&mut self, cmd: CompositorCommand) -> miette::Result<Void> {
    let (command, arg) = match cmd {
      CompositorCommand::FocusWorkspace(index) => (COMPOSITOR_FOCUS_WORKSPACE, index),
      CompositorCommand::FocusWindow(id) => (COMPOSITOR_FOCUS_WINDOW, id as i32),
    };

    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _state_scope = ScopedCompositorState::current(&mut self.state);
    let ctx = Context::new(host());
    self.inner.send_command(&ctx, command, arg);

    Ok(Void)
  }

  fn initialize(&mut self, config: &Config, listeners: &mut Listeners) -> miette::Result<Void> {
    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _config_scope = ScopedConfig::current(config);
    let _listeners_scope = ScopedListeners::current(listeners);
    let _state_scope = ScopedCompositorState::current(&mut self.state);

    let ctx = Context::new(host());
    self.inner.initialize(&ctx);
    Ok(Void)
  }

  fn is_active(&self, config: &Config) -> bool {
    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _config_scope = ScopedConfig::current(config);
    self.inner.is_active()
  }

  fn update_state(&mut self, store: Option<&Store>) -> miette::Result<CompositorEvent> {
    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _store_scope = store.map(store_scope);
    let _state_scope = ScopedCompositorState::current(&mut self.state);

    let ctx = Context::new(host());
    self.inner.update_state(&ctx);

    Ok(CompositorEvent::ALL)
  }
}
