use std::{any::TypeId, collections::HashMap};

use iced::{
  Alignment, Element, Length,
  widget::{Space, column, container, mouse_area, row, space, text},
};
use iced_layershell::reexport::{
  Anchor, IcedId, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use slowshell_commons::panels::PanelOrientation;
pub use slowshell_commons::panels::{PanelEdge, PanelPositions};
use slowshell_components::{Component, ComponentContext, ComponentOptions, Components};
use slowshell_config::{
  Config, ConfigParser, Value, child, child_str_owned, find_node, node_children, node_name,
  node_options_map, prop_bool, prop_i64, prop_str, str_arg,
};
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  types::{OptionalPayloadBox, PayloadBox, PayloadBuilder, ToUstr, Ustr, Void},
};
use slowshell_desktop::{
  DeployableDesktopItem, DesktopItem, EventFilter, ItemEffect, ItemMessage, MonitorScope,
  UpdateWhen, Visibility, WindowSettings,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Position {
  #[default]
  Top,
  Bottom,
  Left,
  Right,
}

impl Position {
  pub fn from_str(s: &str) -> Option<Self> {
    match s {
      "top" => Some(Position::Top),
      "bottom" => Some(Position::Bottom),
      "left" => Some(Position::Left),
      "right" => Some(Position::Right),
      _ => None,
    }
  }

  fn anchor(self) -> Anchor {
    match self {
      Position::Top => Anchor::Top | Anchor::Left | Anchor::Right,
      Position::Bottom => Anchor::Bottom | Anchor::Left | Anchor::Right,
      Position::Left => Anchor::Left | Anchor::Top | Anchor::Bottom,
      Position::Right => Anchor::Right | Anchor::Top | Anchor::Bottom,
    }
  }

  fn is_horizontal(self) -> bool {
    matches!(self, Position::Top | Position::Bottom)
  }
}

#[derive(Clone, PartialEq, Debug)]
pub struct PanelConfigComponentItem {
  pub label: String,
  pub component: String,
  pub options: Option<HashMap<Ustr, Value>>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum PanelConfigComponent {
  Named(String),
  Component(PanelConfigComponentItem),
  Group {
    label: String,
    items: Vec<PanelConfigComponent>,
    prefix: Option<PanelConfigComponentItem>,
  },
}

impl PanelConfigComponent {
  pub fn label(&self) -> String {
    match self {
      PanelConfigComponent::Named(s) => s.clone(),
      PanelConfigComponent::Component(PanelConfigComponentItem {
        label, component, ..
      }) => {
        if label.is_empty() {
          component.clone()
        } else {
          label.clone()
        }
      }
      PanelConfigComponent::Group { label, .. } => label.clone(),
    }
  }

  pub fn component(&self) -> Option<String> {
    match self {
      PanelConfigComponent::Named(name) => Some(name.clone()),
      PanelConfigComponent::Component(PanelConfigComponentItem { component, .. }) => {
        Some(component.clone())
      }
      _ => None,
    }
  }

  pub fn options(&self) -> Option<HashMap<Ustr, Value>> {
    match self {
      PanelConfigComponent::Named(_) => None,
      PanelConfigComponent::Component(PanelConfigComponentItem { options, .. }) => options.clone(),
      _ => None,
    }
  }
}

#[derive(Clone, PartialEq, Debug)]
pub struct PanelConfig {
  pub position: Position,
  pub components: Option<HashMap<String, Vec<PanelConfigComponent>>>,
  pub monitor: Option<String>,
  pub height: Option<u32>,
  pub transparent: bool,
  pub overlay: bool,
  pub autohide: bool,
  pub autohide_trigger: Option<u32>,
}

pub type PanelConfigs = HashMap<String, PanelConfig>;

pub fn build_panel_from_config(name: &str, conf: &PanelConfig, store: &Store) -> Panel {
  let mut panel = Panel::new(name, conf.position)
    .with_transparency(conf.transparent)
    .with_overlay(conf.overlay)
    .with_autohide(conf.autohide);

  if let Some(height) = conf.height {
    panel = panel.with_height(height);
  }

  if let Some(trigger) = conf.autohide_trigger {
    panel = panel.with_trigger(trigger);
  }

  if let (Some(components), Some(registry)) = (&conf.components, store.borrow::<Components>()) {
    for section in ["left", "right", "center"] {
      if let Some(components) = components.get(section) {
        for component in components.iter() {
          if let PanelConfigComponent::Group {
            items,
            label,
            prefix,
          } = component
          {
            panel = panel.with_group(
              section,
              label,
              items
                .iter()
                .map(|component| {
                  (
                    component.label(),
                    component
                      .component()
                      .and_then(|name| registry.get(&Ustr::from(name)).map(|factory| factory())),
                    component.options().map(|x| x.into()),
                  )
                })
                .collect(),
              prefix.as_ref().map(|component| {
                (
                  if component.label.is_empty() {
                    component.component.clone()
                  } else {
                    component.label.clone()
                  },
                  registry
                    .get(&Ustr::from(&component.component))
                    .map(|factory| factory()),
                  component.options.clone().map(|x| x.into()),
                )
              }),
            );
          } else {
            panel = panel.with_item(
              section,
              component.label(),
              component
                .component()
                .and_then(|name| registry.get(&Ustr::from(name)).map(|factory| factory())),
              component.options().map(|x| x.into()),
            );
          }
        }
      }
    }
  }

  panel = panel.with_scope(match conf.monitor.as_deref() {
    Some("*") | None => MonitorScope::PerMonitor,
    Some(name) => MonitorScope::Single(Some(name.into())),
  });

  panel
}

#[derive(Default)]
pub struct PanelDeloyer {
  last_configs: HashMap<String, PanelConfig>,
}

impl PanelDeloyer {
  pub fn new(config: &Config) -> Self {
    Self {
      last_configs: config.typed::<PanelConfigs>().cloned().unwrap_or_default(),
    }
  }
}

impl DeployableDesktopItem for PanelDeloyer {
  fn id(&self) -> &str {
    "panel-deployer"
  }

  fn deploys_on(&self) -> Vec<EventFilter> {
    vec![
      EventFilter::Payload("panel.create".into()),
      EventFilter::Named("config.reload".into()),
    ]
  }

  fn deploy(
    &mut self,
    config: &Config,
    store: &mut Store,
    action: &ListenerAction,
  ) -> miette::Result<Option<slowshell_desktop::DeployDesktopItemAction>> {
    match action {
      ListenerAction::Payload { name, payload } if &**name == "panel.create" => {
        let Some(payload) = payload.transform::<PanelPayload>() else {
          return Ok(None);
        };
        let Some(panel_name) = payload.name.clone() else {
          return Ok(None);
        };
        let position = payload.position;
        let height = payload.height;

        let panel = Panel::new(&*panel_name, position)
          .with_height(height)
          .with_transparency(payload.enabled)
          .with_scope(payload.monitor.clone().unwrap_or(MonitorScope::PerMonitor));

        if let Some(panels) = store.borrow_mut::<PanelPositions>() {
          panels.set(
            panel_name,
            match position {
              Position::Bottom => PanelEdge::Bottom { height },
              Position::Top => PanelEdge::Top { height },
              Position::Left => PanelEdge::Left { width: height },
              Position::Right => PanelEdge::Right { width: height },
            },
          );
        }

        Ok(Some(slowshell_desktop::DeployDesktopItemAction::Deploy(
          Box::new(panel),
        )))
      }
      ListenerAction::Named(n)
      | ListenerAction::Signal { name: n, .. }
      | ListenerAction::Timer { name: n, .. }
        if n.as_ref() == "config.reload" =>
      {
        let Some(new_configs) = config.typed::<PanelConfigs>().cloned() else {
          return Ok(None);
        };

        if self.last_configs.is_empty() {
          self.last_configs = new_configs;
          return Ok(None);
        }

        let mut actions = Vec::new();

        for (name, _) in &self.last_configs {
          if !new_configs.contains_key(name) {
            let panel_id = format!("panel/{}", name.to_lowercase().replace(' ', "-")).to_ustr();
            if let Some(panels) = store.borrow_mut::<PanelPositions>() {
              panels.remove(name);
            }
            actions.push(slowshell_desktop::DeployDesktopItemAction::Destroy(
              panel_id,
            ));
          }
        }

        for (name, conf) in &new_configs {
          if !self.last_configs.contains_key(name) {
            let mut panel = build_panel_from_config(name, conf, store);
            let _ = panel.initialize(store);
            actions.push(slowshell_desktop::DeployDesktopItemAction::Deploy(
              Box::new(panel),
            ));
          }
        }

        self.last_configs = new_configs;

        if actions.is_empty() {
          Ok(None)
        } else {
          Ok(Some(slowshell_desktop::DeployDesktopItemAction::Many(
            actions,
          )))
        }
      }
      _ => Ok(None),
    }
  }
}

pub struct PanelItem {
  label: String,
  component: Option<Box<dyn Component>>,
  options: Option<ComponentOptions>,
}

impl PanelItem {
  pub fn into_cell(self) -> PanelItemCell {
    PanelItemCell::Item(self)
  }
}

pub enum PanelItemCell {
  Group {
    label: String,
    items: Vec<PanelItem>,
    prefix: Option<PanelItem>,
  },
  Item(PanelItem),
}

impl PanelItemCell {
  pub fn into_all(self) -> Vec<PanelItem> {
    match self {
      PanelItemCell::Item(item) => vec![item],
      PanelItemCell::Group { items, prefix, .. } => {
        let mut res = items;
        if let Some(prefix) = prefix {
          res.push(prefix);
        }
        res
      }
    }
  }

  pub fn is_group(&self) -> bool {
    matches!(self, PanelItemCell::Group { .. })
  }

  pub fn as_item(&self) -> Option<&PanelItem> {
    match self {
      PanelItemCell::Item(item) => Some(item),
      _ => None,
    }
  }

  pub fn as_item_mut(&mut self) -> Option<&mut PanelItem> {
    match self {
      PanelItemCell::Item(item) => Some(item),
      _ => None,
    }
  }

  pub fn all(&self) -> impl Iterator<Item = &PanelItem> {
    match self {
      PanelItemCell::Item(item) => std::slice::from_ref(item).iter().chain(None),
      PanelItemCell::Group { items, prefix, .. } => items.iter().chain(prefix.as_ref()),
    }
  }

  pub fn all_mut(&mut self) -> impl Iterator<Item = &mut PanelItem> {
    match self {
      PanelItemCell::Item(item) => std::slice::from_mut(item).iter_mut().chain(None),
      PanelItemCell::Group { items, prefix, .. } => items.iter_mut().chain(prefix.as_mut()),
    }
  }

  pub fn label(&self) -> &str {
    match self {
      PanelItemCell::Item(item) => &item.label,
      PanelItemCell::Group { label, .. } => &label,
    }
  }

  pub fn component(&self) -> Option<&Box<dyn Component>> {
    match self {
      PanelItemCell::Item(item) => item.component.as_ref(),
      PanelItemCell::Group { .. } => None,
    }
  }

  pub fn component_mut(&mut self) -> Option<&mut Box<dyn Component>> {
    match self {
      PanelItemCell::Item(item) => item.component.as_mut(),
      PanelItemCell::Group { .. } => None,
    }
  }

  pub fn group_items_mut(&mut self) -> Option<&mut Vec<PanelItem>> {
    match self {
      PanelItemCell::Item(_) => None,
      PanelItemCell::Group { items, .. } => Some(items),
    }
  }

  pub fn has_items(&self) -> bool {
    match self {
      PanelItemCell::Item(_) => false,
      PanelItemCell::Group { items, .. } => !items.is_empty(),
    }
  }

  pub fn options(&self) -> Option<&ComponentOptions> {
    match self {
      PanelItemCell::Item(item) => item.options.as_ref(),
      PanelItemCell::Group { .. } => None,
    }
  }
}

#[derive(Default)]
pub struct PanelSections {
  pub left: Vec<PanelItemCell>,
  pub center: Vec<PanelItemCell>,
  pub right: Vec<PanelItemCell>,
}

/// `panel.<name>.add`: `section` (left/center/right), `label`, `component`
/// `panel.<name>.remove`: `section`, `label`
/// `panel.<name>.configure`: `height`, `position`, `enabled`
/// `panel.<name>.destroy`
pub struct Panel {
  cached_id: Ustr,
  name: Ustr,
  position: Position,
  height: u32,
  sections: PanelSections,
  scope: MonitorScope,
  enabled: bool,
  hidden: bool,
  transparent: bool,
  overlay: bool,
  autohide: bool,
  trigger: i32,
}

impl Panel {
  pub fn new(name: impl Into<Ustr>, position: Position) -> Self {
    let name = name.into();
    let cached_id = format!("panel/{}", name.to_lowercase().replace(' ', "-")).to_ustr();
    Self {
      cached_id,
      name,
      position,
      height: 32,
      sections: PanelSections::default(),
      scope: MonitorScope::PerMonitor,
      enabled: true,
      hidden: false,
      transparent: false,
      overlay: false,
      autohide: false,
      trigger: 4,
    }
  }

  pub fn with_height(mut self, height: u32) -> Self {
    self.height = height;
    self
  }

  pub fn with_transparency(mut self, transparent: bool) -> Self {
    self.transparent = transparent;
    self
  }

  pub fn with_scope(mut self, scope: MonitorScope) -> Self {
    self.scope = scope;
    self
  }

  pub fn with_overlay(mut self, overlay: bool) -> Self {
    self.overlay = overlay;
    self
  }

  pub fn with_autohide(mut self, autohide: bool) -> Self {
    self.autohide = autohide;
    self
  }

  pub fn with_trigger(mut self, trigger: u32) -> Self {
    self.trigger = trigger as i32;
    self
  }

  pub fn with_item(
    mut self,
    section: &str,
    label: impl Into<String>,
    component: Option<Box<dyn Component>>,
    options: Option<ComponentOptions>,
  ) -> Self {
    let item = PanelItem {
      label: label.into(),
      component,
      options,
    };
    match section {
      "left" => self.sections.left.push(item.into_cell()),
      "center" => self.sections.center.push(item.into_cell()),
      "right" => self.sections.right.push(item.into_cell()),
      _ => {}
    }
    self
  }

  pub fn with_group(
    mut self,
    section: &str,
    label: impl Into<String>,
    items: Vec<(
      impl Into<String>,
      Option<Box<dyn Component>>,
      Option<ComponentOptions>,
    )>,
    prefix: Option<(
      impl Into<String>,
      Option<Box<dyn Component>>,
      Option<ComponentOptions>,
    )>,
  ) -> Self {
    let item = PanelItemCell::Group {
      label: label.into(),
      items: items
        .into_iter()
        .map(|(label, component, options)| PanelItem {
          label: label.into(),
          component,
          options,
        })
        .collect(),
      prefix: prefix.map(|(label, component, options)| PanelItem {
        label: label.into(),
        component,
        options,
      }),
    };

    match section {
      "left" => self.sections.left.push(item),
      "center" => self.sections.center.push(item),
      "right" => self.sections.right.push(item),
      _ => {}
    }
    self
  }

  fn event_prefix(&self) -> String {
    format!("panel.{}", self.name.to_lowercase().replace(' ', "-"))
  }

  fn size(&self) -> (u32, u32) {
    if self.position.is_horizontal() {
      (0, self.height)
    } else {
      (self.height, 0)
    }
  }

  fn add_item_from_payload(&mut self, store: &mut Store, payload: &PanelComponentPayload) {
    let component = payload.component.as_ref().and_then(|name| {
      store
        .borrow::<Components>()
        .and_then(|registry| registry.get(name))
        .map(|factory| factory())
    });
    let options = payload.options.clone();

    let mut item = PanelItem {
      label: payload.label.to_string(),
      component,
      options: options,
    };

    if let Some(comp) = &mut item.component {
      comp.watch(store, item.options.as_ref());
    }

    match &*payload.section {
      "left" => self.sections.left.push(item.into_cell()),
      "right" => self.sections.right.push(item.into_cell()),
      _ => self.sections.center.push(item.into_cell()),
    }
  }

  fn remove_from_payload(&mut self, store: &mut Store, payload: &PanelComponentPayload) {
    let section = &payload.section;

    let label = payload.label.clone();

    let target = match &**section {
      "left" => &mut self.sections.left,
      "right" => &mut self.sections.right,
      _ => &mut self.sections.center,
    };

    if let Some(group) = target.iter_mut().find(|item| item.label() == &**section) {
      if let Some(items) = group.group_items_mut() {
        items.retain_mut(|sub_item| {
          if sub_item.label == &*label {
            if let Some(component) = &mut sub_item.component {
              component.stop(store, sub_item.options.as_ref());
            }
            false
          } else {
            true
          }
        });
      }

      return;
    }

    target.retain_mut(|item| {
      if item.label() == &*label {
        for sub_item in item.all_mut() {
          if let Some(component) = &mut sub_item.component {
            component.stop(store, sub_item.options.as_ref());
          }
        }
        false
      } else {
        true
      }
    });
  }

  fn diff_section(
    &mut self,
    section_name: &str,
    new_components: Option<&Vec<PanelConfigComponent>>,
    store: &mut Store,
  ) -> bool {
    let section_list = match section_name {
      "left" => &mut self.sections.left,
      "center" => &mut self.sections.center,
      "right" => &mut self.sections.right,
      _ => return false,
    };

    let Some(new_components) = new_components else {
      if section_list.is_empty() {
        return false;
      }
      for item_cell in section_list.drain(..) {
        for mut sub_item in item_cell.into_all() {
          if let Some(comp) = &mut sub_item.component {
            comp.stop(store, sub_item.options.as_ref());
          }
        }
      }
      return true;
    };

    let mut changed = false;
    let mut new_cells = Vec::new();
    let old_cells = std::mem::take(section_list);
    let mut old_map: HashMap<String, PanelItemCell> = HashMap::new();
    for cell in old_cells {
      old_map.insert(cell.label().to_string(), cell);
    }

    for conf_comp in new_components {
      let label = conf_comp.label();
      match conf_comp {
        PanelConfigComponent::Group {
          items: group_items,
          prefix: group_prefix,
          ..
        } => {
          let mut old_cell = old_map.remove(&label);
          let (existing_items, existing_prefix) = match old_cell.take() {
            Some(PanelItemCell::Group { items, prefix, .. }) => (items, prefix),
            Some(other) => {
              for mut item in other.into_all() {
                if let Some(comp) = &mut item.component {
                  comp.stop(store, item.options.as_ref());
                }
              }
              changed = true;
              (Vec::new(), None)
            }
            None => {
              changed = true;
              (Vec::new(), None)
            }
          };

          let mut old_item_map: HashMap<String, PanelItem> = HashMap::new();
          for item in existing_items {
            old_item_map.insert(item.label.clone(), item);
          }

          let mut new_group_items = Vec::new();
          for item_conf in group_items {
            let item_label = item_conf.label();
            let new_options: Option<ComponentOptions> = item_conf.options().map(|o| o.into());
            if let Some(mut existing) = old_item_map.remove(&item_label) {
              if existing.options != new_options {
                existing.options = new_options;
                if let Some(comp) = &mut existing.component {
                  comp.watch(store, existing.options.as_ref());
                }
                changed = true;
              }
              new_group_items.push(existing);
            } else {
              let comp = item_conf.component().and_then(|name| {
                store
                  .borrow::<Components>()
                  .and_then(|reg| reg.get(&Ustr::from(name)))
                  .map(|f| f())
              });
              let mut item = PanelItem {
                label: item_label,
                component: comp,
                options: new_options,
              };
              if let Some(comp) = &mut item.component {
                comp.watch(store, item.options.as_ref());
              }
              new_group_items.push(item);
              changed = true;
            }
          }

          for (_, mut old_item) in old_item_map {
            if let Some(comp) = &mut old_item.component {
              comp.stop(store, old_item.options.as_ref());
            }
            changed = true;
          }

          let new_prefix_item = if let Some(p_conf) = group_prefix {
            let p_label = if p_conf.label.is_empty() {
              p_conf.component.clone()
            } else {
              p_conf.label.clone()
            };
            let new_p_options: Option<ComponentOptions> = p_conf.options.clone().map(|o| o.into());
            if let Some(mut old_p) = existing_prefix {
              if old_p.options != new_p_options {
                old_p.options = new_p_options;
                if let Some(comp) = &mut old_p.component {
                  comp.watch(store, old_p.options.as_ref());
                }
                changed = true;
              }
              Some(old_p)
            } else {
              let comp = store
                .borrow::<Components>()
                .and_then(|reg| reg.get(&Ustr::from(&p_conf.component)))
                .map(|f| f());
              let mut item = PanelItem {
                label: p_label,
                component: comp,
                options: new_p_options,
              };
              if let Some(comp) = &mut item.component {
                comp.watch(store, item.options.as_ref());
              }
              changed = true;
              Some(item)
            }
          } else {
            if let Some(mut old_p) = existing_prefix {
              if let Some(comp) = &mut old_p.component {
                comp.stop(store, old_p.options.as_ref());
              }
              changed = true;
            }
            None
          };

          new_cells.push(PanelItemCell::Group {
            label,
            items: new_group_items,
            prefix: new_prefix_item,
          });
        }
        _ => {
          let new_options: Option<ComponentOptions> = conf_comp.options().map(|o| o.into());
          if let Some(old_cell) = old_map.remove(&label) {
            match old_cell {
              PanelItemCell::Item(mut item) => {
                if item.options != new_options {
                  item.options = new_options;
                  if let Some(comp) = &mut item.component {
                    comp.watch(store, item.options.as_ref());
                  }
                  changed = true;
                }
                new_cells.push(PanelItemCell::Item(item));
              }
              PanelItemCell::Group { items, prefix, .. } => {
                for mut item in items {
                  if let Some(comp) = &mut item.component {
                    comp.stop(store, item.options.as_ref());
                  }
                }
                if let Some(mut item) = prefix {
                  if let Some(comp) = &mut item.component {
                    comp.stop(store, item.options.as_ref());
                  }
                }
                let comp = conf_comp.component().and_then(|name| {
                  store
                    .borrow::<Components>()
                    .and_then(|reg| reg.get(&Ustr::from(name)))
                    .map(|f| f())
                });
                let mut item = PanelItem {
                  label,
                  component: comp,
                  options: new_options,
                };
                if let Some(comp) = &mut item.component {
                  comp.watch(store, item.options.as_ref());
                }
                new_cells.push(PanelItemCell::Item(item));
                changed = true;
              }
            }
          } else {
            let comp = conf_comp.component().and_then(|name| {
              store
                .borrow::<Components>()
                .and_then(|reg| reg.get(&Ustr::from(name)))
                .map(|f| f())
            });
            let mut item = PanelItem {
              label,
              component: comp,
              options: new_options,
            };
            if let Some(comp) = &mut item.component {
              comp.watch(store, item.options.as_ref());
            }
            new_cells.push(PanelItemCell::Item(item));
            changed = true;
          }
        }
      }
    }

    for (_, cell) in old_map {
      for mut item in cell.into_all() {
        if let Some(comp) = &mut item.component {
          comp.stop(store, item.options.as_ref());
        }
      }
      changed = true;
    }

    *section_list = new_cells;
    changed
  }

  fn configure_from_payload(&mut self, payload: &PanelPayload) {
    self.height = payload.height;
    self.position = payload.position;
    self.enabled = payload.enabled;
    self.transparent = payload.transparent;
  }

  fn edge(&self) -> PanelEdge {
    match self.position {
      Position::Bottom => PanelEdge::Bottom {
        height: self.height,
      },
      Position::Top => PanelEdge::Top {
        height: self.height,
      },
      Position::Left => PanelEdge::Left { width: self.height },
      Position::Right => PanelEdge::Right { width: self.height },
    }
  }

  fn exclusive_zone(&self, collapsed: bool) -> i32 {
    if self.overlay {
      -1
    } else if collapsed {
      0
    } else {
      self.height as i32
    }
  }

  fn collapsed_margin(&self) -> (i32, i32, i32, i32) {
    let height = self.height as i32;
    let trigger = self.trigger.clamp(0, (height - 1).max(0));
    let offset = -(height - trigger);
    match self.position {
      Position::Top => (offset, 0, 0, 0),
      Position::Bottom => (0, 0, offset, 0),
      Position::Left => (0, 0, 0, offset),
      Position::Right => (0, offset, 0, 0),
    }
  }

  fn items(&self) -> impl Iterator<Item = &PanelItemCell> {
    self
      .sections
      .left
      .iter()
      .chain(self.sections.center.iter())
      .chain(self.sections.right.iter())
  }

  fn items_mut(&mut self) -> impl Iterator<Item = &mut PanelItemCell> {
    self
      .sections
      .left
      .iter_mut()
      .chain(self.sections.center.iter_mut())
      .chain(self.sections.right.iter_mut())
  }

  fn create_item<'a>(
    config: &Config,
    store: &Store,
    ctx: &ComponentContext,
    item: &'a PanelItem,
    style: &slowshell_config::style::Style,
  ) -> Element<'a, ItemMessage> {
    let theme = &config.theme;
    let font_size = style.number("font.size").unwrap_or(13.0);
    let color = style.color(&theme, "color", theme.text);

    match &item.component {
      Some(comp) => comp.view(config, store, ctx, item.options.as_ref()),
      None => text(&item.label).size(font_size).color(color).into(),
    }
  }

  fn render_item<'a>(
    config: &Config,
    store: &Store,
    ctx: &ComponentContext,
    item: &'a PanelItemCell,
    horizontal: bool,
  ) -> Element<'a, ItemMessage> {
    let style = config.style("item");
    let theme = &config.theme;

    let bg = style.color(&theme, "background", theme.mantle);
    let prefix_bg = style.color(&theme, "prefix.background", theme.base);
    let radius = style.number("radius").unwrap_or(4.0);
    let prefix_radius = style.number("prefix.radius").unwrap_or(4.0);
    let padding = style.padding([4.0, 8.0]);
    let spacing = style.number("spacing").unwrap_or(4.0);
    let margin = style.number("margin").unwrap_or(4.0);

    let container_style = move |_t: &iced::Theme| container::Style {
      background: Some(bg.into()),
      border: iced::Border {
        radius: radius.into(),
        ..Default::default()
      },
      ..Default::default()
    };

    let mut has_prefix: Option<Element<'a, ItemMessage>> = None;

    let content: Element<'a, ItemMessage> = match item {
      PanelItemCell::Item(item) => {
        if let Some(comp) = &item.component {
          if !comp.check_view(store, item.options.as_ref()) {
            return space().into();
          }
        }
        Self::create_item(config, store, ctx, item, &style)
      }

      PanelItemCell::Group { items, prefix, .. } if item.has_items() => {
        let main = if horizontal {
          row(
            items
              .iter()
              .map(|item| Self::create_item(config, store, ctx, item, &style)),
          )
          .align_y(Alignment::Center)
          .width(if horizontal {
            Length::Shrink
          } else {
            Length::Fill
          })
          .height(if horizontal {
            Length::Fill
          } else {
            Length::Shrink
          })
          .spacing(spacing)
          .into()
        } else {
          column(
            items
              .iter()
              .map(|item| Self::create_item(config, store, ctx, item, &style)),
          )
          .width(if horizontal {
            Length::Shrink
          } else {
            Length::Fill
          })
          .height(if horizontal {
            Length::Fill
          } else {
            Length::Shrink
          })
          .align_x(Alignment::Center)
          .spacing(spacing)
          .into()
        };

        if let Some(prefix) = prefix {
          let bg = prefix
            .options
            .as_ref()
            .and_then(|x| x.str("highlight"))
            .and_then(|name| theme.color(name))
            .unwrap_or(prefix_bg);

          let prefix_content: Element<'a, ItemMessage> =
            Self::create_item(config, store, ctx, prefix, &style);

          has_prefix = Some(
            container(prefix_content)
              .style(move |_t: &iced::Theme| container::Style {
                background: Some(bg.into()),
                border: iced::Border {
                  radius: prefix_radius.into(),
                  ..Default::default()
                },
                ..Default::default()
              })
              .padding(padding)
              .width(if horizontal {
                Length::Shrink
              } else {
                Length::Fill
              })
              .height(if horizontal {
                Length::Fill
              } else {
                Length::Shrink
              })
              .align_x(Alignment::Center)
              .align_y(Alignment::Center)
              .into(),
          );
        };

        main
      }

      _ => space().into(),
    };

    let inner: Element<'_, _> = if let Some(prefix) = has_prefix {
      if horizontal {
        row![prefix, content]
          .height(Length::Fill)
          .align_y(Alignment::Center)
          .into()
      } else {
        column![prefix, content]
          .width(Length::Fill)
          .align_x(Alignment::Center)
          .into()
      }
    } else {
      container(content).padding(padding).into()
    };

    let mut content = container(inner).style(container_style);

    if horizontal {
      content = content.height(Length::Fill).align_y(Alignment::Center);
    } else {
      content = content.width(Length::Fill).align_x(Alignment::Center);
    }

    if margin > 0.0 {
      container(content).padding(margin).into()
    } else {
      content.into()
    }
  }

  fn render_section<'a>(
    config: &Config,
    store: &Store,
    ctx: &ComponentContext,
    items: &'a [PanelItemCell],
    horizontal: bool,
  ) -> Element<'a, ItemMessage> {
    if items.is_empty() {
      return Space::new().into();
    }

    let children: Vec<Element<'a, ItemMessage>> = items
      .iter()
      .map(|i| Self::render_item(config, store, ctx, i, horizontal))
      .collect();

    if horizontal {
      row(children)
        .spacing(config.style("panel").number("item.spacing").unwrap_or(4.0))
        .into()
    } else {
      column(children)
        .spacing(config.style("panel").number("item.spacing").unwrap_or(4.0))
        .into()
    }
  }

  fn all_events(&self) -> Vec<EventFilter> {
    let prefix = self.event_prefix();
    let mut events = vec![
      EventFilter::Payload(format!("{}.add", prefix).into()),
      EventFilter::Payload(format!("{}.remove", prefix).into()),
      EventFilter::Payload(format!("{}.configure", prefix).into()),
      EventFilter::Named(format!("{}.destroy", prefix).into()),
      EventFilter::Named(format!("{}.hide", prefix).into()),
      EventFilter::Named(format!("{}.show", prefix).into()),
      EventFilter::Named(format!("{}.toggle", prefix).into()),
      EventFilter::Named("config.reload".into()),
    ];
    for item in self.items() {
      for item in item.all() {
        if let Some(comp) = &item.component {
          events.extend(comp.events());
        }
      }
    }
    events
  }

  fn hide(&mut self, store: &mut Store) -> ItemEffect {
    if let Some(panels) = store.borrow_mut::<PanelPositions>() {
      panels.remove(&self.name);
    }
    self.hidden = true;
    ItemEffect::ReallyHide
  }

  fn show(&mut self, store: &mut Store) -> ItemEffect {
    if let Some(panels) = store.borrow_mut::<PanelPositions>() {
      let height = self.height;
      panels.set(
        self.name.clone(),
        match self.position {
          Position::Bottom => PanelEdge::Bottom { height },
          Position::Top => PanelEdge::Top { height },
          Position::Left => PanelEdge::Left { width: height },
          Position::Right => PanelEdge::Right { width: height },
        },
      );
    }
    self.hidden = false;
    ItemEffect::Show
  }
}

