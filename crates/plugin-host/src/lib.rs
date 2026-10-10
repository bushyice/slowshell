mod config;
mod events;
mod host;
mod services;
mod state;
mod store;

mod compositor;
mod desktop;
mod payload;
mod quicksettings;
mod render;
mod renderable;

#[cfg(feature = "spotlight")]
mod spotlight;

#[cfg(feature = "panels")]
mod component;

use std::{
  collections::{BTreeSet, HashMap},
  path::{Path, PathBuf},
  sync::{Mutex, OnceLock},
};

use abi_stable::library::{RawLibrary, lib_header_from_raw_library};
use slowshell_config::{Config, find_node, node_children, node_name};
use slowshell_core::Store;
use slowshell_desktop::DesktopItem;
use slowshell_plugin_abi::{
  ComponentFactoryBox, CompositorFactoryBox, Context, DesktopItemFactoryBox, PluginMod_Ref,
  RenderableFactoryBox,
};
use slowshell_registry::{GlobalRegistry, ResourceRegistration};

use crate::state::{Pending, PluginScope, contribute, plugin_ids, plugin_infos, plugin_log};

pub use crate::config::PluginConfigs;
pub use crate::state::{PluginContributions, PluginInfo, set_silent};

#[derive(Default, Clone)]
pub struct PluginSelection {
  pub enabled: Vec<String>,
  pub disabled: Vec<String>,
}

impl PluginSelection {
  pub fn from_path(path: Option<&Path>) -> Self {
    let Some(path) = path else {
      return Self::default();
    };
    let Ok(content) = std::fs::read_to_string(path) else {
      return Self::default();
    };
    let Ok(document) = content.parse::<kdl::KdlDocument>() else {
      return Self::default();
    };
    let Some(node) = find_node(document.nodes(), "plugins") else {
      return Self::default();
    };

    let mut selection = Self::default();
    for child in node_children(node) {
      let values: Vec<String> = child
        .entries()
        .iter()
        .filter_map(|entry| entry.value().as_string().map(str::to_owned))
        .collect();

      match node_name(child) {
        "enabled" => selection.enabled.extend(values),
        "disabled" => selection.disabled.extend(values),
        _ => {}
      }
    }
    selection
  }

  fn allows(&self, id: &str) -> bool {
    if self.disabled.iter().any(|disabled| disabled == id) {
      return false;
    }
    if self.enabled.is_empty() {
      return true;
    }
    self.enabled.iter().any(|enabled| enabled == id)
  }
}

pub struct PluginHost;

