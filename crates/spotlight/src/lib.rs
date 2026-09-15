pub mod apps;
pub mod calc;
pub mod clipboard;

use std::{
  any::TypeId,
  cell::RefCell,
  collections::{HashMap, HashSet},
  path::PathBuf,
  process::{Command, Stdio},
  sync::{Arc, Mutex},
};

use iced::{
  Alignment, Element, Event, Length, keyboard,
  widget::{center, column, container, row, space, text},
};
use iced_layershell::reexport::{
  Anchor, IcedId, KeyboardInteractivity, Layer, NewLayerShellSettings,
};
use slowshell_commons::desktop::DesktopEntries;
use slowshell_config::{Config, ConfigParser};
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  types::{OptionalPayloadBox, PayloadBox, PayloadBuilder, Ustr},
};
use slowshell_desktop::{
  DesktopItem, EventFilter, ItemEffect, ItemMessage, MonitorScope, UpdateWhen, Visibility,
};
use slowshell_widgets::{EventWrapper, Icon, clickable};

const ROW_HEIGHT: f32 = 52.0;
const LIST_HEIGHT: f32 = 440.0;
const WINDOW_LEN: usize = (LIST_HEIGHT / ROW_HEIGHT) as usize;

const GRID_COLUMNS: usize = 4;
const GRID_TILE_HEIGHT: f32 = 96.0;
const GRID_PAGE: usize = GRID_COLUMNS * 4;

const IMAGE_COLUMNS: usize = 2;
const IMAGE_TILE_HEIGHT: f32 = 204.0;
const IMAGE_PAGE: usize = IMAGE_COLUMNS * 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DisplayStyle {
  List,
  Grid,
  ImageList,
}

impl DisplayStyle {
  pub fn parse(value: &str) -> Option<Self> {
    match value {
      "list" => Some(Self::List),
      "grid" => Some(Self::Grid),
      "image-list" | "image_list" | "imagelist" => Some(Self::ImageList),
      _ => None,
    }
  }

  pub fn name(self) -> &'static str {
    match self {
      Self::List => "list",
      Self::Grid => "grid",
      Self::ImageList => "image-list",
    }
  }

  fn page(self) -> usize {
    match self {
      Self::List => WINDOW_LEN,
      Self::Grid => GRID_PAGE,
      Self::ImageList => IMAGE_PAGE,
    }
  }

  fn columns(self) -> usize {
    match self {
      Self::List => 1,
      Self::Grid => GRID_COLUMNS,
      Self::ImageList => IMAGE_COLUMNS,
    }
  }

  fn tile_height(self) -> f32 {
    match self {
      Self::List => ROW_HEIGHT,
      Self::Grid => GRID_TILE_HEIGHT,
      Self::ImageList => IMAGE_TILE_HEIGHT,
    }
  }
}

#[derive(Clone)]
pub enum SpotlightAction {
  Exec(String),
  Copy(String),
  Clip(String),
  Expand(PathBuf),
  Custom(Arc<dyn Fn() + Send + Sync>),
}

#[derive(Clone)]
pub struct SpotlightRequest {
  pub mode: Ustr,
  pub style: Option<DisplayStyle>,
}

#[derive(Clone)]
pub struct SpotlightActionDef {
  pub title: Option<String>,
  pub action: SpotlightAction,
}

pub struct SpotlightItem {
  pub image: Option<PathBuf>,
  pub image_bytes: Option<Vec<u8>>,
  pub image_icon: Option<String>,
  pub title: String,
  pub subtitle: Option<String>,
  pub subtext: Option<String>,
  pub tags: Option<Vec<String>>,
  pub actions: Vec<SpotlightActionDef>,
}

impl SpotlightActionDef {
  pub fn label(&self, index: usize) -> String {
    self
      .title
      .clone()
      .unwrap_or_else(|| format!("Action {}", index + 1))
  }
}

pub enum SpotlightKind {
  Generate(Box<dyn Fn(&str, &Store) -> Vec<SpotlightItem> + Send + Sync>),
  SingleResult(Box<dyn Fn(&str, &Store) -> Option<SpotlightItem> + Send + Sync>),
}

pub struct SpotlightMode {
  pub kind: SpotlightKind,
  pub display: &'static [DisplayStyle],
}

impl SpotlightMode {
  pub fn generate(
    generate: impl Fn(&str, &Store) -> Vec<SpotlightItem> + Send + Sync + 'static,
  ) -> Self {
    Self {
      kind: SpotlightKind::Generate(Box::new(generate)),
      display: &[DisplayStyle::List],
    }
  }

  pub fn single(
    evaluate: impl Fn(&str, &Store) -> Option<SpotlightItem> + Send + Sync + 'static,
  ) -> Self {
    Self {
      kind: SpotlightKind::SingleResult(Box::new(evaluate)),
      display: &[DisplayStyle::List],
    }
  }

  pub fn display(mut self, display: &'static [DisplayStyle]) -> Self {
    self.display = display;
    self
  }
}

pub struct Trigger {
  pub target_mode: Ustr,
  pub check: Box<dyn Fn(&str) -> bool + Send + Sync>,
}

pub struct SpotlightModes {
  pub modes: HashMap<Ustr, SpotlightMode>,
  pub allow_triggers: HashSet<Ustr>,
  pub triggers: Vec<Trigger>,
  cache_enabled: bool,
  results: RefCell<HashMap<Ustr, (u64, Arc<Vec<SpotlightItem>>)>>,
  versions: HashMap<Ustr, Box<dyn Fn() -> u64 + Send + Sync>>,
}