impl DesktopItem for Panel {
  fn id(&self) -> &str {
    &self.cached_id
  }

  fn layer(&self, _config: &Config, monitor: &str) -> NewLayerShellSettings {
    let (w, h) = self.size();
    NewLayerShellSettings {
      layer: Layer::Top,
      anchor: self.position.anchor(),
      exclusive_zone: Some(self.exclusive_zone(self.autohide)),
      size: Some((w, h)),
      margin: Some(if self.autohide {
        self.collapsed_margin()
      } else {
        (0, 0, 0, 0)
      }),
      keyboard_interactivity: KeyboardInteractivity::None,
      events_transparent: false,
      namespace: Some(format!("slowshell-panel-{}", self.name.to_lowercase())),
      output_option: OutputOption::OutputName(monitor.to_string()),
    }
  }

  fn visibility(&self) -> Visibility {
    Visibility::Visible
  }

  fn update_strategy(&self) -> UpdateWhen {
    UpdateWhen::OnDemand
  }

  fn monitor(&self, _config: &Config) -> MonitorScope {
    self.scope.clone()
  }

  fn init_events(&self) -> Vec<EventFilter> {
    self.all_events()
  }

  fn initialize(&mut self, store: &mut Store) -> miette::Result<Void> {
    for item in self.items_mut() {
      for sub_item in item.all_mut() {
        if let Some(comp) = &mut sub_item.component {
          comp.watch(store, sub_item.options.as_ref());
        }
      }
    }

    if let Some(panels) = store.borrow_mut::<PanelPositions>() {
      let height = self.height;
      panels.set(
        self.name.clone(),
        match self.position {
          Position::Bottom => PanelEdge::Bottom { height },
          Position::Top => PanelEdge::Top { height },
          Position::Left => PanelEdge::Left { width: height },
          Position::Right => PanelEdge::Right { width: height },
        },
      );
    }

    Ok(Void)
  }