impl PluginHost {
  pub fn load(registry: &mut GlobalRegistry, selection: &PluginSelection) -> Self {
    let candidates = discover(selection);
    let metas: Vec<CandidateMeta> = candidates
      .iter()
      .map(|candidate| candidate.meta.clone())
      .collect();
    let plan = resolve(&metas);

    for (index, report) in &plan.skipped {
      let id = &candidates[*index].meta.id;
      state::record_plugin_error(id, report.code, &report.reason);
    }

    for index in &plan.order {
      let candidate = &candidates[*index];
      register_one(candidate, registry);
      plugin_log(format_args!("[plugin] loaded {}", candidate.path.display()));
    }

    let has_payloads = payload::payloads()
      .lock()
      .map(|payloads| !payloads.is_empty())
      .unwrap_or(false);
    if has_payloads {
      registry.include_in("app", ResourceRegistration::Item(plugin_dispatcher_item));
    }

    let has_renderables = renderable_entries()
      .lock()
      .map(|renderables| !renderables.is_empty())
      .unwrap_or(false);
    if has_renderables {
      registry.include_in(
        "app",
        ResourceRegistration::CustomConfig(register_plugin_renderables),
      );
    }

    #[cfg(feature = "spotlight")]
    {
      let has_spotlights = spotlight::spotlights()
        .lock()
        .map(|spotlights| !spotlights.is_empty())
        .unwrap_or(false);
      if has_spotlights {
        registry.include_in(
          "app",
          ResourceRegistration::CustomConfig(register_plugin_spotlights),
        );
      }
    }

    let has_desktop_items = desktop_entries()
      .lock()
      .map(|items| !items.is_empty())
      .unwrap_or(false);
    if has_desktop_items {
      registry.include_in("app", ResourceRegistration::Item(plugin_desktop_items));
    }

    let has_desktop_deployers = desktop_deployer_entries()
      .lock()
      .map(|items| !items.is_empty())
      .unwrap_or(false);
    if has_desktop_deployers {
      registry.include_in(
        "app",
        ResourceRegistration::Deployable(plugin_desktop_deployers),
      );
    }

    let has_quick_settings = quicksettings::providers()
      .lock()
      .map(|providers| !providers.is_empty())
      .unwrap_or(false);
    if has_quick_settings {
      registry.include_in("app", ResourceRegistration::Custom(quicksettings::install));
    }

    let has_plugins = plugin_ids()
      .lock()
      .map(|ids| !ids.is_empty())
      .unwrap_or(false);
    if has_plugins {
      registry.include_in(
        "config",
        ResourceRegistration::Unknown(Box::new(slowshell_config::ConfigParser {
          type_id: std::any::TypeId::of::<PluginConfigs>(),
          de: config::parse_plugin_configs,
        })),
      );
    }

    let payload_count = payload::payloads().lock().map(|p| p.len()).unwrap_or(0);
    let renderable_count = renderable_entries().lock().map(|r| r.len()).unwrap_or(0);
    let desktop_count = desktop_entries().lock().map(|d| d.len()).unwrap_or(0);
    let deployer_count = desktop_deployer_entries()
      .lock()
      .map(|d| d.len())
      .unwrap_or(0);
    if payload_count + renderable_count + desktop_count + deployer_count > 0 {
      plugin_log(format_args!(
        "[plugin] registered {payload_count} payload(s), {renderable_count} renderable(s), {desktop_count} desktop item(s), {deployer_count} desktop deployer(s)"
      ));
    }

    Self
  }
}

pub fn registered_surface_counts() -> (usize, usize, usize) {
  let payloads = payload::payloads().lock().map(|p| p.len()).unwrap_or(0);
  let renderables = renderable_entries().lock().map(|r| r.len()).unwrap_or(0);
  let desktop_items = desktop_entries().lock().map(|d| d.len()).unwrap_or(0);
  (payloads, renderables, desktop_items)
}

pub fn registered_renderable_names() -> Vec<String> {
  renderable_names()
}

pub fn renderable_names() -> Vec<String> {
  renderable_entries()
    .lock()
    .map(|entries| entries.iter().map(|entry| entry.name.to_string()).collect())
    .unwrap_or_default()
}

pub fn spotlight_names() -> Vec<String> {
  #[cfg(feature = "spotlight")]
  {
    spotlight::spotlights()
      .lock()
      .map(|entries| entries.iter().map(|entry| entry.name.clone()).collect())
      .unwrap_or_default()
  }

  #[cfg(not(feature = "spotlight"))]
  {
    Vec::new()
  }
}

pub fn loaded_plugins() -> Vec<PluginInfo> {
  plugin_infos().lock().unwrap().clone()
}

pub fn last_plugin_error(plugin_id: &str) -> Option<(u32, String)> {
  crate::state::plugin_errors()
    .lock()
    .unwrap()
    .get(plugin_id)
    .cloned()
}

pub fn contributions(plugin_id: &str) -> PluginContributions {
  crate::state::plugin_contributions()
    .lock()
    .unwrap()
    .get(plugin_id)
    .cloned()
    .unwrap_or_default()
}

#[derive(Clone, Debug)]
struct Requirement {
  id: String,
  req: semver::VersionReq,
  raw: String,
}

#[derive(Clone, Debug)]
struct CandidateMeta {
  id: String,
  version: String,
  parsed_version: Option<semver::Version>,
  requires: Vec<Requirement>,
}

struct Candidate {
  path: PathBuf,
  root: PluginMod_Ref,
  meta: CandidateMeta,
}

#[derive(Debug)]
struct SkipReport {
  code: u32,
  reason: String,
}