impl SpotlightModes {
  fn new() -> Self {
    Self {
      modes: HashMap::new(),
      allow_triggers: HashSet::new(),
      triggers: Vec::new(),
      cache_enabled: false,
      results: RefCell::new(HashMap::new()),
      versions: HashMap::new(),
    }
  }
}

impl Default for SpotlightModes {
  fn default() -> Self {
    let mut modes = Self::new();

    modes.register_mode(
      "applications",
      SpotlightMode::generate(apps::search_applications)
        .display(&[DisplayStyle::List, DisplayStyle::Grid]),
      true,
    );
    modes.register_version(
      "applications",
      slowshell_commons::desktop::DesktopEntries::global_version,
    );

    modes.register_mode(
      "clipboard",
      SpotlightMode::generate(clipboard::search_clipboard),
      false,
    );
    modes.register_version("clipboard", clipboard::version);

    let calc_mode: Ustr = "calculator".into();
    modes.register_mode(
      calc_mode.clone(),
      SpotlightMode::single(|q, _| {
        if let Ok(val) = calc::eval(q) {
          let formatted = calc::format_result(val);
          Some(SpotlightItem {
            image: None,
            image_bytes: None,
            image_icon: Some("accessories-calculator-symbolic".into()),
            title: format!("= {formatted}"),
            subtitle: Some(q.trim().to_string()),
            subtext: None,
            tags: Some(vec!["Calculator".into()]),
            actions: vec![SpotlightActionDef {
              title: Some("Copy".into()),
              action: SpotlightAction::Copy(formatted),
            }],
          })
        } else {
          None
        }
      }),
      false,
    );

    modes.triggers.push(Trigger {
      target_mode: calc_mode,
      check: Box::new(|q| calc::is_math_expression(q)),
    });

    modes
  }
}

impl SpotlightModes {
  pub fn configure(&mut self, config: &Config) {
    self.cache_enabled = config
      .typed::<SpotlightConfig>()
      .map(|c| {
        if c.cliphist {
          let _ = Command::new("wl-paste")
            .args(["--watch", "cliphist", "store"])
            .spawn();
        }
        c.cache
      })
      .unwrap_or(false);
  }

  pub fn precache(&self, store: &Store) {
    if !self.cache_enabled {
      return;
    }
    for name in self
      .modes
      .keys()
      .filter(|name| {
        matches!(
          self.modes.get(*name).map(|mode| &mode.kind),
          Some(SpotlightKind::Generate(_))
        )
      })
      .cloned()
      .collect::<Vec<Ustr>>()
    {
      let _ = self.generate(&name, "", store);
    }
  }

  pub fn generate(&self, name: &Ustr, query: &str, store: &Store) -> Arc<Vec<SpotlightItem>> {
    if !self.cache_enabled || !query.is_empty() {
      return Arc::new(self.generate_inner(name, query, store));
    }

    let version = self.versions.get(name).map(|f| f()).unwrap_or(0);
    if let Some((cached_version, items)) = self.results.borrow().get(name)
      && *cached_version == version
      && !items.is_empty()
    {
      return Arc::clone(items);
    }

    let items = Arc::new(self.generate_inner(name, query, store));
    if !items.is_empty() || version > 0 {
      self
        .results
        .borrow_mut()
        .insert(name.clone(), (version, Arc::clone(&items)));
    }
    items
  }

  fn generate_inner(&self, name: &Ustr, query: &str, store: &Store) -> Vec<SpotlightItem> {
    match self.modes.get(name).map(|mode| &mode.kind) {
      Some(SpotlightKind::Generate(generator)) => generator(query, store),
      Some(SpotlightKind::SingleResult(evaluator)) => evaluator(query, store).into_iter().collect(),
      None => Vec::new(),
    }
  }

  pub fn register_mode(&mut self, name: impl Into<Ustr>, mode: SpotlightMode, allow_trigger: bool) {
    let name = name.into();
    if allow_trigger {
      self.allow_triggers.insert(name.clone());
    }
    self.modes.insert(name, mode);
  }

  pub fn register_version(
    &mut self,
    name: impl Into<Ustr>,
    version: impl Fn() -> u64 + Send + Sync + 'static,
  ) {
    let name = name.into();
    self.versions.insert(name, Box::new(version));
  }

  pub fn register_trigger(
    &mut self,
    target_mode: impl Into<Ustr>,
    check: impl Fn(&str) -> bool + Send + Sync + 'static,
  ) {
    self.triggers.push(Trigger {
      target_mode: target_mode.into(),
      check: Box::new(check),
    });
  }
}

struct SpotlightInner {
  search: String,
  selected_index: usize,
  action_index: usize,
  display_index: usize,
  requested_style: Option<DisplayStyle>,
  expanded: Option<PathBuf>,
  should_close: bool,
  current_mode: Ustr,
  base_mode: Ustr,
  pending_action: Option<SpotlightAction>,
  visible_start_idx: usize,
  visible_end_idx: usize,
}

pub struct Spotlight {
  inner: Arc<Mutex<SpotlightInner>>,
  shown: bool,
}

fn scroll_target(selected: usize, start: usize, end: usize, page: usize) -> Option<usize> {
  if selected < start {
    Some(selected)
  } else if selected >= end {
    Some(selected + 1 - page.min(selected + 1))
  } else {
    None
  }
}