  fn update(
    &mut self,
    config: &Config,
    store: &mut Store,
    event: &ListenerAction,
  ) -> miette::Result<ItemEffect> {
    let prefix = self.event_prefix();

    let mut effect = match event {
      ListenerAction::Payload { name, payload } if &**name == format!("{}.add", prefix) => {
        if let Some(p) = payload {
          self.add_item_from_payload(
            store,
            p.as_this::<PanelComponentPayload>()
              .expect("wrong panel payload"),
          );
        }
        ItemEffect::Subscribe(self.all_events())
      }
      ListenerAction::Payload { name, payload } if &**name == format!("{}.remove", prefix) => {
        if let Some(p) = payload {
          self.remove_from_payload(
            store,
            p.as_this::<PanelComponentPayload>()
              .expect("wrong panel payload"),
          );
        }
        ItemEffect::Subscribe(self.all_events())
      }
      ListenerAction::Payload { name, payload } if &**name == format!("{}.configure", prefix) => {
        if let Some(p) = payload {
          self.configure_from_payload(p.as_this::<PanelPayload>().expect("wrong panel payload"));

          if let Some(panels) = store.borrow_mut::<PanelPositions>() {
            let height = self.height;
            panels.remove(&**name);
            panels.set(
              name.clone(),
              match self.position {
                Position::Bottom => PanelEdge::Bottom { height },
                Position::Top => PanelEdge::Top { height },
                Position::Left => PanelEdge::Left { width: height },
                Position::Right => PanelEdge::Right { width: height },
              },
            );
          }
        }
        if !self.enabled {
          ItemEffect::Hide
        } else {
          ItemEffect::Redraw
        }
      }
      ListenerAction::Named(name) if &**name == format!("{}.destroy", prefix) => {
        if let Some(panels) = store.borrow_mut::<PanelPositions>() {
          panels.remove(&**name);
        }
        for item in self.items_mut() {
          for sub_item in item.all_mut() {
            if let Some(comp) = &mut sub_item.component {
              comp.stop(store, sub_item.options.as_ref());
            }
          }
        }
        ItemEffect::ReallyDestroy
      }
      ListenerAction::Named(name) if &**name == format!("{}.hide", prefix) => self.hide(store),
      ListenerAction::Named(name) if &**name == format!("{}.show", prefix) => self.show(store),
      ListenerAction::Named(name) if &**name == format!("{}.toggle", prefix) => {
        if self.hidden {
          self.show(store)
        } else {
          self.hide(store)
        }
      }
      ListenerAction::Named(name)
      | ListenerAction::Signal { name, .. }
      | ListenerAction::Timer { name, .. }
        if name.as_ref() == "config.reload" =>
      {
        let mut need_resubscribe = false;
        if let Some(configs) = config.typed::<PanelConfigs>() {
          if let Some(conf) = configs.get(&*self.name) {
            let old_edge = self.edge();
            self.height = conf.height.unwrap_or(32);
            self.position = conf.position;
            self.transparent = conf.transparent;
            self.overlay = conf.overlay;
            self.autohide = conf.autohide;
            if let Some(trigger) = conf.autohide_trigger {
              self.trigger = trigger as i32;
            }
            if let Some(panels) = store.borrow_mut::<PanelPositions>() {
              if old_edge != self.edge() {
                panels.set(self.name.clone(), self.edge());
              }
            }
            let left_changed = self.diff_section(
              "left",
              conf.components.as_ref().and_then(|c| c.get("left")),
              store,
            );
            let center_changed = self.diff_section(
              "center",
              conf.components.as_ref().and_then(|c| c.get("center")),
              store,
            );
            let right_changed = self.diff_section(
              "right",
              conf.components.as_ref().and_then(|c| c.get("right")),
              store,
            );
            if left_changed || center_changed || right_changed {
              need_resubscribe = true;
            }
          }
        }
        if need_resubscribe {
          ItemEffect::Subscribe(self.all_events())
        } else {
          ItemEffect::Redraw
        }
      }
      _ => ItemEffect::None,
    };

    for item in self.items_mut() {
      for sub_item in item.all_mut() {
        if let Some(comp) = &mut sub_item.component {
          if comp.events().iter().any(|filter| filter.matches(event)) {
            if let Ok(comp_effect) = comp.update(config, store, event, sub_item.options.as_ref()) {
              if comp_effect != ItemEffect::None {
                effect = comp_effect;
              }
            }
          }
        }
      }
    }

    Ok(effect)
  }