struct PluginPlan {
  order: Vec<usize>,
  skipped: Vec<(usize, SkipReport)>,
}

const ERR_DEPENDENCY_MISSING: u32 = 0x1001;
const ERR_DEPENDENCY_VERSION: u32 = 0x1002;
const ERR_DEPENDENCY_CYCLE: u32 = 0x1003;

fn discover(selection: &PluginSelection) -> Vec<Candidate> {
  let mut by_id: HashMap<String, Candidate> = HashMap::new();

  for dir in plugin_dirs() {
    let Ok(entries) = std::fs::read_dir(&dir) else {
      continue;
    };

    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();

    for path in paths {
      if path.extension().and_then(|ext| ext.to_str()) != Some("so") {
        continue;
      }

      match read_candidate(&path) {
        Ok(candidate) if !selection.allows(&candidate.meta.id) => {
          plugin_log(format_args!("[plugin] {} disabled", candidate.meta.id));
        }
        Ok(candidate) => {
          if let Some(existing) = by_id.get(&candidate.meta.id) {
            plugin_log(format_args!(
              "[plugin] {} from {} shadowed by {}",
              candidate.meta.id,
              path.display(),
              existing.path.display()
            ));
          } else {
            by_id.insert(candidate.meta.id.clone(), candidate);
          }
        }
        Err(e) => plugin_log(format_args!(
          "[plugin] failed to load {}: {e}",
          path.display()
        )),
      }
    }
  }

  let mut candidates: Vec<Candidate> = by_id.into_values().collect();
  candidates.sort_by(|a, b| a.meta.id.cmp(&b.meta.id));
  candidates
}

fn read_candidate(path: &Path) -> miette::Result<Candidate> {
  let raw_library = RawLibrary::load_at(path).map_err(|e| miette::miette!("{e}"))?;
  let header =
    unsafe { lib_header_from_raw_library(&raw_library) }.map_err(|e| miette::miette!("{e}"))?;
  header.ensure_layout::<PluginMod_Ref>().map_err(|e| {
    miette::miette!("{e} (the plugin may need to be rebuilt against the current slowshell ABI)")
  })?;

  let library = unsafe { header.init_root_module_with_unchecked_layout::<PluginMod_Ref>() }
    .map_err(|e| miette::miette!("{e}"))?;

  std::mem::forget(raw_library);

  let id = library.id().to_string();
  let version = library.version().to_string();
  let parsed_version = semver::Version::parse(&version).ok();

  let mut requires = Vec::new();
  for dependency in library.requires().iter() {
    let dep_id = dependency.id.to_string();
    let raw = dependency.version_req.to_string();
    let req = semver::VersionReq::parse(&raw).map_err(|e| {
      miette::miette!("invalid version requirement `{raw}` for dependency `{dep_id}`: {e}")
    })?;
    requires.push(Requirement {
      id: dep_id,
      req,
      raw,
    });
  }

  Ok(Candidate {
    path: path.to_path_buf(),
    root: library,
    meta: CandidateMeta {
      id,
      version,
      parsed_version,
      requires,
    },
  })
}

