use std::collections::HashMap;

use iced::{
  Alignment, Element, Length,
  widget::{container, text},
};
use iced_exwlshell::reexport::{
  Anchor, BlurOption, IcedId, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::layer_size,
  types::{ToUstr, Ustr},
};
use slowshell_desktop::{
  DeployDesktopItemAction, DeployableDesktopItem, DesktopItem, EventFilter, ItemEffect,
  ItemMessage, MonitorScope, UpdateWhen, Visibility,
};
use slowshell_widgets::{ComponentOptions, Renderables, truncate};

use crate::clock;

pub struct DesktopWidget {
  cached_id: Ustr,
  name: Ustr,
  inner: Option<Ustr>,
  inner_data: Option<Box<dyn std::any::Any + Send>>,
  options: Option<ComponentOptions>,
  width: u32,
  height: u32,
  margin_x: i32,
  margin_y: i32,
  root: bool,
  anchor: WidgetAnchor,
  layer: WidgetLayer,
  scope: MonitorScope,
  hidden: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetAnchor {
  TopLeft,
  TopCenter,
  TopRight,
  CenterLeft,
  Center,
  CenterRight,
  BottomLeft,
  BottomCenter,
  BottomRight,
}

impl Default for WidgetAnchor {
  fn default() -> Self {
    WidgetAnchor::TopLeft
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetLayer {
  Bottom,
  Top,
  Overlay,
}

impl Default for WidgetLayer {
  fn default() -> Self {
    WidgetLayer::Bottom
  }
}

impl WidgetLayer {
  pub fn from_str(s: &str) -> Option<Self> {
    match s {
      "bottom" => Some(WidgetLayer::Bottom),
      "top" => Some(WidgetLayer::Top),
      "overlay" => Some(WidgetLayer::Overlay),
      _ => None,
    }
  }

  fn to_layer(self) -> Layer {
    match self {
      WidgetLayer::Bottom => Layer::Bottom,
      WidgetLayer::Top => Layer::Top,
      WidgetLayer::Overlay => Layer::Overlay,
    }
  }
}

impl WidgetAnchor {
  pub fn from_str(s: &str) -> Option<Self> {
    match s {
      "top-left" => Some(WidgetAnchor::TopLeft),
      "top-center" | "top" => Some(WidgetAnchor::TopCenter),
      "top-right" => Some(WidgetAnchor::TopRight),
      "center-left" | "left" => Some(WidgetAnchor::CenterLeft),
      "center-center" | "center" => Some(WidgetAnchor::Center),
      "center-right" | "right" => Some(WidgetAnchor::CenterRight),
      "bottom-left" => Some(WidgetAnchor::BottomLeft),
      "bottom-center" | "bottom" => Some(WidgetAnchor::BottomCenter),
      "bottom-right" => Some(WidgetAnchor::BottomRight),
      _ => None,
    }
  }

  fn placement(
    self,
    monitor: Option<(u32, u32)>,
    width: u32,
    height: u32,
    x: i32,
    y: i32,
  ) -> (Anchor, (i32, i32, i32, i32)) {
    let clamp = |value: i32| value.max(0);
    let centered = |outer: u32, inner: u32, offset: i32| {
      let max = (outer as i32 - inner as i32).max(0);
      ((outer as i32 - inner as i32) / 2 + offset).clamp(0, max)
    };

    match self {
      WidgetAnchor::TopLeft => (Anchor::Top | Anchor::Left, (clamp(y), 0, 0, clamp(x))),
      WidgetAnchor::TopCenter => match monitor {
        Some((monitor_width, _)) => (
          Anchor::Top | Anchor::Left,
          (clamp(y), 0, 0, centered(monitor_width, width, x)),
        ),
        None => (Anchor::Top, (clamp(y), 0, 0, 0)),
      },
      WidgetAnchor::TopRight => (Anchor::Top | Anchor::Right, (clamp(y), clamp(x), 0, 0)),
      WidgetAnchor::CenterLeft => match monitor {
        Some((_, monitor_height)) => (
          Anchor::Top | Anchor::Left,
          (centered(monitor_height, height, y), 0, 0, clamp(x)),
        ),
        None => (Anchor::Left, (0, 0, 0, clamp(x))),
      },
      WidgetAnchor::Center => match monitor {
        Some((monitor_width, monitor_height)) => (
          Anchor::Top | Anchor::Left,
          (
            centered(monitor_height, height, y),
            0,
            0,
            centered(monitor_width, width, x),
          ),
        ),
        None => (Anchor::empty(), (0, 0, 0, 0)),
      },
      WidgetAnchor::CenterRight => match monitor {
        Some((_, monitor_height)) => (
          Anchor::Top | Anchor::Right,
          (centered(monitor_height, height, y), clamp(x), 0, 0),
        ),
        None => (Anchor::Right, (0, clamp(x), 0, 0)),
      },
      WidgetAnchor::BottomLeft => (Anchor::Bottom | Anchor::Left, (0, 0, clamp(y), clamp(x))),
      WidgetAnchor::BottomCenter => match monitor {
        Some((monitor_width, _)) => (
          Anchor::Bottom | Anchor::Left,
          (0, 0, clamp(y), centered(monitor_width, width, x)),
        ),
        None => (Anchor::Bottom, (0, 0, clamp(y), 0)),
      },
      WidgetAnchor::BottomRight => (Anchor::Bottom | Anchor::Right, (0, clamp(x), clamp(y), 0)),
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
      options: None,
      width: 200,
      height: 200,
      margin_x: 0,
      margin_y: 0,
      root: false,
      anchor: WidgetAnchor::default(),
      layer: WidgetLayer::default(),
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

  pub fn with_layer(mut self, layer: WidgetLayer) -> Self {
    self.layer = layer;
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

  pub fn with_options(mut self, options: Option<ComponentOptions>) -> Self {
    self.options = options;
    self
  }

  pub fn with_root(mut self, root: bool) -> Self {
    self.root = root;
    self
  }

  fn all_events(&self, store: &Store) -> Vec<EventFilter> {
    let mut events = self.init_events();

    if let Some(inner) = &self.inner
      && let Some(renderable) = store
        .borrow::<Renderables>()
        .and_then(|renderables| renderables.get(inner))
    {
      events.extend(renderable.events(store));
    }

    events
  }

  fn initialize_inner(&mut self, config: &Config, store: &mut Store) {
    if self.inner_data.is_some() {
      return;
    }

    let Some(current) = self.inner.clone() else {
      return;
    };

    let renderable = store
      .borrow_mut::<Renderables>()
      .and_then(|renderables| renderables.remove(&current));

    let Some(renderable) = renderable else {
      return;
    };

    self.inner_data =
      Some(renderable.initialize_with_options(config, store, self.options.as_ref()));

    if let Some(renderables) = store.borrow_mut::<Renderables>() {
      renderables.insert(current, renderable);
    }
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
    let (anchor, margins) = self.anchor.placement(
      slowshell_core::monitors::size(monitor),
      self.width,
      self.height,
      self.margin_x,
      self.margin_y,
    );

    NewLayerShellSettings {
      layer: self.layer.to_layer(),
      anchor,
      exclusive_zone: Some(-1),
      size: layer_size(self.width, self.height),
      margin: Some(margins),
      keyboard_interactivity: KeyboardInteractivity::None,
      events_transparent: false,
      namespace: Some(format!("slowshell-widget-{}", self.name.to_lowercase())),
      output_option: OutputOption::OutputName(monitor.to_string()),
      blur_option: BlurOption::None,
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
      EventFilter::StartUp,
    ]
  }

  fn update(
    &mut self,
    config: &Config,
    store: &mut Store,
    event: &ListenerAction,
  ) -> miette::Result<ItemEffect> {
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
      ListenerAction::StartUp => {
        self.initialize_inner(config, store);
        Ok(ItemEffect::Subscribe(self.all_events(store)))
      }
      _ => {
        self.initialize_inner(config, store);

        let Some(current) = self.inner.clone() else {
          return Ok(ItemEffect::None);
        };

        let renderable = store
          .borrow_mut::<Renderables>()
          .and_then(|x| x.remove(&current));

        let Some(mut renderable) = renderable else {
          return Ok(ItemEffect::None);
        };

        let data = self.inner_data.take();

        let effect = data.as_ref().and_then(|data| {
          renderable
            .on_event(config, store, event)
            .or_else(|| renderable.update(config, store, &**data))
        });

        self.inner_data = data;

        if let Some(renderables) = store.borrow_mut::<Renderables>() {
          renderables.insert(current, renderable);
        }

        Ok(effect.unwrap_or(ItemEffect::None))
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
    let bg = config.palette().base;
    let text_color = config.palette().text;

    let mut content: Element<'_, ItemMessage> = text(&*self.name).size(14).color(text_color).into();

    if let Some(current) = &self.inner {
      if let Some(renderable) = store.borrow::<Renderables>().and_then(|x| x.get(current)) {
        let data: &dyn std::any::Any = match &self.inner_data {
          Some(data) => &**data,
          None => &(),
        };

        content = renderable
          .view_with_options(config, store, id, data, self.options.as_ref())
          .into();
      }
    };

    if self.root {
      content
    } else {
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
  ) -> miette::Result<Option<DeployDesktopItemAction>> {
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
  layer: Option<WidgetLayer>,
  monitor: Option<String>,
  inner: Option<String>,
  options: Option<ComponentOptions>,
  root: bool,
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

        let mut options = slowshell_config::node_options_map(node);
        for key in [
          "width", "height", "x", "y", "anchor", "layer", "monitor", "inner",
        ] {
          options.remove(&key.to_ustr());
        }

        widgets.insert(
          name.to_string(),
          WidgetConfig {
            width: slowshell_config::child_i64(node, "width").map(|width| width as u32),
            height: slowshell_config::child_i64(node, "height").map(|width| width as u32),
            x: slowshell_config::child_i64(node, "x").map(|x| x as i32),
            y: slowshell_config::child_i64(node, "y").map(|y| y as i32),
            anchor: slowshell_config::child_str(node, "anchor").and_then(WidgetAnchor::from_str),
            layer: slowshell_config::child_str(node, "layer").and_then(WidgetLayer::from_str),
            monitor: slowshell_config::child_str_owned(node, "monitor"),
            inner: slowshell_config::child_str_owned(node, "inner"),
            options: (!options.is_empty()).then(|| ComponentOptions::from(options)),
            root: slowshell_config::child_bool(node, "root").unwrap_or_default(),
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
            .with_anchor(conf.anchor.unwrap_or_default())
            .with_layer(conf.layer.unwrap_or_default())
            .with_root(conf.root);

          if let Some(inner) = &conf.inner {
            widget = widget.with_renderable(inner);
          }

          widget = widget.with_options(conf.options.clone());

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

use std::{any::Any, time::Duration};

use iced::{
  Color,
  widget::{column, image, row},
};
use slowshell_commons::audio::{AUDIO_CHANGED, AudioCmd, SharedAudioState};
use slowshell_core::listeners::FdHandle;
use slowshell_widgets::{
  HoverBackground, Icon, Renderable, SystemMonOptions, clickable, mpris_cover_handle,
  render_system_mon_widget, set_sample_processes,
};

// `name`, `icon`, `label`, `size`, `label-size`, `color`, `show-label`
#[derive(Default)]
pub struct DesktopIconWidget {
  entries: std::sync::Mutex<HashMap<String, slowshell_commons::desktop::AppEntry>>,
}

impl DesktopIconWidget {
  fn resolve_entry(
    &self,
    store: &Store,
    name: &str,
  ) -> Option<slowshell_commons::desktop::AppEntry> {
    if let Ok(guard) = self.entries.lock()
      && let Some(entry) = guard.get(name)
    {
      return Some(entry.clone());
    }

    let entry = store
      .borrow::<slowshell_commons::desktop::DesktopEntries>()
      .and_then(|entries| entries.find(name));

    if let Some(entry) = &entry
      && let Ok(mut guard) = self.entries.lock()
    {
      guard.insert(name.to_string(), entry.clone());
    }

    entry
  }
}

impl Renderable for DesktopIconWidget {
  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    data: &'a dyn Any,
  ) -> Element<'a, ItemMessage> {
    self.view_with_options(config, store, id, data, None)
  }

  fn initialize_with_options(
    &self,
    _config: &Config,
    store: &mut Store,
    _options: Option<&ComponentOptions>,
  ) -> Box<dyn Any + Send> {
    slowshell_commons::desktop::DesktopEntries::initialize_unless(store);
    Box::new(())
  }

  fn events(&self, _store: &Store) -> Vec<EventFilter> {
    vec![
      EventFilter::Named(slowshell_commons::desktop::DESKTOP_ENTRIES_CHANGED.into()),
      EventFilter::Named("apps.reload".into()),
    ]
  }

  fn on_event(
    &mut self,
    _config: &Config,
    _store: &mut Store,
    event: &ListenerAction,
  ) -> Option<ItemEffect> {
    match event {
      ListenerAction::Signal { name, fd }
        if &**name == slowshell_commons::desktop::DESKTOP_ENTRIES_CHANGED =>
      {
        slowshell_services::util::drain_signal_fd(*fd);
        if let Ok(mut entries) = self.entries.lock() {
          entries.clear();
        }
        Some(ItemEffect::Redraw)
      }
      ListenerAction::Named(name)
        if &**name == slowshell_commons::desktop::DESKTOP_ENTRIES_CHANGED =>
      {
        Some(ItemEffect::Redraw)
      }
      ListenerAction::Named(name) if &**name == "apps.reload" => {
        if let Ok(mut entries) = self.entries.lock() {
          entries.clear();
        }
        Some(ItemEffect::Redraw)
      }
      _ => None,
    }
  }

  fn view_with_options<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    _id: IcedId,
    _data: &'a dyn Any,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let theme = config.palette();
    let size = options.and_then(|o| o.number("size")).unwrap_or(48.0) as u16;
    let label_size = options.and_then(|o| o.number("label-size")).unwrap_or(12.0);
    let show_label = options.and_then(|o| o.bool("show-label")).unwrap_or(true);

    let requested = options.and_then(|o| o.str("name"));
    let entry = requested.and_then(|name| self.resolve_entry(store, name));

    let icon_name = options
      .and_then(|o| o.str("icon"))
      .map(str::to_string)
      .or_else(|| entry.as_ref().and_then(|entry| entry.icon.clone()));

    let label = options
      .and_then(|o| o.str("label"))
      .map(str::to_string)
      .or_else(|| entry.as_ref().map(|entry| entry.name.clone()))
      .or_else(|| requested.map(str::to_string));

    let icon = match icon_name {
      Some(name) => Icon::any([name, "application-x-executable".to_string()]),
      None => match options.and_then(|o| o.strings("icons")) {
        Some(names) => Icon::any(names),
        None => Icon::new("application-x-executable"),
      },
    };

    let icon = match options
      .and_then(|o| o.str("color"))
      .and_then(|name| theme.color(name))
    {
      Some(color) => icon.size(size).color(color).into_element(),
      None => icon.size(size).into_element(),
    };

    let mut content = column![icon].spacing(4).align_x(Alignment::Center);

    if show_label && let Some(label) = label {
      content = content.push(text(label).size(label_size).color(theme.text));
    }

    let content: Element<'a, ItemMessage> = content.into();

    match entry {
      Some(entry) => {
        let exec = entry.exec.clone();
        let hovered = HoverBackground::new(
          content,
          Color {
            a: 0.18,
            ..theme.primary
          },
          8.0,
        );

        clickable(hovered.into(), move |_, _, _| {
          slowshell_commons::desktop::spawn_exec(&exec);
          None
        })
      }
      None => content,
    }
  }
}

// `format`, `format-2`, `interval`, `size`, `analog`,
// `label-size`, `color`, `color-2`, `label`
pub struct ClockWidget {
  hand_cache: clock::HandCache,
}

impl ClockWidget {
  pub fn new() -> Self {
    Self {
      hand_cache: clock::HandCache::default(),
    }
  }
}

impl Default for ClockWidget {
  fn default() -> Self {
    Self::new()
  }
}

const CLOCK_TICK: &str = "widgets/clock.tick";

impl Renderable for ClockWidget {
  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    data: &'a dyn Any,
  ) -> Element<'a, ItemMessage> {
    self.view_with_options(config, store, id, data, None)
  }

  fn initialize_with_options(
    &self,
    _config: &Config,
    store: &mut Store,
    options: Option<&ComponentOptions>,
  ) -> Box<dyn Any + Send> {
    let format = options.and_then(|o| o.str("format")).unwrap_or("%H:%M");
    let format_2 = options.and_then(|o| o.str("format-2")).unwrap_or_default();
    let interval = options
      .and_then(|o| o.int("interval"))
      .map(|interval| interval.clamp(1, 3600) as u64)
      .unwrap_or(if format.contains("%S") || format_2.contains("%S") {
        1
      } else {
        60
      });

    if let Some(handle) = store.borrow::<FdHandle>() {
      let _ = handle.set_named_interval(Duration::from_secs(interval), CLOCK_TICK);
    }

    Box::new(())
  }

  fn events(&self, _store: &Store) -> Vec<EventFilter> {
    vec![EventFilter::Named(CLOCK_TICK.into())]
  }

  fn on_event(
    &mut self,
    _config: &Config,
    _store: &mut Store,
    event: &ListenerAction,
  ) -> Option<ItemEffect> {
    match event {
      ListenerAction::Timer { name, .. } if &**name == CLOCK_TICK => Some(ItemEffect::Redraw),
      ListenerAction::Named(name) if &**name == CLOCK_TICK => Some(ItemEffect::Redraw),
      _ => None,
    }
  }

  fn view_with_options<'a>(
    &self,
    config: &'a Config,
    _store: &'a Store,
    _id: IcedId,
    _data: &'a dyn Any,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    if options
      .and_then(|options| options.bool("analog"))
      .unwrap_or(false)
    {
      return clock::view(config, options, &self.hand_cache);
    }

    let theme = config.palette();
    let format = options.and_then(|o| o.str("format")).unwrap_or("%H:%M");
    let size = options.and_then(|o| o.number("size")).unwrap_or(28.0);
    let label_size = options.and_then(|o| o.number("label-size")).unwrap_or(11.0);

    let color = options
      .and_then(|o| o.str("color"))
      .and_then(|name| theme.color(name))
      .unwrap_or(theme.text);

    let now = chrono::Local::now();
    let value = now.format(format).to_string();

    let mut content = column![text(value).size(size).color(color)]
      .spacing(2)
      .align_x(Alignment::Center);

    if let Some(format_2) = options.and_then(|o| o.str("format-2")) {
      let color_2 = options
        .and_then(|o| o.str("color-2"))
        .and_then(|name| theme.color(name))
        .unwrap_or(theme.subtext);

      content = content.push(
        text(now.format(format_2).to_string())
          .size(label_size)
          .color(color_2),
      );
    } else if let Some(label) = options.and_then(|o| o.str("label")) {
      content = content.push(
        text(label.to_string())
          .size(label_size)
          .color(theme.subtext),
      );
    }

    content.into()
  }
}

// `cover`, `cover-size`, `show-artist`, `controls`, `max-length`
pub struct NowplayingWidget;

impl Renderable for NowplayingWidget {
  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    data: &'a dyn Any,
  ) -> Element<'a, ItemMessage> {
    self.view_with_options(config, store, id, data, None)
  }

  fn initialize_with_options(
    &self,
    _config: &Config,
    store: &mut Store,
    _options: Option<&ComponentOptions>,
  ) -> Box<dyn Any + Send> {
    #[cfg(feature = "audio")]
    {
      slowshell_services::audio::ensure(store);
    }

    #[cfg(not(feature = "audio"))]
    {
      let _ = store;
    }

    Box::new(())
  }

  fn events(&self, _store: &Store) -> Vec<EventFilter> {
    vec![EventFilter::Named(AUDIO_CHANGED.into())]
  }

  fn on_event(
    &mut self,
    _config: &Config,
    _store: &mut Store,
    event: &ListenerAction,
  ) -> Option<ItemEffect> {
    match event {
      ListenerAction::Signal { name, fd } if &**name == AUDIO_CHANGED => {
        slowshell_services::util::drain_signal_fd(*fd);
        Some(ItemEffect::Redraw)
      }
      ListenerAction::Named(name) if &**name == AUDIO_CHANGED => Some(ItemEffect::Redraw),
      _ => None,
    }
  }

  fn view_with_options<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    _id: IcedId,
    _data: &'a dyn Any,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let theme = config.palette();

    let Some(shared) = store.borrow::<SharedAudioState>() else {
      return placeholder(config, "Audio unavailable");
    };

    let player = shared.player.lock().unwrap().clone();

    let Some(player) = player else {
      return placeholder(config, "Nothing playing");
    };

    let show_cover = options.and_then(|o| o.bool("cover")).unwrap_or(true);
    let cover_size = options
      .and_then(|o| o.int("cover-size"))
      .unwrap_or(72)
      .clamp(24, 256) as f32;
    let show_artist = options.and_then(|o| o.bool("show-artist")).unwrap_or(true);
    let show_controls = options.and_then(|o| o.bool("controls")).unwrap_or(true);
    let max_length = options
      .and_then(|o| o.int("max-length"))
      .unwrap_or(32)
      .clamp(4, 200) as usize;

    let cover = if show_cover {
      player.art_url.as_deref().and_then(mpris_cover_handle)
    } else {
      None
    };

    let mut lines = column![].spacing(2);

    if !player.title.is_empty() {
      lines = lines.push(
        text(truncate(&player.title, max_length))
          .size(14)
          .color(theme.text),
      );
    }

    if show_artist && !player.artist.is_empty() {
      lines = lines.push(
        text(truncate(&player.artist, max_length))
          .size(12)
          .color(theme.subtext),
      );
    }

    let media: Element<'a, ItemMessage> = match cover {
      Some(handle) => row![
        image(handle)
          .width(Length::Fixed(cover_size))
          .height(Length::Fixed(cover_size))
          .border_radius(8.0),
        lines,
      ]
      .spacing(10)
      .align_y(Alignment::Center)
      .into(),
      None => lines.into(),
    };

    let mut content = column![media]
      .spacing(8)
      .align_x(Alignment::Center)
      .width(Length::Fill);

    if show_controls {
      let previous_color = if player.can_go_previous {
        theme.text
      } else {
        theme.subtext
      };
      let next_color = if player.can_go_next {
        theme.text
      } else {
        theme.subtext
      };
      let play_icon = if player.playback_status == "Playing" {
        "media-playback-pause-symbolic"
      } else {
        "media-playback-start-symbolic"
      };

      let controls = row![
        audio_button(
          &shared,
          "media-skip-backward-symbolic",
          14,
          previous_color,
          AudioCmd::Previous,
        ),
        audio_button(&shared, play_icon, 18, theme.primary, AudioCmd::PlayPause),
        audio_button(
          &shared,
          "media-skip-forward-symbolic",
          14,
          next_color,
          AudioCmd::Next,
        ),
      ]
      .spacing(12)
      .align_y(Alignment::Center);

      content = content.push(
        container(controls)
          .align_x(Alignment::Center)
          .width(Length::Fill),
      );
    }

    content.into()
  }
}

// `interval`, `processes`, `max-processes`, `temp`.
pub struct SysmonWidget;

const SYSMON_TICK: &str = "widgets/sysmon.tick";

impl Renderable for SysmonWidget {
  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    data: &'a dyn Any,
  ) -> Element<'a, ItemMessage> {
    self.view_with_options(config, store, id, data, None)
  }