impl Spotlight {
  pub fn new() -> Self {
    Self {
      inner: Arc::new(Mutex::new(SpotlightInner {
        search: String::new(),
        selected_index: 0,
        action_index: 0,
        display_index: 0,
        requested_style: None,
        expanded: None,
        should_close: false,
        current_mode: "applications".into(),
        base_mode: "applications".into(),
        pending_action: None,
        visible_start_idx: 0,
        visible_end_idx: WINDOW_LEN,
      })),
      shown: false,
    }
  }

  fn execute_action(action: SpotlightAction) {
    match action {
      SpotlightAction::Exec(cmd) => {
        apps::spawn_app(&cmd);
      }
      SpotlightAction::Copy(text) => {
        copy_to_clipboard(&text);
      }
      SpotlightAction::Clip(line) => {
        clipboard::copy(&line);
      }
      SpotlightAction::Expand(_) => {}
      SpotlightAction::Custom(f) => {
        f();
      }
    }
  }

  fn toggle(&mut self, request: Option<&SpotlightRequest>) -> miette::Result<ItemEffect> {
    if self.shown {
      self.close();
      self.shown = false;
      Ok(ItemEffect::Hide)
    } else {
      self.open(request);
      self.shown = true;
      Ok(ItemEffect::Show)
    }
  }

  fn open(&self, request: Option<&SpotlightRequest>) {
    let mut inner = self.inner.lock().unwrap();
    inner.search.clear();
    inner.selected_index = 0;
    inner.action_index = 0;
    inner.display_index = 0;
    inner.requested_style = request.and_then(|request| request.style);
    inner.expanded = None;
    inner.visible_start_idx = 0;
    inner.visible_end_idx = WINDOW_LEN;
    inner.should_close = false;
    let mode = request
      .map(|request| request.mode.clone())
      .unwrap_or_else(|| "applications".into());
    inner.current_mode = mode.clone();
    inner.base_mode = mode;
    inner.pending_action = None;
  }

  fn close(&self) {
    let mut inner = self.inner.lock().unwrap();
    inner.current_mode = "applications".into();
    inner.base_mode = "applications".into();
    inner.search.clear();
    inner.selected_index = 0;
    inner.action_index = 0;
    inner.display_index = 0;
    inner.requested_style = None;
    inner.expanded = None;
  }
}

fn copy_to_clipboard(text: &str) {
  if let Ok(mut child) = Command::new("wl-copy")
    .stdin(Stdio::piped())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
  {
    if let Some(mut stdin) = child.stdin.take() {
      use std::io::Write;
      let _ = stdin.write_all(text.as_bytes());
    }
    let _ = child.wait();
  }
}

enum ModeResult {
  List(Arc<Vec<SpotlightItem>>),
  Single(Option<SpotlightItem>),
}

impl DesktopItem for Spotlight {
  fn id(&self) -> &str {
    "spotlight"
  }