fn resolve(metas: &[CandidateMeta]) -> PluginPlan {
  let count = metas.len();
  let mut by_id: HashMap<&str, usize> = HashMap::with_capacity(count);
  for (index, meta) in metas.iter().enumerate() {
    by_id.insert(meta.id.as_str(), index);
  }

  let mut skipped: Vec<Option<SkipReport>> = (0..count).map(|_| None).collect();

  for (index, meta) in metas.iter().enumerate() {
    for dependency in &meta.requires {
      let Some(&dep_index) = by_id.get(dependency.id.as_str()) else {
        set_skip(
          &mut skipped,
          index,
          ERR_DEPENDENCY_MISSING,
          format!(
            "requires `{}` {}, which is not installed or is disabled",
            dependency.id, dependency.raw
          ),
        );
        break;
      };

      if dep_index == index {
        set_skip(
          &mut skipped,
          index,
          ERR_DEPENDENCY_MISSING,
          format!("depends on itself (`{}`)", dependency.id),
        );
        break;
      }

      match metas[dep_index].parsed_version.as_ref() {
        Some(version) if dependency.req.matches(version) => {}
        Some(version) => {
          set_skip(
            &mut skipped,
            index,
            ERR_DEPENDENCY_VERSION,
            format!(
              "requires `{}` {}, but {} is installed",
              dependency.id, dependency.raw, version
            ),
          );
          break;
        }
        None => {
          set_skip(
            &mut skipped,
            index,
            ERR_DEPENDENCY_VERSION,
            format!(
              "requires `{}` {}, but `{}` has a non-semver version `{}`",
              dependency.id, dependency.raw, dependency.id, metas[dep_index].version
            ),
          );
          break;
        }
      }
    }
  }

  loop {
    let mut changed = false;
    for index in 0..count {
      if skipped[index].is_some() {
        continue;
      }

      let blocked = metas[index].requires.iter().find_map(|dependency| {
        let dep_index = *by_id.get(dependency.id.as_str())?;
        skipped[dep_index].as_ref().map(|_| dependency)
      });

      if let Some(dependency) = blocked {
        skipped[index] = Some(SkipReport {
          code: ERR_DEPENDENCY_MISSING,
          reason: format!(
            "requires `{}` {}, which was skipped",
            dependency.id, dependency.raw
          ),
        });
        changed = true;
      }
    }

    if !changed {
      break;
    }
  }

  let mut indegree = vec![0usize; count];
  let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); count];
  for (index, meta) in metas.iter().enumerate() {
    if skipped[index].is_some() {
      continue;
    }

    for dependency in &meta.requires {
      if let Some(&dep_index) = by_id.get(dependency.id.as_str())
        && skipped[dep_index].is_none()
      {
        indegree[index] += 1;
        dependents[dep_index].push(index);
      }
    }
  }

  let mut ready: BTreeSet<usize> = (0..count)
    .filter(|&index| skipped[index].is_none() && indegree[index] == 0)
    .collect();

  let mut order = Vec::with_capacity(count);
  let mut scheduled = vec![false; count];
  while let Some(index) = ready.pop_first() {
    scheduled[index] = true;
    order.push(index);
    for &dependent in &dependents[index] {
      indegree[dependent] -= 1;
      if indegree[dependent] == 0 {
        ready.insert(dependent);
      }
    }
  }

  let cycle: Vec<usize> = (0..count)
    .filter(|&index| skipped[index].is_none() && !scheduled[index])
    .collect();
  if !cycle.is_empty() {
    let reason = format!(
      "dependency cycle involving: {}",
      cycle
        .iter()
        .map(|&index| metas[index].id.as_str())
        .collect::<Vec<_>>()
        .join(", ")
    );
    for &index in &cycle {
      skipped[index] = Some(SkipReport {
        code: ERR_DEPENDENCY_CYCLE,
        reason: reason.clone(),
      });
    }
  }

  PluginPlan {
    order,
    skipped: skipped
      .into_iter()
      .enumerate()
      .filter_map(|(index, report)| report.map(|report| (index, report)))
      .collect(),
  }
}

fn set_skip(skipped: &mut [Option<SkipReport>], index: usize, code: u32, reason: String) {
  if skipped[index].is_none() {
    skipped[index] = Some(SkipReport { code, reason });
  }
}

fn register_one(candidate: &Candidate, registry: &mut GlobalRegistry) {
  let id = &candidate.meta.id;

  plugin_ids().lock().unwrap().push(id.clone());
  plugin_infos().lock().unwrap().push(PluginInfo {
    id: id.clone(),
    version: candidate.meta.version.clone(),
    requires: candidate
      .meta
      .requires
      .iter()
      .map(|dependency| format!("{} {}", dependency.id, dependency.raw))
      .collect(),
    path: candidate.path.clone(),
  });

  state::PENDING.with(|pending| pending.borrow_mut().clear());

  {
    let _scope = PluginScope::enter(id);
    (candidate.root.register())(host::host());
  }

  drain_pending(registry, id);
}

