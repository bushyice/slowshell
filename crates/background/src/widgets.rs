use std::collections::HashMap;

use iced::{
  Alignment, Element, Length,
  widget::{container, text},
};
use iced_layershell::reexport::{
  Anchor, IcedId, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  types::{ToUstr, Ustr},
};
use slowshell_desktop::{
  DeployDesktopItemAction, DeployableDesktopItem, DesktopItem, EventFilter, ItemEffect,
  ItemMessage, MonitorScope, UpdateWhen, Visibility,
};
use slowshell_widgets::Renderables;

pub struct DesktopWidget {
  cached_id: Ustr,
  name: Ustr,
  inner: Option<Ustr>,
  inner_data: Option<Box<dyn std::any::Any + Send>>,
  width: u32,
  height: u32,
  margin_x: i32,
  margin_y: i32,
  anchor: WidgetAnchor,
  scope: MonitorScope,
  hidden: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetAnchor {
  TopLeft,
  TopRight,
  BottomLeft,
  BottomRight,
}

impl Default for WidgetAnchor {
  fn default() -> Self {
    WidgetAnchor::TopLeft
  }
}

impl WidgetAnchor {
  pub fn from_str(s: &str) -> Option<Self> {
    match s {
      "top-left" => Some(WidgetAnchor::TopLeft),
      "top-right" => Some(WidgetAnchor::TopRight),
      "bottom-left" => Some(WidgetAnchor::BottomLeft),
      "bottom-right" => Some(WidgetAnchor::BottomRight),
      _ => None,
    }
  }

  fn to_anchor(self) -> Anchor {
    match self {
      WidgetAnchor::TopLeft => Anchor::Top | Anchor::Left,
      WidgetAnchor::TopRight => Anchor::Top | Anchor::Right,
      WidgetAnchor::BottomLeft => Anchor::Bottom | Anchor::Left,
      WidgetAnchor::BottomRight => Anchor::Bottom | Anchor::Right,
    }
  }

  fn margins(self, x: i32, y: i32) -> (i32, i32, i32, i32) {
    match self {
      WidgetAnchor::TopLeft => (y, 0, 0, x),
      WidgetAnchor::TopRight => (y, x, 0, 0),
      WidgetAnchor::BottomLeft => (0, 0, y, x),
      WidgetAnchor::BottomRight => (0, x, y, 0),
    }
  }
}

impl DesktopWidget {
  pub fn new(name: impl Into<Ustr>) -> Self {
    let name = name.into();
    let cached_id = format!("widget/{}", name.to_lowercase().replace(' ', "-")).to_ustr();
    Self {
      cached_id,
      name,
      inner: None,
      inner_data: None,
      width: 200,
      height: 200,
      margin_x: 0,
      margin_y: 0,
      anchor: WidgetAnchor::default(),
      scope: MonitorScope::default(),
      hidden: false,
    }
  }

  pub fn with_size(mut self, width: u32, height: u32) -> Self {
    self.width = width;
    self.height = height;
    self
  }

  pub fn with_position(mut self, x: i32, y: i32) -> Self {
    self.margin_x = x;
    self.margin_y = y;
    self
  }

  pub fn with_anchor(mut self, anchor: WidgetAnchor) -> Self {
    self.anchor = anchor;
    self
  }

  pub fn with_scope(mut self, scope: MonitorScope) -> Self {
    self.scope = scope;
    self
  }

  pub fn with_renderable(mut self, inner: impl Into<Ustr>) -> Self {
    self.inner = Some(inner.into());
    self
  }

  fn event_prefix(&self) -> String {
    format!("widget.{}", self.name.to_lowercase().replace(' ', "-"))
  }
}

impl DesktopItem for DesktopWidget {
  fn id(&self) -> &str {
    &self.cached_id
  }

