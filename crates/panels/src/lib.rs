use std::collections::HashMap;

use iced::{
  Color, Element, Length,
  widget::{Space, column, container, row, text},
};
use iced_layershell::reexport::{
  Anchor, IcedId, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
pub use slowshell_commons::panels::{PanelEdge, PanelPositions};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  types::{ToUstr, Ustr},
};
use slowshell_desktop::{
  DeployableDesktopItem, DesktopItem, EventFilter, ItemEffect, ItemMessage, MonitorScope,
  UpdateWhen, Visibility,
};

pub struct PanelDeloyer;

impl DeployableDesktopItem for PanelDeloyer {
  fn id(&self) -> &str {
    "panel-deployer"
  }

  fn deploys_on(&self) -> Vec<EventFilter> {
    vec![EventFilter::Payload {
      name: "panel.create".into(),
      payload: None,
    }]
  }

  fn deploy(
    &mut self,
    store: &mut Store,
    action: &ListenerAction,
  ) -> anyhow::Result<Option<slowshell_desktop::DeployDesktopItemAction>> {
    let mut try_deploy = || {
      let (_name, payload) = match action {
        ListenerAction::Payload { name, payload } if name.as_str() == "panel.create" => {
          println!("Deploying!!!0");
          (name, payload.as_ref()?)
        }
        _ => return None,
      };

      let panel_name = payload.get::<str>("name")?.to_string();

      let position = payload
        .get::<str>("position")
        .and_then(|s| Position::from_str(s.as_str()))
        .unwrap_or(Position::Top);

      let height = payload
        .get::<str>("height")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(32);

      let scope = match payload.get::<str>("monitor").map(|s| s.as_str()) {
        Some("*") | None => MonitorScope::PerMonitor,
        Some(name) => MonitorScope::Single(Some(name.into())),
      };

      let panel = Panel::new(&panel_name, position)
        .with_height(height)
        .with_scope(scope);

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

      Some(Box::new(panel))
    };

    Ok(try_deploy().map(|p| slowshell_desktop::DeployDesktopItemAction::Deploy(p)))
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
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

#[derive(Debug, Clone)]
pub struct PanelItem {
  pub label: String,
  pub component: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct PanelSections {
  pub left: Vec<PanelItem>,
  pub center: Vec<PanelItem>,
  pub right: Vec<PanelItem>,
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
    }
  }

  pub fn with_height(mut self, height: u32) -> Self {
    self.height = height;
    self
  }

  pub fn with_scope(mut self, scope: MonitorScope) -> Self {
    self.scope = scope;
    self
  }

  pub fn with_item(
    mut self,
    section: &str,
    label: impl Into<String>,
    component: Option<String>,
  ) -> Self {
    let item = PanelItem {
      label: label.into(),
      component,
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

  fn add_item_from_payload(&mut self, payload: &HashMap<Ustr, Ustr>) {
    let section = payload
      .get::<str>("section")
      .map(|s| s.as_str())
      .unwrap_or("center");
    let label = payload
      .get::<str>("label")
      .map(|s| s.to_string())
      .unwrap_or_default();
    let component = payload.get::<str>("component").map(|s| s.to_string());

    let item = PanelItem { label, component };

    match section {
      "left" => self.sections.left.push(item),
      "right" => self.sections.right.push(item),
      _ => self.sections.center.push(item),
    }
  }

  fn remove_from_payload(&mut self, payload: &HashMap<Ustr, Ustr>) {
    let section = payload
      .get::<str>("section")
      .map(|s| s.as_str())
      .unwrap_or("center");
    let label = payload
      .get::<str>("label")
      .map(|s| s.to_string())
      .unwrap_or_default();

    let target = match section {
      "left" => &mut self.sections.left,
      "right" => &mut self.sections.right,
      _ => &mut self.sections.center,
    };

    target.retain(|i| i.label != label);
  }

  fn configure_from_payload(&mut self, payload: &HashMap<Ustr, Ustr>) {
    if let Some(h) = payload
      .get::<str>("height")
      .and_then(|s| s.parse::<u32>().ok())
    {
      self.height = h;
    }
    if let Some(pos) = payload
      .get::<str>("position")
      .and_then(|s| Position::from_str(s.as_str()))
    {
      self.position = pos;
    }
    if let Some(val) = payload.get::<str>("enabled") {
      self.enabled = val.as_str() != "false";
    }
  }

  fn render_item<'a>(item: &'a PanelItem) -> Element<'a, ItemMessage> {
    let label_text = if let Some(ref comp) = item.component {
      comp.as_str()
    } else {
      item.label.as_str()
    };

    container(text(label_text).size(13).color(Color::WHITE))
      .padding([4, 8])
      .style(|_t: &iced::Theme| container::Style {
        background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.06).into()),
        border: iced::Border {
          radius: 4.0.into(),
          ..Default::default()
        },
        ..container::Style::default()
      })
      .into()
  }

  fn render_section<'a>(items: &'a [PanelItem]) -> Element<'a, ItemMessage> {
    if items.is_empty() {
      return Space::new().into();
    }

    let children: Vec<Element<'a, ItemMessage>> = items.iter().map(Self::render_item).collect();
    row(children).spacing(4).into()
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
      exclusive_zone: Some(self.height as i32),
      size: Some((w, h)),
      margin: Some((0, 0, 0, 0)),
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
    let prefix = self.event_prefix();
    vec![
      EventFilter::Payload {
        name: format!("{}.add", prefix).into(),
        payload: None,
      },
      EventFilter::Payload {
        name: format!("{}.remove", prefix).into(),
        payload: None,
      },
      EventFilter::Payload {
        name: format!("{}.configure", prefix).into(),
        payload: None,
      },
      EventFilter::Named(format!("{}.destroy", prefix).into()),
      EventFilter::Tick,
    ]
  }

  fn update(&mut self, store: &mut Store, event: &ListenerAction) -> anyhow::Result<ItemEffect> {
    let prefix = self.event_prefix();

    Ok(match event {
      ListenerAction::Payload { name, payload } if name.as_str() == format!("{}.add", prefix) => {
        if let Some(p) = payload {
          self.add_item_from_payload(p);
        }
        ItemEffect::Redraw
      }
      ListenerAction::Payload { name, payload }
        if name.as_str() == format!("{}.remove", prefix) =>
      {
        if let Some(p) = payload {
          self.remove_from_payload(p);
        }
        ItemEffect::Redraw
      }
      ListenerAction::Payload { name, payload }
        if name.as_str() == format!("{}.configure", prefix) =>
      {
        if let Some(p) = payload {
          self.configure_from_payload(p);

          if let Some(panels) = store.borrow_mut::<PanelPositions>() {
            let height = self.height;
            panels.remove(name.as_str());
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
      ListenerAction::Named(name) if name.as_str() == format!("{}.destroy", prefix) => {
        if let Some(panels) = store.borrow_mut::<PanelPositions>() {
          panels.remove(name.as_str());
        }
        ItemEffect::ReallyDestroy
      }
      _ => ItemEffect::None,
    })
  }

  fn view(&self, _store: &Store, _id: IcedId) -> Element<'_, ItemMessage> {
    let left = Self::render_section(&self.sections.left);
    let center = Self::render_section(&self.sections.center);
    let right = Self::render_section(&self.sections.right);

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
        container(left)
          .height(Length::Fill)
          .align_top(Length::Shrink),
        container(center)
          .height(Length::Shrink)
          .center_y(Length::Shrink),
        container(right)
          .height(Length::Fill)
          .align_bottom(Length::Shrink),
      ]
      .align_x(iced::Alignment::Center)
      .width(Length::Fill)
      .height(Length::Fill)
      .into()
    };

    container(bar_content)
      .width(Length::Fill)
      .height(Length::Fill)
      .padding([0, 8])
      .style(|_t: &iced::Theme| container::Style {
        background: Some(Color::from_rgba(0.08, 0.08, 0.10, 0.4).into()),
        ..container::Style::default()
      })
      .into()
  }
}
