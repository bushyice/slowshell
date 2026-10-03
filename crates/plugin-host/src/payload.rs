use std::sync::{Mutex, OnceLock};

use abi_stable::std_types::{RStr, RString};
use iced::Element;
use iced_exwlshell::reexport::{IcedId, NewLayerShellSettings};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{EventFilter, ItemEffect, ItemMessage},
  types::{PayloadBox as CorePayloadBox, PayloadBuilderArgs, Ustr},
};
use slowshell_desktop::{DesktopItem, UpdateWhen, Visibility};
use slowshell_plugin_abi::{
  Args, Context, PayloadBox, PayloadObjectBox, PayloadParserBox, ROption,
};

use crate::{
  host::host,
  state::{
    PluginScope, ScopedConfig, compositor_scope, notifications_scope, store_scope,
    with_plugin_calls,
  },
};

pub(crate) struct PayloadEntry {
  pub command: Ustr,
  pub plugin_id: String,
  pub inner: PayloadObjectBox,
}

pub(crate) fn payloads() -> &'static Mutex<Vec<PayloadEntry>> {
  static PAYLOADS: OnceLock<Mutex<Vec<PayloadEntry>>> = OnceLock::new();
  PAYLOADS.get_or_init(|| Mutex::new(Vec::new()))
}

pub(crate) struct PayloadParserEntry {
  pub commands: Vec<String>,
  pub plugin_id: String,
  pub parser: PayloadParserBox,
}

pub(crate) fn parsers() -> &'static Mutex<Vec<PayloadParserEntry>> {
  static PARSERS: OnceLock<Mutex<Vec<PayloadParserEntry>>> = OnceLock::new();
  PARSERS.get_or_init(|| Mutex::new(Vec::new()))
}

pub(crate) fn register_parser(plugin_id: String, commands: Vec<String>, parser: PayloadParserBox) {
  parsers().lock().unwrap().push(PayloadParserEntry {
    commands,
    plugin_id,
    parser,
  });
}

fn matches(commands: &[String], command: &str) -> bool {
  commands.iter().any(|candidate| {
    candidate == command || (candidate.ends_with('.') && command.starts_with(candidate.as_str()))
  })
}

pub(crate) fn plugin_payload_build(
  command: &str,
  args: PayloadBuilderArgs,
) -> Option<CorePayloadBox> {
  let abi_args = Args::new(
    args
      .as_map()
      .into_iter()
      .map(|(key, value)| (RString::from(key), RString::from(value))),
  );

  let entries = std::mem::take(&mut *parsers().lock().unwrap());
  let mut parsed = None;
  for entry in &entries {
    if matches(&entry.commands, command) {
      let _plugin_scope = PluginScope::enter(&entry.plugin_id);
      parsed = entry
        .parser
        .parse(RStr::from_str(command), &abi_args)
        .into_option();
      if parsed.is_some() {
        break;
      }
    }
  }

  {
    let mut guard = parsers().lock().unwrap();
    let mut newly_registered = std::mem::take(&mut *guard);
    newly_registered.extend(entries);
    *guard = newly_registered;
  }

  let boxed = parsed.unwrap_or_else(|| PayloadBox::new(abi_args));
  Some(CorePayloadBox::new(boxed))
}

pub(crate) fn dispatch_payload(name: &Ustr, payload: Option<&CorePayloadBox>) {
  let mut payloads = payloads().lock().unwrap();
  let Some(entry) = payloads.iter_mut().find(|entry| &entry.command == name) else {
    return;
  };

  let _plugin_scope = PluginScope::enter(&entry.plugin_id);
  let ctx = Context::new(host());

  let abi_payload = payload.and_then(|payload| payload.as_this::<PayloadBox>());
  let direct_args = payload.and_then(|payload| payload.as_this::<Args>());
  let args = direct_args
    .cloned()
    .or_else(|| {
      abi_payload
        .and_then(|payload| payload.as_this::<Args>())
        .cloned()
    })
    .unwrap_or_default();
  let payload_ref = abi_payload.map(ROption::RSome).unwrap_or(ROption::RNone);

  entry.inner.invoke(&ctx, &args, payload_ref);
}

pub(crate) struct PluginDispatcher;

impl DesktopItem for PluginDispatcher {
  fn id(&self) -> &str {
    "plugin-dispatcher"
  }

  fn layer(&self, _config: &Config, _monitor: &str) -> NewLayerShellSettings {
    Default::default()
  }

  fn visibility(&self) -> Visibility {
    Visibility::Transient
  }

  fn update_strategy(&self) -> UpdateWhen {
    UpdateWhen::OnDemand
  }

  fn init_events(&self) -> Vec<EventFilter> {
    payloads()
      .lock()
      .unwrap()
      .iter()
      .map(|entry| EventFilter::Payload(entry.command.clone()))
      .collect()
  }

  fn update(
    &mut self,
    config: &Config,
    store: &mut Store,
    event: &ListenerAction,
  ) -> miette::Result<ItemEffect> {
    let _config_scope = ScopedConfig::current(config);
    let _compositor_scope = compositor_scope(store);
    let _notifications_scope = notifications_scope(store);
    let _store_scope = store_scope(store);

    if let ListenerAction::Payload { name, payload } = event {
      let (_, calls) = with_plugin_calls(|| dispatch_payload(name, payload.as_ref()));
      if calls.redraw {
        return Ok(ItemEffect::Redraw);
      }
    }

    Ok(ItemEffect::None)
  }

  fn view<'a>(
    &'a self,
    _config: &'a Config,
    _store: &'a Store,
    _id: IcedId,
    _monitor: &str,
  ) -> Element<'a, ItemMessage> {
    iced::widget::space().into()
  }
}