  fn layer(&self, _config: &Config, _monitor: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
      layer: Layer::Overlay,
      anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
      exclusive_zone: Some(-1),
      size: None,
      margin: Some((0, 0, 0, 0)),
      keyboard_interactivity: KeyboardInteractivity::Exclusive,
      events_transparent: false,
      namespace: Some("slowshell-spotlight".into()),
      ..Default::default()
    }
  }

  fn visibility(&self) -> Visibility {
    Visibility::Transient
  }

  fn update_strategy(&self) -> UpdateWhen {
    UpdateWhen::OnEvent
  }

  fn monitor(&self, _config: &Config) -> MonitorScope {
    MonitorScope::Single(None)
  }

  fn init_events(&self) -> Vec<EventFilter> {
    vec![
      EventFilter::Named("spotlight.toggle".into()),
      EventFilter::Named("spotlight.close".into()),
      EventFilter::Payload("spotlight.open".into()),
      EventFilter::Payload("spotlight.toggle".into()),
      EventFilter::Named("config.reload".into()),
    ]
  }

  fn update(
    &mut self,
    config: &Config,
    store: &mut Store,
    event: &ListenerAction,
  ) -> miette::Result<ItemEffect> {
    match event {
      ListenerAction::Named(name)
      | ListenerAction::Signal { name, .. }
      | ListenerAction::Timer { name, .. }
        if name.as_ref() == "config.reload" =>
      {
        if let Some(modes) = store.borrow_mut::<SpotlightModes>() {
          modes.configure(config);
        }
        Ok(ItemEffect::Redraw)
      }
      ListenerAction::Payload { name, payload } if name.as_ref() == "spotlight.toggle" => {
        self.toggle(payload.transform::<SpotlightRequest>())
      }
      ListenerAction::Named(name) if name.as_ref() == "spotlight.toggle" => self.toggle(None),
      ListenerAction::Named(name) if name.as_ref() == "spotlight.close" => {
        self.close();
        self.shown = false;
        Ok(ItemEffect::Hide)
      }
      ListenerAction::Payload { name, payload } if name.as_ref() == "spotlight.open" => {
        self.open(payload.transform::<SpotlightRequest>());
        self.shown = true;
        Ok(ItemEffect::Show)
      }
      _ => Ok(ItemEffect::None),
    }
  }

  fn handle_message(&mut self, _store: Option<&mut Store>, message: &ItemMessage) -> ItemEffect {
    match message {
      ItemMessage::Effect(_, ItemEffect::Redraw) => {
        let mut inner = self.inner.lock().unwrap();
        if inner.should_close {
          if let Some(action) = inner.pending_action.take() {
            Self::execute_action(action);
          }
          drop(inner);
          self.close();
          self.shown = false;
          return ItemEffect::Hide;
        }
        return ItemEffect::Redraw;
      }
      ItemMessage::Effect(_, ItemEffect::Custom([0, start, end, _])) => {
        let mut inner = self.inner.lock().unwrap();
        inner.visible_start_idx = *start;
        inner.visible_end_idx = *end;
        return ItemEffect::Redraw;
      }
      ItemMessage::Effect(_, ItemEffect::Custom([2, index, _, _])) => {
        let mut inner = self.inner.lock().unwrap();
        inner.selected_index = *index;
        return ItemEffect::Redraw;
      }
      _ => {}
    }
    ItemEffect::None
  }

  fn view(
    &self,
    config: &Config,
    store: &Store,
    id: IcedId,
    _monitor: &str,
  ) -> Element<'static, ItemMessage> {
    let Some(modes) = store.borrow::<SpotlightModes>() else {
      return space().into();
    };

    let style = config.style("spotlight");
    let theme = &config.theme;

    let (
      current_mode,
      search,
      selected,
      action_index,
      win_start,
      mut display_index,
      requested_style,
      expanded,
    ) = {
      let mut inner = self.inner.lock().unwrap();
      (
        inner.current_mode.clone(),
        inner.search.clone(),
        inner.selected_index,
        inner.action_index,
        inner.visible_start_idx,
        inner.display_index,
        inner.requested_style.take(),
        inner.expanded.clone(),
      )
    };

    let allowed: &'static [DisplayStyle] = modes
      .modes
      .get(&current_mode)
      .map(|mode| mode.display)
      .unwrap_or(&[DisplayStyle::List]);

    if let Some(style) = requested_style {
      display_index = allowed.iter().position(|item| *item == style).unwrap_or(0);
    }
    if display_index >= allowed.len() {
      display_index = 0;
    }
    if let Ok(mut inner) = self.inner.lock() {
      inner.display_index = display_index;
    }
    let display = allowed
      .get(display_index)
      .copied()
      .unwrap_or(DisplayStyle::List);
    let page = display.page();

    if let Some(path) = expanded {
      return expanded_view(self.inner.clone(), id, path, &style, theme);
    }

    let mode_result = match modes.modes.get(&current_mode).map(|mode| &mode.kind) {
      Some(SpotlightKind::Generate(_)) => {
        ModeResult::List(modes.generate(&current_mode, &search, store))
      }
      Some(SpotlightKind::SingleResult(evaluator)) => ModeResult::Single(evaluator(&search, store)),
      None => ModeResult::List(Arc::new(Vec::new())),
    };

    let search_font = style.number("search.font.size").unwrap_or(22.0);
    let search_display = if search.is_empty() {
      container(
        text(format!("Search {}...", current_mode))
          .size(search_font)
          .color(style.color(theme, "search.placeholder.color", theme.overlay)),
      )
    } else {
      container(
        text(format!("{search}|"))
          .size(search_font)
          .color(style.color(theme, "search.color", theme.text)),
      )
    };

    let search_bg = style.color(theme, "search.background", theme.mantle);
    let radius = style.number("radius").unwrap_or(12.0);
    let border_color = style.color(theme, "border.color", theme.overlay);
    let border_width = style.number("border.width").unwrap_or(1.0);
    let padding = style.number("padding").unwrap_or(14.0);

    let search_bar = container(search_display)
      .width(Length::Fill)
      .padding(padding)
      .style(move |_t| container::Style {
        background: Some(search_bg.into()),
        border: iced::Border {
          radius: radius.into(),
          width: border_width,
          color: border_color,
        },
        ..container::Style::default()
      });

    let result_bg = style.color(theme, "row.background", theme.mantle);
    let selected_bg = style.color(theme, "row.selected", theme.primary);
    let result_color = style.color(theme, "result.color", theme.text);
    let result_font = style.number("result.font.size").unwrap_or(15.0);
    let row_padding = style.number("row.padding").unwrap_or(10.0);
    let spacing = style.number("spacing").unwrap_or(8.0);
    let mantle_bg = theme.mantle;
    let subtext_color = style.color(theme, "subtitle.color", theme.subtext);
    let hint_color = style.color(theme, "hint.color", theme.overlay);
    let tag_color = style.color(theme, "tag.color", theme.subtext);
    let accent_color = style.color(theme, "color.accent", theme.primary);
    let single_font_size = style.number("single.font.size").unwrap_or(32.0);
    let single_padding = style.number("single.padding").unwrap_or(16.0);

    let mut actions: Vec<Vec<SpotlightActionDef>> = Vec::new();
    let body: Element<'static, ItemMessage>;

    match mode_result {
      ModeResult::Single(Some(item)) => {
        actions = vec![item.actions.clone()];

        let single_card = container(
          column![
            row![
              Icon::any(
                item
                  .image_icon
                  .as_deref()
                  .into_iter()
                  .chain(std::iter::once("accessories-calculator-symbolic"))
              )
              .size(28)
              .color(accent_color),
              text(item.title).size(single_font_size).color(result_color),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
            if let Some(sub) = item.subtitle {
              text(sub).size(14.0).color(subtext_color)
            } else {
              text(String::new())
            },
            if let Some(hint) = item.subtext {
              text(hint).size(11.0).color(hint_color)
            } else {
              text(String::new())
            }
          ]
          .spacing(6),
        )
        .width(Length::Fill)
        .padding(single_padding)
        .style(move |_t| container::Style {
          background: Some(result_bg.into()),
          border: iced::Border {
            radius: radius.into(),
            width: border_width,
            color: border_color,
          },
          ..container::Style::default()
        });

        body = container(single_card).into();
      }
      ModeResult::Single(None) => {
        body = container(text("No result").size(14.0).color(hint_color))
          .padding(row_padding)
          .into();
      }
      ModeResult::List(items) => {
        let max_idx = items.len();
        actions = items
          .iter()
          .map(|item| action_defs(item, display))
          .collect();

        let mut render_start = win_start.min(max_idx);
        let render_end;
        if max_idx <= page {
          render_start = 0;
          render_end = max_idx;
        } else {
          if render_start >= max_idx || render_start + page > max_idx {
            render_start = max_idx.saturating_sub(page);
          }
          render_end = (render_start + page).min(max_idx);
        }
        if let Ok(mut inner) = self.inner.lock() {
          inner.visible_start_idx = render_start;
          inner.visible_end_idx = render_end;
        }

        let row_border_width = style.number("row.border.width").unwrap_or(0.0);
        let row_border_color = style.color(theme, "row.border.color", theme.overlay);
        let mut results_col = column![].spacing(spacing);

        match display {
          DisplayStyle::List => {
            for (j, item) in items[render_start..render_end].iter().enumerate() {
              let i = render_start + j;

              let is_selected = i == selected;
              let bg = if is_selected { selected_bg } else { result_bg };

              let icon_elem = item_icon(item, i, 24);

              let title_elem = text(item.title.clone())
                .size(result_font)
                .color(result_color);

              let text_col = if let Some(subtitle) = &item.subtitle {
                column![
                  title_elem,
                  text(subtitle.clone()).size(11.0).color(subtext_color)
                ]
                .spacing(2)
              } else {
                column![title_elem]
              };

              let mut row_content = row![icon_elem, text_col]
                .spacing(12)
                .align_y(Alignment::Center)
                .width(Length::Fill);

              if let Some(tags) = &item.tags {
                if let Some(tag) = tags.iter().next() {
                  let tag_border_width = style.number("tag.border.width").unwrap_or(0.0);
                  let tag_border_color = style.color(theme, "tag.border.color", theme.overlay);
                  row_content = row_content.push(
                    container(text(tag.clone()).size(10.0).color(tag_color))
                      .padding([2.0, 6.0])
                      .style(move |_t| container::Style {
                        background: Some(mantle_bg.into()),
                        border: iced::Border {
                          radius: 4.0.into(),
                          width: tag_border_width,
                          color: tag_border_color,
                        },
                        ..container::Style::default()
                      }),
                  );
                }
              }

              if is_selected {
                let defs = &actions[i];
                let action_label = defs
                  .get(action_index)
                  .map(|def| def.label(action_index))
                  .unwrap_or_default();
                if !action_label.is_empty() {
                  let label = if defs.len() > 1 {
                    format!("{action_label}  [Tab]")
                  } else {
                    action_label
                  };
                  let action_border_width = style.number("action.border.width").unwrap_or(0.0);
                  let action_border_color =
                    style.color(theme, "action.border.color", theme.overlay);
                  row_content = row_content.push(
                    container(text(label).size(10.0).color(accent_color))
                      .padding([2.0, 6.0])
                      .style(move |_t| container::Style {
                        background: Some(mantle_bg.into()),
                        border: iced::Border {
                          radius: 4.0.into(),
                          width: action_border_width,
                          color: action_border_color,
                        },
                        ..container::Style::default()
                      }),
                  );
                }
              }

              let row_container = clickable(
                container(row_content)
                  .width(Length::Fill)
                  .height(Length::Fixed(ROW_HEIGHT))
                  .padding(row_padding)
                  .style(move |_t| container::Style {
                    background: Some(bg.into()),
                    border: iced::Border {
                      radius: radius.into(),
                      width: row_border_width,
                      color: row_border_color,
                    },
                    ..container::Style::default()
                  })
                  .into(),
                move |_, _, _| Some(ItemMessage::Effect(id, ItemEffect::Custom([2, i, 0, 0]))),
              );

              results_col = results_col.push(row_container);
            }
          }
          DisplayStyle::Grid => {
            let mut rows: Vec<Element<'static, ItemMessage>> = Vec::new();
            let mut current: Vec<Element<'static, ItemMessage>> = Vec::new();

            for (j, item) in items[render_start..render_end].iter().enumerate() {
              let i = render_start + j;

              let is_selected = i == selected;
              let bg = if is_selected { selected_bg } else { result_bg };

              let icon: Element<'static, ItemMessage> = container(item_icon(item, i, 36))
                .center_x(Length::Fill)
                .into();
              let title: Element<'static, ItemMessage> =
                container(text(item.title.clone()).size(12.0).color(result_color))
                  .center_x(Length::Fill)
                  .into();

              let mut tile_column = column![icon, title].spacing(8).align_x(Alignment::Center);

              if is_selected {
                let defs = &actions[i];
                let action_label = defs
                  .get(action_index)
                  .map(|def| def.label(action_index))
                  .unwrap_or_default();
                if !action_label.is_empty() {
                  tile_column = tile_column.push(
                    container(
                      text(format!("[{action_label}]"))
                        .size(10.0)
                        .color(accent_color),
                    )
                    .center_x(Length::Fill),
                  );
                }
              }

              let tile = container(tile_column)
                .width(Length::FillPortion(1))
                .height(Length::Fixed(GRID_TILE_HEIGHT))
                .padding(row_padding)
                .style(move |_t| container::Style {
                  background: Some(bg.into()),
                  border: iced::Border {
                    radius: radius.into(),
                    width: row_border_width,
                    color: row_border_color,
                  },
                  ..container::Style::default()
                });

              current.push(clickable(tile.into(), move |_, _, _| {
                Some(ItemMessage::Effect(id, ItemEffect::Custom([2, i, 0, 0])))
              }));

              if current.len() == GRID_COLUMNS {
                rows.push(row(std::mem::take(&mut current)).spacing(spacing).into());
              }
            }

            if !current.is_empty() {
              while current.len() < GRID_COLUMNS {
                current.push(space().width(Length::FillPortion(1)).into());
              }
              rows.push(row(current).spacing(spacing).into());
            }

            results_col = column(rows).spacing(spacing);
          }
          DisplayStyle::ImageList => {
            let mut rows: Vec<Element<'static, ItemMessage>> = Vec::new();
            let mut current: Vec<Element<'static, ItemMessage>> = Vec::new();

            for (j, item) in items[render_start..render_end].iter().enumerate() {
              let i = render_start + j;

              let is_selected = i == selected;
              let bg = if is_selected { selected_bg } else { result_bg };

              let preview = match item_image(item, i) {
                Some(image) => image,
                None => container(item_icon(item, i, 36))
                  .center_x(Length::Fill)
                  .center_y(Length::Fixed(140.0))
                  .into(),
              };

              let mut tile_content = column![preview].spacing(6);
              tile_content = tile_content.push(
                container(text(item.title.clone()).size(12.0).color(result_color))
                  .center_x(Length::Fill),
              );

              if is_selected {
                let defs = &actions[i];
                let action_label = defs
                  .get(action_index)
                  .map(|def| def.label(action_index))
                  .unwrap_or_default();
                if !action_label.is_empty() {
                  tile_content = tile_content.push(
                    container(text(action_label).size(10.0).color(accent_color))
                      .center_x(Length::Fill),
                  );
                }
              }

              let tile = container(tile_content)
                .width(Length::FillPortion(1))
                .height(Length::Fixed(IMAGE_TILE_HEIGHT))
                .padding(row_padding)
                .style(move |_t| container::Style {
                  background: Some(bg.into()),
                  border: iced::Border {
                    radius: radius.into(),
                    width: row_border_width,
                    color: row_border_color,
                  },
                  ..container::Style::default()
                });

              current.push(clickable(tile.into(), move |_, _, _| {
                Some(ItemMessage::Effect(id, ItemEffect::Custom([2, i, 0, 0])))
              }));

              if current.len() == IMAGE_COLUMNS {
                rows.push(row(std::mem::take(&mut current)).spacing(spacing).into());
              }
            }

            if !current.is_empty() {
              while current.len() < IMAGE_COLUMNS {
                current.push(space().width(Length::FillPortion(1)).into());
              }
              rows.push(row(current).spacing(spacing).into());
            }

            results_col = column(rows).spacing(spacing);
          }
        }

        body = container(results_col)
          .width(Length::Fill)
          .height(Length::Fixed(LIST_HEIGHT))
          .clip(true)
          .into();
      }
    }

    let content = column![search_bar, body]
      .spacing(style.number("spacing").unwrap_or(10.0))
      .padding(padding)
      .width(Length::Fixed(600.0));

    let backdrop_bg = style.color(theme, "backdrop", theme.base);

    let main_container = center(container(content))
      .width(Length::Fill)
      .height(Length::Fill)
      .center_x(Length::Fill)
      .center_y(Length::Fill)
      .style(move |_t| container::Style {
        background: Some(backdrop_bg.into()),
        ..container::Style::default()
      });

    let inner_ref = self.inner.clone();
    let result_count = actions.len();

    {
      let mut s = inner_ref.lock().unwrap();
      if modes.allow_triggers.contains(&s.current_mode) {
        let mut triggered = None;

        for trigger in &modes.triggers {
          if (trigger.check)(&s.search) {
            triggered = Some(trigger.target_mode.clone());
            break;
          }
        }

        if let Some(triggered) = triggered {
          if s.current_mode != triggered {
            s.current_mode = triggered;
            s.display_index = 0;
            s.expanded = None;
          }
        } else if s.current_mode != s.base_mode {
          s.current_mode = s.base_mode.clone();
          s.display_index = 0;
          s.expanded = None;
        }
      }
    }

    EventWrapper::new(
      main_container.into(),
      move |event, layout, cursor| match event {
        Event::Keyboard(keyboard::Event::KeyPressed {
          key,
          text,
          modifiers,
          ..
        }) => {
          match key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::Escape) => {
              let mut s = inner_ref.lock().unwrap();
              s.should_close = true;
              s.pending_action = None;
              s.current_mode = "applications".into();
              s.base_mode = "applications".into();
              s.search.clear();
              s.selected_index = 0;
              s.action_index = 0;
              s.display_index = 0;
              s.requested_style = None;
              s.expanded = None;
              s.visible_start_idx = 0;
              s.visible_end_idx = WINDOW_LEN;
              return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
            }
            keyboard::Key::Named(keyboard::key::Named::Enter) => {
              let mut s = inner_ref.lock().unwrap();
              if s.selected_index < actions.len() {
                let defs = &actions[s.selected_index];
                if let Some(def) = defs.get(s.action_index) {
                  match &def.action {
                    SpotlightAction::Expand(path) => {
                      s.expanded = Some(path.clone());
                      return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
                    }
                    action => {
                      s.pending_action = Some(action.clone());
                    }
                  }
                }
              }
              s.should_close = true;
              return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
            }
            keyboard::Key::Named(keyboard::key::Named::Tab) => {
              if modifiers.control() && allowed.len() > 1 {
                let mut s = inner_ref.lock().unwrap();
                s.display_index = (s.display_index + 1) % allowed.len();
                s.action_index = 0;
                s.expanded = None;
                return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
              }
              let mut s = inner_ref.lock().unwrap();
              if s.selected_index < actions.len() {
                let defs = &actions[s.selected_index];
                if defs.len() > 1 {
                  s.action_index = (s.action_index + 1) % defs.len();
                }
              }
              return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
              let mut s = inner_ref.lock().unwrap();
              if s.selected_index == 0 {
                return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
              }
              s.selected_index -= 1;
              s.action_index = 0;
              if let Some(ns) = scroll_target(
                s.selected_index,
                s.visible_start_idx,
                s.visible_end_idx,
                page,
              ) {
                s.visible_start_idx = ns;
                s.visible_end_idx = (ns + page).min(result_count);
              }
              return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
              let mut s = inner_ref.lock().unwrap();
              if s.selected_index + 1 >= result_count {
                return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
              }
              s.selected_index += 1;
              s.action_index = 0;
              if let Some(ns) = scroll_target(
                s.selected_index,
                s.visible_start_idx,
                s.visible_end_idx,
                page,
              ) {
                s.visible_start_idx = ns;
                s.visible_end_idx = (ns + page).min(result_count);
              }
              return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
            }
            keyboard::Key::Named(keyboard::key::Named::Backspace) => {
              let mut s = inner_ref.lock().unwrap();
              s.search.pop();
              s.selected_index = 0;
              s.action_index = 0;

              return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
            }
            _ => {
              if let Some(txt) = text {
                let ch = txt.as_str();
                if !ch.is_empty() && ch.chars().all(|c| !c.is_control()) {
                  let mut s = inner_ref.lock().unwrap();
                  s.search.push_str(ch);
                  s.selected_index = 0;
                  s.action_index = 0;

                  return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
                }
              }
            }
          }
          None
        }
        Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
          if let Some(center_layout) = layout.children().next() {
            if let Some(col_layout) = center_layout.children().next() {
              if !cursor.is_over(col_layout.bounds()) {
                let mut s = inner_ref.lock().unwrap();
                s.should_close = true;
                s.pending_action = None;
                s.current_mode = "applications".into();
                s.base_mode = "applications".into();
                s.search.clear();
                s.selected_index = 0;
                s.action_index = 0;
                s.display_index = 0;
                s.requested_style = None;
                s.expanded = None;
                s.visible_start_idx = 0;
                s.visible_end_idx = WINDOW_LEN;
                return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
              }

              let mut col_children = col_layout.children();
              let _ = col_children.next();
              if let Some(body_layout) = col_children.next() {
                let start = {
                  let s = inner_ref.lock().unwrap();
                  s.visible_start_idx
                };
                if let Some(pos) = cursor.position() {
                  let bounds = body_layout.bounds();
                  if cursor.is_over(bounds) {
                    let columns = display.columns();
                    let tile_width =
                      ((bounds.width - spacing * (columns as f32 - 1.0)) / columns as f32).max(1.0);
                    let step_x = tile_width + spacing;
                    let step_y = display.tile_height() + spacing;
                    let column = ((pos.x - bounds.x) / step_x).floor().max(0.0) as usize;
                    let row = ((pos.y - bounds.y) / step_y).floor().max(0.0) as usize;
                    let idx = start + row * columns + column;
                    if column < columns && idx < actions.len() {
                      let mut s = inner_ref.lock().unwrap();
                      if let Some(def) = actions[idx].get(s.action_index) {
                        match &def.action {
                          SpotlightAction::Expand(path) => s.expanded = Some(path.clone()),
                          action => {
                            s.pending_action = Some(action.clone());
                            s.should_close = true;
                          }
                        }
                      }
                      return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
                    }
                  }
                }
              }
            }
          }
          None
        }
        _ => None,
      },
    )
    .into()
  }
}