fn plugin_dirs() -> Vec<PathBuf> {
  let mut dirs = Vec::new();

  if let Ok(paths) = std::env::var("SLOWSHELL_PLUGIN_PATH") {
    dirs.extend(
      paths
        .split(':')
        .filter(|p| !p.is_empty())
        .map(PathBuf::from),
    );
  }

  let data_home = std::env::var_os("XDG_DATA_HOME")
    .map(PathBuf::from)
    .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
  if let Some(data_home) = data_home {
    dirs.push(data_home.join("slowshell/plugins"));
  }

  let data_dirs =
    std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
  dirs.extend(
    data_dirs
      .split(':')
      .filter(|d| !d.is_empty())
      .map(|d| PathBuf::from(d).join("slowshell/plugins")),
  );

  if cfg!(debug_assertions) {
    dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug"));
  }

  dirs
}

fn drain_pending(registry: &mut GlobalRegistry, plugin_id: &str) {
  let pending = state::PENDING.with(|pending| std::mem::take(&mut *pending.borrow_mut()));

  for item in pending {
    match item {
      Pending::Component { name, factory } => {
        register_component(registry, plugin_id, name, factory);
      }
      Pending::Compositor { name, factory } => {
        register_compositor(registry, plugin_id, name, factory);
      }
      Pending::Payload { command, factory } => {
        let ctx = Context::new(host::host());
        let inner = factory.create(&ctx);
        payload::payloads()
          .lock()
          .unwrap()
          .push(payload::PayloadEntry {
            command: command.clone().into(),
            plugin_id: plugin_id.to_owned(),
            inner,
          });
        contribute(plugin_id, |contributions| {
          contributions.payloads.push(command.clone());
        });
        registry.include_in(
          "payload",
          ResourceRegistration::Unknown(Box::new(slowshell_core::types::PayloadBuilder {
            commands: Box::leak(
              vec![Box::leak(command.into_boxed_str()) as &'static str].into_boxed_slice(),
            ),
            build: payload::plugin_payload_build,
          })),
        );
      }
      Pending::Renderable { name, factory } => {
        renderable_entries().lock().unwrap().push(RenderableEntry {
          name: name.clone().into(),
          plugin_id: plugin_id.to_owned(),
          factory,
        });
        contribute(plugin_id, |contributions| {
          contributions.renderables.push(name);
        });
      }
      Pending::DesktopItem { name, factory } => {
        desktop_entries().lock().unwrap().push(DesktopEntry {
          name: name.clone().into(),
          plugin_id: plugin_id.to_owned(),
          factory,
        });
        contribute(plugin_id, |contributions| {
          contributions.desktop_items.push(name);
        });
      }
      Pending::DesktopItemDeployer { name, factory } => {
        desktop_deployer_entries()
          .lock()
          .unwrap()
          .push(DesktopDeployerEntry {
            name: name.clone().into(),
            plugin_id: plugin_id.to_owned(),
            factory,
          });
        contribute(plugin_id, |contributions| {
          contributions.desktop_item_deployers.push(name);
        });
      }
      Pending::PayloadParser { commands, parser } => {
        crate::payload::register_parser(plugin_id.to_owned(), commands.clone(), parser);
        contribute(plugin_id, |contributions| {
          contributions.payload_parsers.push(commands.join(","));
        });

        let commands: Vec<&'static str> = commands
          .iter()
          .map(|command| &*Box::leak(command.clone().into_boxed_str()))
          .collect();
        registry.include_in(
          "payload",
          ResourceRegistration::Unknown(Box::new(slowshell_core::types::PayloadBuilder {
            commands: Box::leak(commands.into_boxed_slice()),
            build: payload::plugin_payload_build,
          })),
        );
      }
      Pending::ConfigParser { name, parser } => {
        contribute(plugin_id, |contributions| {
          contributions.config_parsers.push(name.clone());
        });
        crate::state::plugin_config_parsers().lock().unwrap().push((
          plugin_id.to_owned(),
          name,
          parser,
        ));
      }
      Pending::Spotlight { name, factory } => {
        #[cfg(feature = "spotlight")]
        {
          let ctx = Context::new(host::host());
          let inner = factory.create(&ctx);
          spotlight::spotlights()
            .lock()
            .unwrap()
            .push(spotlight::SpotlightEntry {
              name: name.clone(),
              plugin_id: plugin_id.to_owned(),
              inner,
            });
        }
        #[cfg(not(feature = "spotlight"))]
        let _ = factory;
        contribute(plugin_id, |contributions| {
          contributions.spotlights.push(name);
        });
      }
      Pending::QuickSettings { name, factory } => {
        let ctx = Context::new(host::host());
        let inner = factory.create(&ctx);
        quicksettings::providers().lock().unwrap().push(
          quicksettings::PluginQuickSettingsProvider::new(name, plugin_id.to_owned(), inner),
        );
      }
      Pending::Command {
        name,
        title,
        description,
      } => {
        slowshell_core::commands::register(slowshell_core::commands::CommandEntry {
          name: name.clone().into(),
          title,
          description,
          icon: None,
          action: slowshell_core::listeners::ListenerAction::Named(name.clone().into()),
        });

        contribute(plugin_id, |contributions| {
          contributions.commands.push(name);
        });
      }
    }
  }
}

#[cfg(feature = "panels")]
fn register_component(
  registry: &mut GlobalRegistry,
  plugin_id: &str,
  name: String,
  factory: ComponentFactoryBox,
) {
  use slowshell_components::{Component, ComponentRegistration};

  let owner = plugin_id.to_owned();
  let registration = ComponentRegistration::new(
    name.clone(),
    Box::new(move || {
      let ctx = Context::new(host::host());
      let _scope = PluginScope::enter(&owner);
      let inner = factory.create(&ctx);
      Box::new(component::PluginComponent::new(
        owner.clone(),
        inner,
        factory.hoverable(),
      )) as Box<dyn Component>
    }),
  );

  contribute(plugin_id, |contributions| {
    contributions.components.push(name);
  });

  registry.include_in(
    "components",
    ResourceRegistration::Unknown(Box::new(registration)),
  );
}

#[cfg(not(feature = "panels"))]
fn register_component(
  _registry: &mut GlobalRegistry,
  _plugin_id: &str,
  _name: String,
  _factory: ComponentFactoryBox,
) {
}

fn register_compositor(
  registry: &mut GlobalRegistry,
  plugin_id: &str,
  name: String,
  factory: CompositorFactoryBox,
) {
  use slowshell_compositor::{Compositor, CompositorRegistration};

  let owner = plugin_id.to_owned();
  let registration = CompositorRegistration::new(
    name.clone(),
    Box::new(move || {
      let ctx = Context::new(host::host());
      let _scope = PluginScope::enter(&owner);
      let inner = factory.create(&ctx);
      Box::new(compositor::PluginCompositor::new(owner.clone(), inner)) as Box<dyn Compositor>
    }),
  );

  contribute(plugin_id, |contributions| {
    contributions.compositors.push(name);
  });

  registry.include_in(
    "compositor",
    ResourceRegistration::Unknown(Box::new(registration)),
  );
}

#[cfg_attr(not(feature = "panels"), allow(dead_code))]
struct RenderableEntry {
  name: slowshell_core::types::Ustr,
  plugin_id: String,
  factory: RenderableFactoryBox,
}

fn renderable_entries() -> &'static Mutex<Vec<RenderableEntry>> {
  static ENTRIES: OnceLock<Mutex<Vec<RenderableEntry>>> = OnceLock::new();
  ENTRIES.get_or_init(|| Mutex::new(Vec::new()))
}