  fn view<'a>(
    &'a self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    monitor: &str,
  ) -> Element<'a, ItemMessage> {
    let ctx = ComponentContext {
      panel_name: &*self.name,
      position: self.edge(),
      orientation: if self.position.is_horizontal() {
        PanelOrientation::Horizontal
      } else {
        PanelOrientation::Vertical
      },
      monitor,
    };
    let left = Self::render_section(
      config,
      store,
      &ctx,
      &self.sections.left,
      self.position.is_horizontal(),
    );
    let center = Self::render_section(
      config,
      store,
      &ctx,
      &self.sections.center,
      self.position.is_horizontal(),
    );
    let right = Self::render_section(
      config,
      store,
      &ctx,
      &self.sections.right,
      self.position.is_horizontal(),
    );

    let style = config.style("panel");

    let bar_content: Element<'_, ItemMessage> = if self.position.is_horizontal() {
      row![
        container(left).width(Length::Fill).align_left(Length::Fill),
        container(center)
          .width(Length::Shrink)
          .center_x(Length::Shrink),
        container(right)
          .width(Length::Fill)
          .align_right(Length::Fill),
      ]
      .align_y(iced::Alignment::Center)
      .height(Length::Fill)
      .width(Length::Fill)
      .into()
    } else {
      column![
        container(left).align_top(Length::Shrink),
        Space::new().height(Length::Fill),
        container(center).center_y(Length::Shrink),
        Space::new().height(Length::Fill),
        container(right).align_bottom(Length::Shrink),
      ]
      .align_x(iced::Alignment::Center)
      .width(Length::Fill)
      .height(Length::Fill)
      .into()
    };

    let background = style.color(
      &config.theme,
      "background",
      config
        .theme
        .overlay_opacity(style.number("background.opacity").unwrap_or(0.4)),
    );

    let pad = style.number("padding").unwrap_or(8.0);
    let bar = mouse_area(
      container(bar_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(if self.position.is_horizontal() {
          [0.0, pad]
        } else {
          [pad, 0.0]
        })
        .style(move |_t: &iced::Theme| container::Style {
          background: if self.transparent {
            None
          } else {
            Some(background.into())
          },
          ..container::Style::default()
        }),
    )
    .interaction(iced::mouse::Interaction::None);

    let bar = if self.autohide {
      let expanded = WindowSettings {
        margin: Some((0, 0, 0, 0)),
        exclusive_zone: Some(self.exclusive_zone(false)),
        ..Default::default()
      };
      let collapsed = WindowSettings {
        margin: Some(self.collapsed_margin()),
        exclusive_zone: Some(self.exclusive_zone(true)),
        ..Default::default()
      };
      bar
        .on_enter(ItemMessage::Effect(id, ItemEffect::UpdateWindow(expanded)))
        .on_exit(ItemMessage::Effect(id, ItemEffect::UpdateWindow(collapsed)))
    } else {
      bar
    };

    bar.into()
  }

  fn handle_message(&mut self, _store: Option<&mut Store>, message: &ItemMessage) -> ItemEffect {
    match message {
      ItemMessage::Effect(_, effect @ ItemEffect::UpdateWindow(_)) => effect.clone(),
      _ => ItemEffect::None,
    }
  }
}