  fn layer(&self, _config: &Config, monitor: &str) -> NewLayerShellSettings {
    let margins = self.anchor.margins(self.margin_x, self.margin_y);
    NewLayerShellSettings {
      layer: Layer::Bottom,
      anchor: self.anchor.to_anchor(),
      exclusive_zone: Some(-1),
      size: Some((self.width, self.height)),
      margin: Some(margins),
      keyboard_interactivity: KeyboardInteractivity::None,
      events_transparent: false,
      namespace: Some(format!("slowshell-widget-{}", self.name.to_lowercase())),
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
      EventFilter::Named(format!("{}.destroy", prefix).into()),
      EventFilter::Named(format!("{}.hide", prefix).into()),
      EventFilter::Named(format!("{}.show", prefix).into()),
      EventFilter::Named(format!("{}.toggle", prefix).into()),
    ]
  }

  fn update(
    &mut self,
    config: &Config,
    store: &mut Store,
    event: &ListenerAction,
  ) -> anyhow::Result<ItemEffect> {
    let prefix = self.event_prefix();

    match event {
      ListenerAction::Named(name) if &**name == format!("{}.destroy", prefix) => {
        Ok(ItemEffect::ReallyDestroy)
      }
      ListenerAction::Named(name) if &**name == format!("{}.hide", prefix) => {
        self.hidden = true;
        Ok(ItemEffect::ReallyHide)
      }
      ListenerAction::Named(name) if &**name == format!("{}.show", prefix) => {
        self.hidden = false;
        Ok(ItemEffect::Show)
      }
      ListenerAction::Named(name) if &**name == format!("{}.toggle", prefix) => {
        if self.hidden {
          self.hidden = false;
          Ok(ItemEffect::Show)
        } else {
          self.hidden = true;
          Ok(ItemEffect::ReallyHide)
        }
      }
      _ => {
        if let Some(current) = &self.inner {
          let renderable = store
            .borrow_mut::<Renderables>()
            .and_then(|x| x.remove(current));

          if let Some(mut renderable) = renderable {
            if let Some(data) = self.inner_data.take() {
              if let Some(effect) = renderable.update(config, store, &*data) {
                return Ok(effect);
              }
            } else {
              self.inner_data = Some(renderable.initialize(config, store));
            }

            store
              .borrow_mut::<Renderables>()
              .unwrap()
              .insert(current.clone(), renderable);
          }
        }
        Ok(ItemEffect::None)
      }
    }
  }

  fn view<'a>(
    &'a self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    _monitor: &str,
  ) -> Element<'a, ItemMessage> {
    let bg = config.theme.base;
    let text_color = config.theme.text;

    let mut content: Element<'_, ItemMessage> = text(&*self.name).size(14).color(text_color).into();

    if let Some(current) = &self.inner {
      if let Some(renderable) = store.borrow::<Renderables>().and_then(|x| x.get(current)) {
        if let Some(data) = &self.inner_data {
          content = renderable.view(config, store, id, &*data).into();
        }
      }
    };

    container(content)
      .width(Length::Fill)
      .height(Length::Fill)
      .align_x(Alignment::Center)
      .align_y(Alignment::Center)
      .style(move |_theme: &iced::Theme| container::Style {
        background: Some(iced::Background::Color(iced::Color { a: 0.6, ..bg })),
        border: iced::Border {
          radius: 12.0.into(),
          ..Default::default()
        },
        ..Default::default()
      })
      .into()
  }
}

pub struct DesktopWidgetDeployer;

impl DeployableDesktopItem for DesktopWidgetDeployer {
  fn id(&self) -> &str {
    "widget-deployer"
  }

  fn deploys_on(&self) -> Vec<EventFilter> {
    vec![EventFilter::Payload("widget.create".into())]
  }

