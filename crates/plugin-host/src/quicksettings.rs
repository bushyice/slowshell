use std::sync::{Arc, Mutex, OnceLock};

use slowshell_commons::quicksettings::{
  QuickSettingsProvider, QuickSettingsTile, TileAction, ensure_registry,
};
use slowshell_config::Config;
use slowshell_core::Store;
use slowshell_plugin_abi::{
  Context, QuickSettingsObjectBox, QuickSettingsTile as AbiQuickSettingsTile, RStr,
  TileAction as AbiTileAction,
};

use crate::{
  host::host,
  state::{PluginScope, compositor_scope, notifications_scope, store_scope},
};

pub(crate) struct PluginQuickSettingsProvider {
  name: String,
  plugin_id: String,
  inner: QuickSettingsObjectBox,
}

unsafe impl Sync for PluginQuickSettingsProvider {}

impl PluginQuickSettingsProvider {
  pub(crate) fn new(name: String, plugin_id: String, inner: QuickSettingsObjectBox) -> Self {
    Self {
      name,
      plugin_id,
      inner,
    }
  }
}

impl QuickSettingsProvider for PluginQuickSettingsProvider {
  fn id(&self) -> &str {
    &self.name
  }

  fn tiles(&self, _config: &Config, store: &Store) -> Vec<QuickSettingsTile> {
    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope(store);

    let ctx = Context::new(host());
    self.inner.tiles(&ctx).iter().map(tile_from_abi).collect()
  }

  fn children(&self, _config: &Config, store: &Store, id: &str) -> Vec<QuickSettingsTile> {
    let _plugin_scope = PluginScope::enter(&self.plugin_id);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope(store);

    let ctx = Context::new(host());
    self
      .inner
      .children(&ctx, RStr::from_str(id))
      .iter()
      .map(tile_from_abi)
      .collect()
  }
}

pub(crate) fn providers() -> &'static Mutex<Vec<PluginQuickSettingsProvider>> {
  static PROVIDERS: OnceLock<Mutex<Vec<PluginQuickSettingsProvider>>> = OnceLock::new();
  PROVIDERS.get_or_init(|| Mutex::new(Vec::new()))
}

pub(crate) fn install(store: &mut Store) {
  let providers = std::mem::take(&mut *providers().lock().unwrap());
  let registry = ensure_registry(store);

  for provider in providers {
    registry.register(Arc::new(provider));
  }
}

fn tile_from_abi(tile: &AbiQuickSettingsTile) -> QuickSettingsTile {
  let on_click = match &tile.on_click {
    AbiTileAction::None => TileAction::None,
    AbiTileAction::Command(command) => TileAction::Command(command.to_string()),
    AbiTileAction::Commands(commands) => {
      TileAction::Commands(commands.iter().map(|command| command.to_string()).collect())
    }
    AbiTileAction::Menu(menu) => TileAction::Menu(menu.to_string()),
    AbiTileAction::Expand => TileAction::Expand,
  };

  QuickSettingsTile {
    id: tile.id.to_string(),
    icon: tile.icon.to_string(),
    title: tile.title.to_string(),
    status: tile.status.to_string(),
    detail: (!tile.detail.is_empty()).then(|| tile.detail.to_string()),
    active: tile.active,
    accent: (!tile.accent.is_empty()).then(|| tile.accent.to_string()),
    on_click,
    order: tile.order,
  }
}