fn action_defs(item: &SpotlightItem, display: DisplayStyle) -> Vec<SpotlightActionDef> {
  if display == DisplayStyle::ImageList {
    if let Some(path) = &item.image {
      let mut defs = Vec::with_capacity(item.actions.len() + 1);
      defs.push(SpotlightActionDef {
        title: Some("Expand".into()),
        action: SpotlightAction::Expand(path.clone()),
      });
      defs.extend(item.actions.iter().cloned());
      return defs;
    }
  }

  item.actions.clone()
}

fn item_icon(item: &SpotlightItem, index: usize, size: u16) -> Element<'static, ItemMessage> {
  if let Some(path) = &item.image {
    iced::widget::image(iced::widget::image::Handle::from_path(path))
      .width(Length::Fixed(size as f32))
      .height(Length::Fixed(size as f32))
      .into()
  } else if let Some(raw_bytes) = &item.image_bytes {
    iced::widget::image(Icon::<ItemMessage>::from_bytes(index as i32, raw_bytes))
      .width(Length::Fixed(size as f32))
      .height(Length::Fixed(size as f32))
      .into()
  } else if let Some(icon_name) = &item.image_icon {
    Icon::new(icon_name.clone()).size(size).into_element()
  } else {
    Icon::new("application-x-executable-symbolic")
      .size(size)
      .into_element()
  }
}