#[cfg(feature = "panels")]
fn register_plugin_renderables(_config: &Config, store: &mut Store) {
  use slowshell_plugin_abi::Context;
  use slowshell_widgets::Renderables;

  let Some(table) = store.borrow_mut::<Renderables>() else {
    return;
  };

  let entries = renderable_entries().lock().unwrap();
  for entry in entries.iter() {
    let ctx = Context::new(host::host());
    let _scope = PluginScope::enter(&entry.plugin_id);
    let inner = entry.factory.create(&ctx);
    table.insert(
      entry.name.clone(),
      Box::new(renderable::PluginRenderable::new(
        entry.plugin_id.clone(),
        inner,
      )),
    );
  }
}

#[cfg(not(feature = "panels"))]
fn register_plugin_renderables(_config: &Config, _store: &mut Store) {}

struct DesktopEntry {
  name: slowshell_core::types::Ustr,
  plugin_id: String,
  factory: DesktopItemFactoryBox,
}

fn desktop_entries() -> &'static Mutex<Vec<DesktopEntry>> {
  static ENTRIES: OnceLock<Mutex<Vec<DesktopEntry>>> = OnceLock::new();
  ENTRIES.get_or_init(|| Mutex::new(Vec::new()))
}

fn plugin_desktop_items(
  _config: &Config,
  _store: &Store,
) -> miette::Result<Vec<Box<dyn DesktopItem + Send>>> {
  let entries = desktop_entries().lock().unwrap();
  let items = entries
    .iter()
    .map(|entry| {
      let ctx = Context::new(host::host());
      let _scope = PluginScope::enter(&entry.plugin_id);
      let inner = entry.factory.create(&ctx);
      Box::new(desktop::PluginDesktopItem::new(
        entry.name.clone(),
        entry.plugin_id.clone(),
        inner,
      )) as Box<dyn DesktopItem + Send>
    })
    .collect();
  Ok(items)
}

