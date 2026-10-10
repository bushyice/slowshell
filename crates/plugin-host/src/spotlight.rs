use std::sync::{Mutex, OnceLock};

use slowshell_plugin_abi::SpotlightObjectBox;

#[cfg(feature = "spotlight")]
use std::sync::Arc;

#[cfg(feature = "spotlight")]
use slowshell_core::Store;
#[cfg(feature = "spotlight")]
use slowshell_plugin_abi::{Context, ROption, RStr, RString, display_style};
#[cfg(feature = "spotlight")]
use slowshell_spotlight::{DisplayStyle, SpotlightAction, SpotlightActionDef, SpotlightItem};

#[cfg(feature = "spotlight")]
use crate::{
  host::host,
  state::{PluginScope, compositor_scope, notifications_scope, store_scope},
};

pub(crate) struct SpotlightEntry {
  pub name: String,
  pub plugin_id: String,
  pub inner: SpotlightObjectBox,
}

pub(crate) fn spotlights() -> &'static Mutex<Vec<SpotlightEntry>> {
  static SPOTLIGHTS: OnceLock<Mutex<Vec<SpotlightEntry>>> = OnceLock::new();
  SPOTLIGHTS.get_or_init(|| Mutex::new(Vec::new()))
}

#[cfg(feature = "spotlight")]
pub(crate) fn display_styles(bits: u32) -> &'static [DisplayStyle] {
  let mut styles = Vec::new();

  if bits & display_style::LIST != 0 {
    styles.push(DisplayStyle::List);
  }
  if bits & display_style::GRID != 0 {
    styles.push(DisplayStyle::Grid);
  }
  if bits & display_style::IMAGE_LIST != 0 {
    styles.push(DisplayStyle::ImageList);
  }
  if styles.is_empty() {
    styles.push(DisplayStyle::List);
  }

  Box::leak(styles.into_boxed_slice())
}

#[cfg(feature = "spotlight")]
pub(crate) fn generate(entry_index: usize, query: &str, store: &Store) -> Vec<SpotlightItem> {
  let mut spotlights = spotlights().lock().unwrap();
  let Some(entry) = spotlights.get_mut(entry_index) else {
    return Vec::new();
  };

  let _plugin_scope = PluginScope::enter(&entry.plugin_id);
  let _compositor_scope = compositor_scope(store);
  let _notifications_scope = notifications_scope(store);
  let _store_scope = store_scope(store);

  let ctx = Context::new(host());
  let results = entry.inner.generate(&ctx, RStr::from_str(query));

  results
    .iter()
    .enumerate()
    .map(|(item_index, item)| {
      let icon = optional_string(&item.icon).filter(|icon| !icon.is_empty());
      let title = item.title.to_string();
      let subtitle = optional_string(&item.subtitle).filter(|subtitle| !subtitle.is_empty());
      let image = optional_string(&item.image)
        .filter(|image| !image.is_empty())
        .map(std::path::PathBuf::from);
      let action_label = optional_string(&item.action_label).filter(|label| !label.is_empty());
      let tags = if item.tags.is_empty() {
        None
      } else {
        Some(item.tags.iter().map(|tag| tag.to_string()).collect())
      };

      let action = SpotlightAction::Custom(Arc::new(move || activate(entry_index, item_index)));

      SpotlightItem {
        image,
        image_bytes: None,
        image_icon: icon,
        title,
        subtitle,
        subtext: None,
        tags,
        actions: vec![SpotlightActionDef {
          title: action_label,
          action,
        }],
      }
    })
    .collect()
}

#[cfg(feature = "spotlight")]
pub(crate) fn trigger_check(entry_index: usize, query: &str) -> bool {
  let spotlights = spotlights().lock().unwrap();
  let Some(entry) = spotlights.get(entry_index) else {
    return false;
  };

  let _plugin_scope = PluginScope::enter(&entry.plugin_id);
  let ctx = Context::new(host());
  entry.inner.trigger_check(&ctx, RStr::from_str(query))
}

#[cfg(feature = "spotlight")]
#[allow(dead_code)]
pub(crate) fn make_trigger_fn(entry_index: usize) -> impl Fn(&str) -> bool + Send + Sync + 'static {
  move |query| trigger_check(entry_index, query)
}

#[cfg(feature = "spotlight")]
fn activate(entry_index: usize, item_index: usize) {
  let mut spotlights = spotlights().lock().unwrap();
  let Some(entry) = spotlights.get_mut(entry_index) else {
    return;
  };

  let _plugin_scope = PluginScope::enter(&entry.plugin_id);
  let ctx = Context::new(host());
  entry.inner.activate(&ctx, item_index);
}

#[cfg(feature = "spotlight")]
fn optional_string(value: &ROption<RString>) -> Option<String> {
  value.as_ref().into_option().map(|value| value.to_string())
}