fn parse_component_item(node: &kdl::KdlNode) -> Option<PanelConfigComponentItem> {
  let component = str_arg(node, 0)?.to_string();
  let label = child_str_owned(node, "label").unwrap_or_default();
  let mut options = node_options_map(node);
  options.remove(&"label".to_ustr());
  Some(PanelConfigComponentItem {
    label,
    component,
    options: if options.is_empty() {
      None
    } else {
      Some(options)
    },
  })
}

fn parse_panel_component(node: &kdl::KdlNode) -> Option<PanelConfigComponent> {
  match node_name(node) {
    "component" => {
      if node_children(node).is_empty() {
        Some(PanelConfigComponent::Named(str_arg(node, 0)?.to_string()))
      } else {
        Some(PanelConfigComponent::Component(parse_component_item(node)?))
      }
    }
    "group" => {
      let label = str_arg(node, 0)?.to_string();
      let mut items = Vec::new();
      let mut prefix = None;
      for child in node_children(node) {
        match node_name(child) {
          "item" => {
            if let Some(item) = parse_component_item(child) {
              items.push(PanelConfigComponent::Component(item));
            }
          }
          "prefix" => prefix = parse_component_item(child),
          _ => {}
        }
      }
      Some(PanelConfigComponent::Group {
        label,
        items,
        prefix,
      })
    }
    _ => None,
  }
}