fn item_image(item: &SpotlightItem, index: usize) -> Option<Element<'static, ItemMessage>> {
  let height = Length::Fixed(140.0);

  if let Some(path) = &item.image {
    Some(
      iced::widget::image(iced::widget::image::Handle::from_path(path))
        .width(Length::Fill)
        .height(height)
        .content_fit(iced::ContentFit::Cover)
        .into(),
    )
  } else if let Some(raw_bytes) = &item.image_bytes {
    Some(
      iced::widget::image(Icon::<ItemMessage>::from_bytes(index as i32, raw_bytes))
        .width(Length::Fill)
        .height(height)
        .content_fit(iced::ContentFit::Cover)
        .into(),
    )
  } else {
    None
  }
}

fn expanded_view(
  inner_ref: Arc<Mutex<SpotlightInner>>,
  id: IcedId,
  path: PathBuf,
  style: &slowshell_config::style::Style,
  theme: &slowshell_config::style::Theme,
) -> Element<'static, ItemMessage> {
  let backdrop_bg = style.color(theme, "backdrop", theme.base);
  let image = iced::widget::image(iced::widget::image::Handle::from_path(&path))
    .width(Length::Fill)
    .height(Length::Fill)
    .content_fit(iced::ContentFit::Contain);

  let view = container(image)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_t| container::Style {
      background: Some(backdrop_bg.into()),
      ..container::Style::default()
    });

  EventWrapper::new(view.into(), move |event, _, _| match event {
    Event::Keyboard(keyboard::Event::KeyPressed { .. })
    | Event::Mouse(iced::mouse::Event::ButtonPressed(_)) => {
      let mut s = inner_ref.lock().unwrap();
      s.expanded = None;
      Some(ItemMessage::Effect(id, ItemEffect::Redraw))
    }
    _ => None,
  })
  .into()
}

