use slowshell_compositor::{Compositor, CompositorCommand, CompositorState};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::{CompositorEvent, Listeners},
  types::{PayloadBox as CorePayloadBox, Void},
};
use slowshell_plugin_abi::{
  Args, CompositorCommand as AbiCompositorCommand, CompositorObjectBox, Context, PayloadBox,
  RString,
};

use crate::{
  host::host,
  state::{PluginScope, ScopedCompositorState, ScopedConfig, ScopedListeners, store_scope},
};

fn abi_payload(payload: &CorePayloadBox) -> PayloadBox {
  payload
    .as_this::<PayloadBox>()
    .cloned()
    .or_else(|| {
      payload
        .as_this::<String>()
        .map(|text| PayloadBox::new(RString::from(text.as_str())))
    })
    .unwrap_or_else(|| PayloadBox::new(Args::default()))
}

fn abi_command(cmd: CompositorCommand) -> AbiCompositorCommand {
  match cmd {
    CompositorCommand::FocusWorkspace(index) => AbiCompositorCommand::FocusWorkspace(index),
    CompositorCommand::FocusWindow(id) => AbiCompositorCommand::FocusWindow(id),
    CompositorCommand::CloseWindow(id) => AbiCompositorCommand::CloseWindow(id),
    CompositorCommand::KillWindow(id) => AbiCompositorCommand::KillWindow(id),
    CompositorCommand::FocusMonitor(name) => {
      AbiCompositorCommand::FocusMonitor(RString::from(name))
    }
    CompositorCommand::Custom(payload) => AbiCompositorCommand::Custom(abi_payload(&payload)),
  }
}

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
    let command = abi_command(cmd);

    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _state_scope = ScopedCompositorState::current(&mut self.state);
    let ctx = Context::new(host());
    self.inner.send_command(&ctx, &command);

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