#[derive(Clone, Default)]
pub struct PanelPayload {
  height: u32,
  position: Position,
  monitor: Option<MonitorScope>,
  enabled: bool,
  transparent: bool,
  name: Option<Ustr>,
}

#[derive(Clone)]
pub struct PanelComponentPayload {
  section: Ustr,
  label: Ustr,
  component: Option<Ustr>,
  options: Option<ComponentOptions>,
}

impl Default for PanelComponentPayload {
  fn default() -> Self {
    Self {
      section: "center".into(),
      label: Default::default(),
      component: None,
      options: None,
    }
  }
}

slowshell_registry::register_resources!(
  payload: Unknown(PayloadBuilder {
    commands: &["panel."],
    build: |command, args| {
      if command == "panel.create" || command.ends_with(".configure") {
        return Some(PayloadBox::new(
          Some(args.as_map()).and_then(|x| Some(PanelPayload {
            height: x.get("height").cloned().and_then(|x| x.parse().ok()).unwrap_or(32),
            enabled: x.get("enabled").map(|x| *x == "true").unwrap_or(false),
            transparent: x.get("transparent").map(|x| *x == "true").unwrap_or(false),
            monitor: x.get("monitor").map(|x| if *x == "*" { MonitorScope::PerMonitor } else { MonitorScope::Single(Some(x.into())) }),
            name: if command == "panel.create" { Some(x.get("name").map(Ustr::from)?) } else { None },
            position: x
              .get("position")
              .and_then(|s| Position::from_str(&**s))
              .unwrap_or(Position::Top)
          }))
          .unwrap_or_default()
        ))
      }
      if command.ends_with(".add") || command.ends_with(".remove") {
        return Some(PayloadBox::new(
          Some(args.as_map()).and_then(|x| Some(PanelComponentPayload {
            component: x.get("component").map(Ustr::from),
            label: x.get("label").map(Ustr::from)?,
            options: x.get("options").map(|options_string| {
              let mut hashmap: HashMap<Ustr, Value> = HashMap::new();

              for group in options_string.split(",") {
                if let Some((k, v)) = group.split_once("") {
                  hashmap.insert(k.into(), v.into());
                }
              }

              ComponentOptions::from(hashmap)
            }),
            section: x.get("section").map(Ustr::from)?,
          }))
          .unwrap_or_default()
        ))
      }
      None
    }
  }.into_boxed()),
  config: Unknown(Box::new(ConfigParser {
    type_id: TypeId::of::<PanelConfigs>(),
    de: |nodes| {
      let Some(panels) = find_node(nodes, "panels") else {
        return Ok(None);
      };

      let mut configs: PanelConfigs = HashMap::new();
      for node in node_children(panels) {
        if node_name(node) != "panel" {
          continue;
        }
        let Some(name) = str_arg(node, 0) else {
          continue;
        };

        let mut conf = PanelConfig {
          position: prop_str(node, "position")
            .and_then(Position::from_str)
            .unwrap_or_default(),
          components: None,
          monitor: prop_str(node, "monitor").map(str::to_owned),
          height: prop_i64(node, "height").map(|height| height as u32),
          transparent: prop_bool(node, "transparent").unwrap_or(false),
          overlay: prop_bool(node, "overlay").unwrap_or(false),
          autohide: prop_bool(node, "autohide").unwrap_or(false),
          autohide_trigger: prop_i64(node, "autohide-trigger").map(|trigger| trigger as u32),
        };

        if let Some(components_node) = child(node, "components") {
          let mut components = HashMap::new();
          for section in ["left", "center", "right"] {
            if let Some(section_node) = child(components_node, section) {
              let items: Vec<PanelConfigComponent> = node_children(section_node)
                .iter()
                .filter_map(parse_panel_component)
                .collect();
              if !items.is_empty() {
                components.insert(section.to_string(), items);
              }
            }
          }
          conf.components = Some(components);
        }

        configs.insert(name.to_string(), conf);
      }

      Ok(Some(Box::new(configs)))
    }
  })),
  app: Item(|config, store| {
    Ok(
      config
        .typed::<PanelConfigs>()
        .ok_or_else(|| miette::miette!("No panel config found"))?
        .iter()
        .map(|(name, conf)| {
          let panel = build_panel_from_config(name, conf, store);
          Box::new(panel) as Box<dyn DesktopItem + Send>
        })
        .collect(),
    )
  }),
  panel: Style(
    slowshell_config::style! {
      "background" => "base",
      "background.opacity" => 0.45,
      "padding" => 8,
      "spacing" => 4,
      "item.spacing" => 4,
    }
  ),
  item: Style(
    slowshell_config::style! {
      "background" => "mantle",
      "background.opacity" => 0.6,
      "radius" => 4,
      "padding.x" => 8,
      "padding.y" => 4,
      "margin" => 4,
      "font.size" => 13,
      "color" => "text",
      "spacing" => 8.0,
    }
  ),
);