#[derive(Default)]
pub struct SpotlightConfig {
  pub cache: bool,
  pub cliphist: bool,
}

slowshell_registry::register_resources!(
  payload: Unknown(PayloadBuilder {
    commands: &["spotlight.open", "spotlight.toggle"],
    build: |_, args| {
      let mode = args
        .0
        .iter()
        .find(|arg| !arg.contains('='))
        .cloned()
        .unwrap_or_else(|| "applications".into());
      let style = args
        .as_map()
        .get("style")
        .copied()
        .and_then(DisplayStyle::parse);
      Some(PayloadBox::new(SpotlightRequest { mode, style }))
    }
  }.into_boxed()),
  config: Unknown(Box::new(ConfigParser {
    type_id: TypeId::of::<SpotlightConfig>(),
    de: |nodes| match slowshell_config::find_node(nodes, "spotlight") {
      Some(node) => Ok(Some(Box::new(SpotlightConfig {
        cache: slowshell_config::child_bool(node, "cache").unwrap_or(false),
        cliphist: slowshell_config::child_bool(node, "cliphist").unwrap_or(false),
      }))),
      None => Ok(None),
    },
  })),
  app: Custom(|store| {
    DesktopEntries::initialize_unless(store);
  }),
  app: Store {
    type_id: TypeId::of::<SpotlightModes>(),
    create: |config, store| {
      let mut modes = SpotlightModes::default();
      modes.configure(config);
      modes.precache(store);
      Ok(Box::new(modes))
    },
  },
  app: Item(|_, _| Ok(vec![Box::new(Spotlight::new())])),
  spotlight: Style(
    slowshell_config::style! {
      "background" => "crust",
      "background.opacity" => 0.8,
      "radius" => 12,
      "border.width" => 1,
      "border.color" => "overlay",
      "border.color.opacity" => 0.3,
      "padding" => 14,
      "spacing" => 8,
      "row.padding" => 10,
      "row.background" => "mantle",
      "row.background.opacity" => 0.6,
      "row.border.width" => 0,
      "row.border.color" => "overlay",
      "row.border.color.opacity" => 0.15,
      "row.selected" => "primary",
      "row.selected.opacity" => 0.7,
      "tag.border.width" => 0,
      "tag.border.color" => "overlay",
      "tag.border.color.opacity" => 0.15,
      "action.border.width" => 0,
      "action.border.color" => "overlay",
      "action.border.color.opacity" => 0.15,
      "search.background" => "mantle",
      "search.background.opacity" => 0.8,
      "search.color" => "text",
      "search.placeholder.color" => "overlay",
      "result.color" => "text",
      "backdrop" => "crust",
      "backdrop.opacity" => 0.6,
      "search.font.size" => 22,
      "result.font.size" => 15,
      "single.font.size" => 32,
      "single.padding" => 18,
    }
  )
);