struct DesktopDeployerEntry {
  name: slowshell_core::types::Ustr,
  plugin_id: String,
  factory: slowshell_plugin_abi::DeployableDesktopItemFactoryBox,
}

fn desktop_deployer_entries() -> &'static Mutex<Vec<DesktopDeployerEntry>> {
  static ENTRIES: OnceLock<Mutex<Vec<DesktopDeployerEntry>>> = OnceLock::new();
  ENTRIES.get_or_init(|| Mutex::new(Vec::new()))
}

fn plugin_desktop_deployers(
  _config: &Config,
  _store: &Store,
) -> miette::Result<Vec<Box<dyn slowshell_desktop::DeployableDesktopItem>>> {
  let entries = desktop_deployer_entries().lock().unwrap();
  let deployers = entries
    .iter()
    .map(|entry| {
      let ctx = Context::new(host::host());
      let _scope = PluginScope::enter(&entry.plugin_id);
      let inner = entry.factory.create(&ctx);
      Box::new(desktop::PluginDeployable::new(
        entry.name.clone(),
        entry.plugin_id.clone(),
        inner,
      )) as Box<dyn slowshell_desktop::DeployableDesktopItem>
    })
    .collect();
  Ok(deployers)
}

#[cfg(feature = "spotlight")]
fn register_plugin_spotlights(_config: &Config, store: &mut Store) {
  use slowshell_spotlight::{SpotlightKind, SpotlightMode, SpotlightModes};

  let Some(modes) = store.borrow_mut::<SpotlightModes>() else {
    return;
  };

  let entries = spotlight::spotlights().lock().unwrap();
  for (index, entry) in entries.iter().enumerate() {
    let display = spotlight::display_styles(entry.inner.display_styles());
    let allows_triggers = entry.inner.allows_triggers();

    modes.register_mode(
      entry.name.clone(),
      SpotlightMode {
        kind: SpotlightKind::Generate(Box::new(move |query, store| {
          spotlight::generate(index, query, store)
        })),
        display,
      },
      allows_triggers,
    );

    modes.register_trigger(entry.name.clone(), move |query: &str| {
      spotlight::trigger_check(index, query)
    });
  }
}

#[cfg(not(feature = "spotlight"))]
#[allow(dead_code)]
fn register_plugin_spotlights(_config: &Config, _store: &mut Store) {}

fn plugin_dispatcher_item(
  _config: &Config,
  _store: &Store,
) -> miette::Result<Vec<Box<dyn DesktopItem + Send>>> {
  Ok(vec![Box::new(payload::PluginDispatcher)])
}