  fn deploy(
    &mut self,
    _config: &Config,
    _store: &mut Store,
    action: &ListenerAction,
  ) -> anyhow::Result<Option<DeployDesktopItemAction>> {
    let try_deploy = || {
      let (_name, _payload) = match action {
        ListenerAction::Payload { name, payload } if &**name == "widget.create" => {
          (name, payload.as_ref()?)
        }
        _ => return None,
      };

      // let widget_name = payload.get::<str>("name")?.to_string();

      // let width = payload
      //   .get::<str>("width")
      //   .and_then(|s| s.parse::<u32>().ok())
      //   .unwrap_or(200);

      // let height = payload
      //   .get::<str>("height")
      //   .and_then(|s| s.parse::<u32>().ok())
      //   .unwrap_or(200);

      // let x = payload
      //   .get::<str>("x")
      //   .and_then(|s| s.parse::<i32>().ok())
      //   .unwrap_or(0);

      // let y = payload
      //   .get::<str>("y")
      //   .and_then(|s| s.parse::<i32>().ok())
      //   .unwrap_or(0);

      // let anchor = payload
      //   .get::<str>("anchor")
      //   .and_then(|s| WidgetAnchor::from_str(&**s))
      //   .unwrap_or_default();

      // let scope = match payload.get::<str>("monitor").map(|s| &**s) {
      //   Some("*") => MonitorScope::PerMonitor,
      //   Some(name) => MonitorScope::Single(Some(name.into())),
      //   None => MonitorScope::default(),
      // };

      // let widget = DesktopWidget::new(widget_name)
      //   .with_size(width, height)
      //   .with_position(x, y)
      //   .with_anchor(anchor)
      //   .with_scope(scope);

      // Some(Box::new(widget) as Box<dyn DesktopItem>)
      None
    };

    Ok(try_deploy().map(|item| DeployDesktopItemAction::Deploy(item)))
  }
}

struct WidgetConfig {
  width: Option<u32>,
  height: Option<u32>,
  x: Option<i32>,
  y: Option<i32>,
  anchor: Option<WidgetAnchor>,
  monitor: Option<String>,
  inner: Option<String>,
}

type WidgetConfigs = HashMap<String, WidgetConfig>;

slowshell_registry::register_resources!(
  config: Unknown(Box::new(slowshell_config::ConfigParser {
    type_id: std::any::TypeId::of::<WidgetConfigs>(),
    de: |nodes| {
      let Some(widgets_node) = slowshell_config::find_node(nodes, "widgets") else {
        return Ok(None);
      };
      let mut widgets: WidgetConfigs = HashMap::new();
      for node in slowshell_config::node_children(widgets_node) {
        if slowshell_config::node_name(node) != "widget" {
          continue;
        }
        let Some(name) = slowshell_config::str_arg(node, 0) else {
          continue;
        };
        widgets.insert(
          name.to_string(),
          WidgetConfig {
            width: slowshell_config::child_i64(node, "width").map(|width| width as u32),
            height: slowshell_config::child_i64(node, "height").map(|width| width as u32),
            x: slowshell_config::child_i64(node, "x").map(|x| x as i32),
            y: slowshell_config::child_i64(node, "y").map(|y| y as i32),
            anchor: slowshell_config::child_str(node, "anchor").and_then(WidgetAnchor::from_str),
            monitor: slowshell_config::child_str_owned(node, "monitor"),
            inner: slowshell_config::child_str_owned(node, "inner"),
          },
        );
      }
      Ok(Some(Box::new(widgets)))
    },
  })),
  app: Item(|config, _| {
    let Some(widgets) = config.typed::<WidgetConfigs>() else {
      return Ok(vec![]);
    };

    Ok(
      widgets
        .iter()
        .map(|(name, conf)| {
          let mut widget = DesktopWidget::new(name.clone())
            .with_size(conf.width.unwrap_or(200), conf.height.unwrap_or(200))
            .with_position(conf.x.unwrap_or(0), conf.y.unwrap_or(0))
            .with_anchor(conf.anchor.unwrap_or_default());

          if let Some(inner) = &conf.inner {
            widget = widget.with_renderable(inner);
          }

          widget = widget.with_scope(match conf.monitor.as_deref() {
            Some("*") => MonitorScope::PerMonitor,
            Some(name) => MonitorScope::Single(Some(name.into())),
            None => MonitorScope::default(),
          });

          Box::new(widget) as Box<dyn DesktopItem + Send>
        })
        .collect(),
    )
  }),
  app: Deployable(|_, _| {
    Ok(vec![
      Box::new(DesktopWidgetDeployer) as Box<dyn DeployableDesktopItem>,
    ])
  }),
);