#[cfg(test)]
mod tests {
  use super::*;

  fn item(image: Option<PathBuf>) -> SpotlightItem {
    SpotlightItem {
      image,
      image_bytes: None,
      image_icon: None,
      title: "Item".into(),
      subtitle: None,
      subtext: None,
      tags: None,
      actions: vec![SpotlightActionDef {
        title: Some("Copy".into()),
        action: SpotlightAction::Copy("x".into()),
      }],
    }
  }

  #[test]
  fn parses_display_style_names() {
    assert_eq!(DisplayStyle::parse("list"), Some(DisplayStyle::List));
    assert_eq!(DisplayStyle::parse("grid"), Some(DisplayStyle::Grid));
    assert_eq!(
      DisplayStyle::parse("image-list"),
      Some(DisplayStyle::ImageList)
    );
    assert_eq!(DisplayStyle::parse("nope"), None);
  }

  #[test]
  fn image_list_prepends_expand_action() {
    let with_image = item(Some(PathBuf::from("/tmp/a.png")));

    let defs = action_defs(&with_image, DisplayStyle::ImageList);
    assert_eq!(defs.len(), 2);
    assert!(matches!(defs[0].action, SpotlightAction::Expand(_)));

    assert_eq!(action_defs(&with_image, DisplayStyle::List).len(), 1);
    assert_eq!(action_defs(&item(None), DisplayStyle::ImageList).len(), 1);
  }
}