  fn initialize_with_options(
    &self,
    _config: &Config,
    store: &mut Store,
    options: Option<&ComponentOptions>,
  ) -> Box<dyn Any + Send> {
    let interval = options
      .and_then(|o| o.int("interval"))
      .unwrap_or(5)
      .clamp(1, 3600) as u64;

    slowshell_services::system::ensure(store, Duration::from_secs(interval));

    if options.and_then(|o| o.bool("processes")) == Some(true) {
      set_sample_processes(store, true);
    }

    if let Some(handle) = store.borrow::<FdHandle>() {
      let _ = handle.set_named_interval(Duration::from_secs(interval), SYSMON_TICK);
    }

    Box::new(())
  }

  fn events(&self, _store: &Store) -> Vec<EventFilter> {
    vec![EventFilter::Named(SYSMON_TICK.into())]
  }

  fn on_event(
    &mut self,
    _config: &Config,
    _store: &mut Store,
    event: &ListenerAction,
  ) -> Option<ItemEffect> {
    match event {
      ListenerAction::Timer { name, .. } if &**name == SYSMON_TICK => Some(ItemEffect::Redraw),
      ListenerAction::Named(name) if &**name == SYSMON_TICK => Some(ItemEffect::Redraw),
      _ => None,
    }
  }

  fn close(&mut self, store: &mut Store) {
    set_sample_processes(store, false);
  }

  fn view_with_options<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    _id: IcedId,
    _data: &'a dyn Any,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let options = SystemMonOptions {
      show_processes: options.and_then(|o| o.bool("processes")).unwrap_or(false),
      max_processes: options
        .and_then(|o| o.int("max-processes"))
        .unwrap_or(8)
        .clamp(1, 64) as usize,
      show_temp: options.and_then(|o| o.bool("temp")).unwrap_or(true),
    };

    render_system_mon_widget(config, store, options)
  }
}

fn audio_button(
  shared: &SharedAudioState,
  icon: &'static str,
  size: u16,
  color: Color,
  command: AudioCmd,
) -> Element<'static, ItemMessage> {
  let cmd_tx = shared.cmd_tx.clone();

  clickable(
    Icon::new(icon).size(size).color(color).into(),
    move |_, _, _| {
      let _ = cmd_tx.send(command.clone());
      None
    },
  )
}

fn placeholder<'a>(config: &'a Config, message: &str) -> Element<'a, ItemMessage> {
  container(
    text(message.to_string())
      .size(13)
      .color(config.palette().subtext),
  )
  .width(Length::Fill)
  .height(Length::Fill)
  .align_x(Alignment::Center)
  .align_y(Alignment::Center)
  .into()
}
