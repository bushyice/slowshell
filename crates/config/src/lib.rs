use std::{
  any::{Any, TypeId},
  cell::RefCell,
  collections::HashMap,
  fs,
  path::{Path, PathBuf},
};

use iced::{
  Font,
  widget::{Text, container, text::IntoFragment},
};
use icss::theme::{css::ast::PseudoClass, resolve::ComponentSize};
use kdl::{KdlDocument, KdlNode, KdlValue};
use slowshell_core::types::{ToUstr, Ustr};

use crate::style::Theme;

pub mod style;

const DEFAULT_FONT: &str = "Lexend";

pub const DEFAULT_CONFIG: &str = include_str!("default.kdl");

#[derive(thiserror::Error, miette::Diagnostic, Debug)]
#[error("{message}")]
#[diagnostic(code(slowshell::config::error))]
pub struct ConfigError {
  pub message: String,
  #[help]
  pub help: Option<String>,
  #[label("{label}")]
  pub span: Option<miette::SourceSpan>,
  pub label: String,
}

impl ConfigError {
  pub fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
      help: None,
      span: None,
      label: "error here".into(),
    }
  }

  pub fn with_help(message: impl Into<String>, help: impl Into<String>) -> Self {
    Self {
      message: message.into(),
      help: Some(help.into()),
      span: None,
      label: "error here".into(),
    }
  }

  pub fn at_node(
    node: &KdlNode,
    message: impl Into<String>,
    help: Option<impl Into<String>>,
  ) -> Self {
    Self {
      message: message.into(),
      help: help.map(Into::into),
      span: Some(node.span()),
      label: "in this block".into(),
    }
  }

  pub fn at_entry(
    entry: &kdl::KdlEntry,
    message: impl Into<String>,
    help: Option<impl Into<String>>,
  ) -> Self {
    Self {
      message: message.into(),
      help: help.map(Into::into),
      span: Some(entry.span()),
      label: "invalid value".into(),
    }
  }
}

pub struct Config {
  current_path: Option<PathBuf>,
  default_css: String,
  user_css: String,
  engine: icss::Theme,
  style_cache: StyleCache,
  pub palette: Theme,
  pub tick_interval: u64,
  font: Font,
  font_name: Option<&'static str>,
  parsed: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
  pub wgpu_backend: Option<String>,
  pub icon_theme: Option<String>,
  config_parsers: Vec<ConfigParser>,
}

#[derive(Default)]
struct StyleCache {
  containers: RefCell<CacheMap<ClassKey, container::Style>>,
  containers_pseudo: RefCell<CacheMap<(ClassKey, PseudoClass), container::Style>>,
  sizes: RefCell<CacheMap<ClassKey, ComponentSize>>,
  texts: RefCell<CacheMap<ClassKey, crate::style::TextStyle>>,
  colors: RefCell<CacheMap<(ClassKey, Ustr), Option<iced::Color>>>,
  computed:
    RefCell<CacheMap<ClassKey, std::sync::Arc<icss::theme::resolve::computed::ComputedStyle>>>,

  fast_containers: RefCell<FastMap<(usize, u32), container::Style>>,
  fast_sizes: RefCell<FastMap<(usize, u32), ComponentSize>>,
  fast_texts: RefCell<FastMap<(usize, u32), crate::style::TextStyle>>,
  fast_colors: RefCell<FastMap<(usize, u32, Ustr), Option<iced::Color>>>,
  fast_computed:
    RefCell<FastMap<(usize, u32), std::sync::Arc<icss::theme::resolve::computed::ComputedStyle>>>,
}

struct FastEntry<T> {
  classes: Box<[Ustr]>,
  value: T,
}

type FastMap<K, T> = CacheMap<K, FastEntry<T>>;

const FAST_CACHE_LIMIT: usize = 1024;

fn fast_key(classes: &[&str]) -> (usize, u32) {
  (classes.as_ptr() as usize, classes.len() as u32)
}

fn fast_get<K: std::hash::Hash + Eq, T: Clone>(
  map: &RefCell<FastMap<K, T>>,
  key: &K,
  classes: &[&str],
) -> Option<T> {
  let map = map.borrow();
  let entry = map.get(key)?;

  if entry.classes.len() == classes.len()
    && entry
      .classes
      .iter()
      .zip(classes)
      .all(|(cached, class)| &**cached == *class)
  {
    Some(entry.value.clone())
  } else {
    None
  }
}

fn fast_insert<K: std::hash::Hash + Eq, T: Clone>(
  map: &RefCell<FastMap<K, T>>,
  key: K,
  classes: &[&str],
  value: T,
) {
  let mut map = map.borrow_mut();

  if map.len() >= FAST_CACHE_LIMIT || map.contains_key(&key) {
    return;
  }

  map.insert(
    key,
    FastEntry {
      classes: classes.iter().map(|class| Ustr::from(*class)).collect(),
      value,
    },
  );
}

impl StyleCache {
  fn clear(&self) {
    self.containers.borrow_mut().clear();
    self.containers_pseudo.borrow_mut().clear();
    self.sizes.borrow_mut().clear();
    self.texts.borrow_mut().clear();
    self.colors.borrow_mut().clear();
    self.computed.borrow_mut().clear();
    self.fast_containers.borrow_mut().clear();
    self.fast_sizes.borrow_mut().clear();
    self.fast_texts.borrow_mut().clear();
    self.fast_colors.borrow_mut().clear();
    self.fast_computed.borrow_mut().clear();
  }
}

fn weight_from_css(value: f32) -> iced::font::Weight {
  use iced::font::Weight;
  match value.round() as i32 {
    100 => Weight::Thin,
    200 => Weight::ExtraLight,
    300 => Weight::Light,
    400 => Weight::Normal,
    500 => Weight::Medium,
    600 => Weight::Semibold,
    700 => Weight::Bold,
    800 => Weight::ExtraBold,
    900 => Weight::Black,
    _ => Weight::Normal,
  }
}

struct FnvHasher(u64);

impl Default for FnvHasher {
  fn default() -> Self {
    Self(0xcbf2_9ce4_8422_2325)
  }
}

impl std::hash::Hasher for FnvHasher {
  fn finish(&self) -> u64 {
    self.0
  }

  fn write(&mut self, bytes: &[u8]) {
    for byte in bytes {
      self.0 ^= u64::from(*byte);
      self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
    }
  }
}

type CacheMap<K, V> = HashMap<K, V, std::hash::BuildHasherDefault<FnvHasher>>;

const CLASS_KEY_INLINE: usize = 48;

#[derive(Clone, PartialEq, Eq)]
enum ClassKey {
  Inline {
    len: u8,
    buf: [u8; CLASS_KEY_INLINE],
  },
  Heap(Box<str>),
}

impl std::hash::Hash for ClassKey {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    match self {
      ClassKey::Inline { len, buf } => {
        state.write(&buf[..usize::from(*len)]);
        state.write_u8(*len);
      }
      ClassKey::Heap(text) => {
        state.write(text.as_bytes());
        state.write_u8(u8::MAX);
      }
    }
  }
}

fn class_key(classes: &[&str]) -> ClassKey {
  fn build(sorted: &[&str]) -> ClassKey {
    let total = sorted.iter().map(|class| class.len() + 1).sum::<usize>();

    if total <= CLASS_KEY_INLINE {
      let mut buf = [0u8; CLASS_KEY_INLINE];
      let mut at = 0;
      for class in sorted {
        buf[at..at + class.len()].copy_from_slice(class.as_bytes());
        at += class.len() + 1;
      }
      ClassKey::Inline { len: at as u8, buf }
    } else {
      let mut joined = String::with_capacity(total);
      for class in sorted {
        joined.push_str(class);
        joined.push('\0');
      }
      ClassKey::Heap(joined.into_boxed_str())
    }
  }

  if classes.len() <= 8 {
    let mut refs: [&str; 8] = [""; 8];
    refs[..classes.len()].copy_from_slice(classes);
    let refs = &mut refs[..classes.len()];
    refs.sort_unstable();
    build(refs)
  } else {
    let mut refs = classes.to_vec();
    refs.sort_unstable();
    build(&refs)
  }
}

impl Config {
  pub fn theme(&self) -> &icss::Theme {
    &self.engine
  }

  pub fn container_style(&self, classes: &[&str]) -> container::Style {
    let key = fast_key(classes);
    if let Some(style) = fast_get(&self.style_cache.fast_containers, &key, classes) {
      return style;
    }

    let style = self.container_style_with_pseudo(classes, None);
    fast_insert(&self.style_cache.fast_containers, key, classes, style);
    style
  }

  pub fn container_style_with_pseudo(
    &self,
    classes: &[&str],
    pseudo: Option<PseudoClass>,
  ) -> container::Style {
    let key = class_key(classes);

    if let Some(pseudo) = pseudo {
      let key = (key, pseudo);
      if let Some(style) = self.style_cache.containers_pseudo.borrow().get(&key) {
        return *style;
      }
      let computed = self.engine.resolve(classes, Some(pseudo));
      let style = crate::style::container_style(&computed).unwrap_or_default();
      self
        .style_cache
        .containers_pseudo
        .borrow_mut()
        .insert(key, style);
      style
    } else {
      if let Some(style) = self.style_cache.containers.borrow().get(&key) {
        return *style;
      }
      let computed = self.engine.resolve(classes, None);
      let style = crate::style::container_style(&computed).unwrap_or_default();
      self.style_cache.containers.borrow_mut().insert(key, style);
      style
    }
  }

  pub fn container_cached(
    &self,
    classes: &[&str],
  ) -> impl Fn(&iced::Theme) -> container::Style + 'static {
    let style = self.container_style(classes);
    move |_| style
  }

  pub fn sizing(&self, classes: &[&str]) -> ComponentSize {
    let key = fast_key(classes);
    if let Some(size) = fast_get(&self.style_cache.fast_sizes, &key, classes) {
      return size;
    }

    let size = self.sizing_uncached(classes);
    fast_insert(&self.style_cache.fast_sizes, key, classes, size);
    size
  }

  fn sizing_uncached(&self, classes: &[&str]) -> ComponentSize {
    let key = class_key(classes);

    if let Some(size) = self.style_cache.sizes.borrow().get(&key) {
      return *size;
    }

    let size = self.engine.sizing(classes);
    self.style_cache.sizes.borrow_mut().insert(key, size);
    size
  }

  pub fn computed(
    &self,
    classes: &[&str],
  ) -> std::sync::Arc<icss::theme::resolve::computed::ComputedStyle> {
    let key = fast_key(classes);
    if let Some(computed) = fast_get(&self.style_cache.fast_computed, &key, classes) {
      return computed;
    }

    let computed = self.computed_uncached(classes);
    fast_insert(
      &self.style_cache.fast_computed,
      key,
      classes,
      computed.clone(),
    );
    computed
  }

  fn computed_uncached(
    &self,
    classes: &[&str],
  ) -> std::sync::Arc<icss::theme::resolve::computed::ComputedStyle> {
    let key = class_key(classes);

    if let Some(computed) = self.style_cache.computed.borrow().get(&key) {
      return computed.clone();
    }

    let computed = std::sync::Arc::new(self.engine.resolve(classes, None));
    self
      .style_cache
      .computed
      .borrow_mut()
      .insert(key, computed.clone());
    computed
  }

  pub fn text_style(&self, classes: &[&str]) -> crate::style::TextStyle {
    let key = fast_key(classes);
    if let Some(style) = fast_get(&self.style_cache.fast_texts, &key, classes) {
      return style;
    }

    let style = self.text_style_uncached(classes);
    fast_insert(&self.style_cache.fast_texts, key, classes, style);
    style
  }

  fn text_style_uncached(&self, classes: &[&str]) -> crate::style::TextStyle {
    let key = class_key(classes);

    if let Some(style) = self.style_cache.texts.borrow().get(&key) {
      return *style;
    }

    let computed = self.engine.resolve(classes, None);
    let style = crate::style::TextStyle {
      size: computed.length("font-size"),
      color: computed.color("color").map(|color| color.to_iced()),
      weight: computed.number("font-weight").map(weight_from_css),
    };
    self.style_cache.texts.borrow_mut().insert(key, style);
    style
  }

  pub fn text<'a>(
    &self,
    content: impl IntoFragment<'a>,
    classes: &[&str],
  ) -> Text<'a, iced::Theme> {
    let style = self.text_style(classes);
    let mut text = iced::widget::text(content);

    if let Some(size) = style.size {
      text = text.size(size);
    }

    if let Some(color) = style.color {
      text = text.color(color);
    }

    if let Some(weight) = style.weight {
      text = text.font(Font {
        weight,
        ..self.font
      });
    }

    text
  }

  pub fn color(&self, classes: &[&str], property: &str) -> Option<iced::Color> {
    let (ptr, len) = fast_key(classes);
    let key = (ptr, len, Ustr::from(property));
    if let Some(color) = fast_get(&self.style_cache.fast_colors, &key, classes) {
      return color;
    }

    let color = self.color_uncached(classes, property);
    fast_insert(&self.style_cache.fast_colors, key, classes, color);
    color
  }

  fn color_uncached(&self, classes: &[&str], property: &str) -> Option<iced::Color> {
    let key = (class_key(classes), Ustr::from(property));

    if let Some(color) = self.style_cache.colors.borrow().get(&key) {
      return *color;
    }

    let color = self
      .engine
      .resolve(classes, None)
      .color(property)
      .map(|color| color.to_iced());
    self.style_cache.colors.borrow_mut().insert(key, color);
    color
  }

  pub fn background_image(&self, classes: &[&str]) -> Option<PathBuf> {
    use icss::theme::css::ast::CssValue;

    let computed = self.computed(classes);
    let raw = match computed.get("background-image")? {
      CssValue::Raw(raw) => raw.as_str(),
      CssValue::Keyword(keyword) if keyword == "none" => return None,
      _ => return None,
    };

    let value = crate::style::parse_image_path(raw)?;

    let path = if let Some(rest) = value.strip_prefix("~/") {
      std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(rest))
        .unwrap_or_else(|| PathBuf::from(&value))
    } else {
      let path = PathBuf::from(&value);
      if path.is_absolute() {
        path
      } else if let Some(dir) = self.current_dir() {
        dir.join(path)
      } else {
        path
      }
    };

    path.is_file().then_some(path)
  }

  pub fn palette(&self) -> &Theme {
    &self.palette
  }

  pub fn stylesheet(&self) -> String {
    let mut css = self.palette.to_css();
    css.push_str(&self.default_css);
    css.push_str(&self.user_css);
    css
  }

  pub fn current_path(&self) -> Option<&PathBuf> {
    self.current_path.as_ref()
  }

  pub fn current_dir(&self) -> Option<PathBuf> {
    self.current_path.as_ref().and_then(|p| {
      if p.is_dir() {
        Some(p.clone())
      } else {
        p.parent().map(|parent| parent.to_path_buf())
      }
    })
  }

  pub fn resolve_path<P: AsRef<Path>>(path: Option<P>) -> Option<PathBuf> {
    path
      .and_then(|p| p.as_ref().canonicalize().ok())
      .or_else(|| {
        std::env::var_os("HOME")
          .map(PathBuf::from)
          .and_then(|home| {
            [
              home.join(".config/slowshell/config.kdl"),
              home.join(".local/share/slowshell/config.kdl"),
              PathBuf::from("/usr/share/slowshell/config.kdl"),
            ]
            .into_iter()
            .find(|p| p.exists())
            .and_then(|p| p.canonicalize().ok())
          })
      })
  }

  pub fn from_path_or_default<P: AsRef<Path>>(
    path: Option<P>,
    config_parsers: Vec<ConfigParser>,
  ) -> Self {
    let current_path = Self::resolve_path(path);

    let mut config = Config {
      current_path,
      config_parsers,
      ..Default::default()
    };

    if let Err(e) = config.reload() {
      eprintln!("{e:?}");
    }
    config
  }

  pub fn reload(&mut self) -> miette::Result<()> {
    use miette::IntoDiagnostic;

    self.default_css = crate::style::default_css();
    self.user_css.clear();
    self.palette = Theme::default();
    self.parsed = HashMap::new();

    let current_path = self.current_path.clone();

    if let Some(path) = &current_path {
      if !path.exists() {
        self.rebuild_engine();
        return Ok(());
      }

      let content = fs::read_to_string(path).into_diagnostic()?;
      let doc: KdlDocument = match content.parse() {
        Ok(doc) => doc,
        Err(e) => {
          let report = miette::Report::new(e)
            .wrap_err(format!("failed to parse config file {}", path.display()));
          return Err(report);
        }
      };

      let nodes = doc.nodes();

      for deser in &self.config_parsers {
        match (deser.de)(nodes) {
          Ok(Some(parsed)) => {
            self.parsed.insert(deser.type_id, parsed);
          }
          Err(e) => {
            let report = e.with_source_code(miette::NamedSource::new(
              path.display().to_string(),
              content.clone(),
            ));
            eprintln!("{report:?}");
          }
          _ => {}
        }
      }

      self.parse_theme(nodes, current_path.as_ref());
      self.parse_css(nodes, current_path.as_ref());
      self.parse_font(nodes);
      self.parse_misc(nodes);
    }

    self.rebuild_engine();
    Ok(())
  }

  fn rebuild_engine(&mut self) {
    match icss::Theme::load(&self.stylesheet()) {
      Ok(engine) => {
        self.engine = engine.with_default_font(self.font);
        self.style_cache.clear();
      }
      Err(error) => eprintln!("failed to load generated style css: {error}"),
    }
  }

  fn parse_theme(&mut self, nodes: &[KdlNode], path: Option<&PathBuf>) {
    let Some(theme) = find_node(nodes, "theme") else {
      return;
    };

    if let Some(name) = str_arg(theme, 0) {
      if let Some(path) = path.and_then(|x| x.parent()) {
        let mut theme = path.join("themes").join(name);
        theme.add_extension("kdl");

        if let Ok(content) = fs::read_to_string(&theme) {
          let doc: KdlDocument = match content.parse() {
            Ok(doc) => doc,
            Err(e) => {
              let report = miette::Report::new(e)
                .wrap_err(format!("failed to parse theme file {}", theme.display()));
              eprintln!("{report:?}");
              return;
            }
          };

          self.apply_theme(doc.nodes());
        }
      }
    } else {
      self.apply_theme(node_children(theme));
    }
  }

  fn apply_theme(&mut self, theme_nodes: &[KdlNode]) {
    for child in theme_nodes {
      if let Some(color) = str_arg(child, 0) {
        self.palette.apply(node_name(child), color);
      }
    }
  }

  fn parse_css(&mut self, nodes: &[KdlNode], path: Option<&PathBuf>) {
    for node in nodes {
      match node_name(node) {
        "include-css" => {
          if let (Some(cpath), Some(name)) = (path.and_then(|x| x.parent()), str_arg(node, 0)) {
            let file = cpath.join(name);
            match fs::read_to_string(&file) {
              Ok(content) => {
                self.user_css.push_str(&content);
                if !self.user_css.ends_with('\n') {
                  self.user_css.push('\n');
                }
              }
              Err(e) => {
                eprintln!("failed to read css file {}: {e}", file.display());
              }
            }
          }
        }
        "css" => {
          if let Some(css) = str_arg(node, 0) {
            self.user_css.push_str(css);
            if !self.user_css.ends_with('\n') {
              self.user_css.push('\n');
            }
          }
        }
        _ => {}
      }
    }
  }

  fn parse_font(&mut self, nodes: &[KdlNode]) {
    let Some(name) = find_node(nodes, "font").and_then(|node| str_arg(node, 0)) else {
      return;
    };

    if self.font_name == Some(name) {
      return;
    }

    let name = intern_font_name(name);
    self.font_name = Some(name);
    self.font = Font::with_name(name);
  }

  fn parse_misc(&mut self, nodes: &[KdlNode]) {
    if let Some(int) = find_node(nodes, "tick-interval").and_then(|node| int_arg(node, 0)) {
      self.tick_interval = int as u64;
    }
    if let Some(backend) = find_node(nodes, "wgpu-backend").and_then(|node| str_arg(node, 0)) {
      self.wgpu_backend = Some(backend.into());
    }
    if let Some(theme) = find_node(nodes, "icons").and_then(|node| child_str(node, "theme")) {
      self.icon_theme = Some(theme.into());
    }
  }

  pub fn font(&self) -> Font {
    self.font
  }

  pub fn font_name(&self) -> &str {
    self.font_name.unwrap_or(DEFAULT_FONT)
  }

  pub fn validate(&self) -> miette::Result<()> {
    use miette::IntoDiagnostic;

    let Some(path) = &self.current_path else {
      return Ok(());
    };

    if !path.exists() {
      return Ok(());
    }

    let content = fs::read_to_string(path).into_diagnostic()?;
    let doc: KdlDocument = match content.parse() {
      Ok(doc) => doc,
      Err(e) => {
        let report = miette::Report::new(e)
          .wrap_err(format!("failed to parse config file {}", path.display()));
        return Err(report);
      }
    };

    for deser in &self.config_parsers {
      if let Err(e) = (deser.de)(doc.nodes()) {
        let report = e.with_source_code(miette::NamedSource::new(
          path.display().to_string(),
          content.clone(),
        ));
        return Err(report);
      }
    }

    Ok(())
  }

  pub fn typed<T: 'static>(&self) -> Option<&T> {
    self
      .parsed
      .get(&TypeId::of::<T>())
      .and_then(|x| x.downcast_ref::<T>())
  }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
  Int(i64),
  Float(f64),
  Bool(bool),
  Str(String),
  Array(Vec<Value>),
  Table(Vec<(String, Value)>),
}

impl Value {
  pub fn as_str(&self) -> Option<&str> {
    match self {
      Value::Str(s) => Some(s),
      _ => None,
    }
  }

  pub fn as_int(&self) -> Option<i64> {
    match self {
      Value::Int(i) => Some(*i),
      _ => None,
    }
  }

  pub fn as_float(&self) -> Option<f64> {
    match self {
      Value::Float(f) => Some(*f),
      _ => None,
    }
  }

  pub fn as_bool(&self) -> Option<bool> {
    match self {
      Value::Bool(b) => Some(*b),
      _ => None,
    }
  }

  pub fn as_array(&self) -> Option<&[Value]> {
    match self {
      Value::Array(a) => Some(a),
      _ => None,
    }
  }

  pub fn as_table(&self) -> Option<&[(String, Value)]> {
    match self {
      Value::Table(t) => Some(t),
      _ => None,
    }
  }

  pub fn get(&self, key: &str) -> Option<&Value> {
    self
      .as_table()?
      .iter()
      .find(|(k, _)| k == key)
      .map(|(_, v)| v)
  }
}

impl From<&str> for Value {
  fn from(value: &str) -> Self {
    Value::Str(value.into())
  }
}

impl From<&KdlValue> for Value {
  fn from(value: &KdlValue) -> Self {
    match value {
      KdlValue::String(s) => Value::Str(s.clone()),
      KdlValue::Integer(i) => Value::Int(*i as i64),
      KdlValue::Float(f) => Value::Float(*f),
      KdlValue::Bool(b) => Value::Bool(*b),
      KdlValue::Null => Value::Table(Vec::new()),
    }
  }
}

pub fn node_span(node: &KdlNode) -> miette::SourceSpan {
  node.span()
}

pub fn node_name(node: &KdlNode) -> &str {
  node.name().value()
}

pub fn value_arg(node: &KdlNode, index: usize) -> Option<&KdlValue> {
  node.get(index)
}

pub fn str_arg<'a>(node: &'a KdlNode, index: usize) -> Option<&'a str> {
  value_arg(node, index).and_then(KdlValue::as_string)
}

pub fn int_arg(node: &KdlNode, index: usize) -> Option<i64> {
  value_arg(node, index)
    .and_then(KdlValue::as_integer)
    .map(|i| i as i64)
}

pub fn float_arg(node: &KdlNode, index: usize) -> Option<f64> {
  value_arg(node, index).and_then(KdlValue::as_float)
}

pub fn bool_arg(node: &KdlNode, index: usize) -> Option<bool> {
  value_arg(node, index).and_then(KdlValue::as_bool)
}

pub fn prop<'a>(node: &'a KdlNode, name: &str) -> Option<&'a KdlValue> {
  node.get(name)
}

pub fn prop_str<'a>(node: &'a KdlNode, name: &str) -> Option<&'a str> {
  prop(node, name).and_then(KdlValue::as_string)
}

pub fn prop_i64(node: &KdlNode, name: &str) -> Option<i64> {
  prop(node, name)
    .and_then(KdlValue::as_integer)
    .map(|i| i as i64)
}

pub fn prop_bool(node: &KdlNode, name: &str) -> Option<bool> {
  prop(node, name).and_then(KdlValue::as_bool)
}

pub fn node_children(node: &KdlNode) -> &[KdlNode] {
  node.children().map(|doc| doc.nodes()).unwrap_or(&[])
}

pub fn find_node<'a>(nodes: &'a [KdlNode], name: &str) -> Option<&'a KdlNode> {
  nodes.iter().find(|node| node_name(node) == name)
}

pub fn child<'a>(node: &'a KdlNode, name: &str) -> Option<&'a KdlNode> {
  node_children(node)
    .iter()
    .find(|child| node_name(child) == name)
}

pub fn child_str<'a>(node: &'a KdlNode, name: &str) -> Option<&'a str> {
  child(node, name).and_then(|child| str_arg(child, 0))
}

pub fn child_str_owned(node: &KdlNode, name: &str) -> Option<String> {
  child_str(node, name).map(str::to_owned)
}

pub fn child_i64(node: &KdlNode, name: &str) -> Option<i64> {
  child(node, name).and_then(|child| int_arg(child, 0))
}

pub fn child_f64(node: &KdlNode, name: &str) -> Option<f64> {
  child(node, name).and_then(|child| float_arg(child, 0))
}

pub fn child_bool(node: &KdlNode, name: &str) -> Option<bool> {
  child(node, name).and_then(|child| bool_arg(child, 0))
}

pub fn node_to_value(node: &KdlNode) -> Value {
  let children = node_children(node);
  if !children.is_empty() {
    return Value::Table(
      children
        .iter()
        .map(|child| (node_name(child).to_string(), node_to_value(child)))
        .collect(),
    );
  }

  let mut args = Vec::new();
  let mut props = Vec::new();
  for entry in node.entries() {
    match entry.name() {
      Some(key) => props.push((key.value().to_string(), Value::from(entry.value()))),
      None => args.push(entry.value()),
    }
  }

  match args.len() {
    1 => Value::from(args[0]),
    0 => {
      if props.is_empty() {
        Value::Table(Vec::new())
      } else {
        Value::Table(props)
      }
    }
    _ => Value::Array(args.iter().map(|value| Value::from(*value)).collect()),
  }
}

pub fn node_options_map(node: &KdlNode) -> HashMap<Ustr, Value> {
  let mut map = HashMap::new();
  for entry in node.entries() {
    if let Some(key) = entry.name() {
      map.insert(key.value().to_ustr(), Value::from(entry.value()));
    }
  }
  for child in node_children(node) {
    map.insert(node_name(child).to_ustr(), node_to_value(child));
  }
  map
}

impl Default for Config {
  fn default() -> Self {
    let default_css = crate::style::default_css();
    let palette = Theme::default();
    let mut stylesheet = palette.to_css();
    stylesheet.push_str(&default_css);
    let font = Font::with_name(DEFAULT_FONT);
    let engine = icss::Theme::load(&stylesheet)
      .unwrap_or_else(|error| {
        eprintln!("failed to load generated style css: {error}");
        icss::Theme::load("").expect("empty stylesheet")
      })
      .with_default_font(font);

    Self {
      default_css,
      user_css: String::new(),
      engine,
      style_cache: StyleCache::default(),
      palette,
      font,
      font_name: None,
      parsed: HashMap::new(),
      tick_interval: 3,
      wgpu_backend: None,
      icon_theme: None,
      current_path: None,
      config_parsers: Vec::new(),
    }
  }
}

fn intern_font_name(name: &str) -> &'static str {
  use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
  };

  static NAMES: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
  let mut names = NAMES
    .get_or_init(|| Mutex::new(HashSet::new()))
    .lock()
    .unwrap();

  if let Some(existing) = names.get(name) {
    return *existing;
  }

  let interned: &'static str = Box::leak(name.to_string().into_boxed_str());
  names.insert(interned);
  interned
}

pub const FONT: &[u8] = include_bytes!("../../../assets/lexend.ttf");

#[derive(Debug, Clone, Copy)]
pub struct AnimationsConfig {
  pub enabled: bool,
}

#[derive(Clone, Copy)]
pub struct ConfigParser {
  pub type_id: TypeId,
  pub de: fn(&[KdlNode]) -> miette::Result<Option<Box<dyn Any + Send + Sync>>>,
}

pub fn animations_parser() -> ConfigParser {
  ConfigParser {
    type_id: TypeId::of::<AnimationsConfig>(),
    de: |nodes| {
      let Some(node) = find_node(nodes, "animations") else {
        return Ok(None);
      };

      Ok(Some(Box::new(AnimationsConfig {
        enabled: child_bool(node, "enabled").unwrap_or(true),
      })))
    },
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn config_with(css: &str) -> Config {
    let mut config = Config::default();
    config.default_css = css.into();
    config.rebuild_engine();
    config
  }

  #[test]
  fn container_style_cache_tracks_stylesheet_rebuilds() {
    let mut config = config_with(".card { background-color: #ff0000; }");
    let red = config.container_style(&["card"]);
    assert_eq!(
      red.background,
      Some(iced::Background::Color(iced::Color::from_rgb(
        1.0, 0.0, 0.0
      )))
    );

    config.default_css = ".card { background-color: #00ff00; }".into();
    config.rebuild_engine();

    let green = config.container_style(&["card"]);
    assert_ne!(red, green);
    assert_eq!(
      green.background,
      Some(iced::Background::Color(iced::Color::from_rgb(
        0.0, 1.0, 0.0
      )))
    );
  }

  #[test]
  fn hover_pseudo_style_layers_over_the_base() {
    let config = config_with(
      ".card { background-color: #ff0000; border-radius: 4px; }\n\
       .card:hover { background-color: #0000ff; }",
    );

    let base = config.container_style(&["card"]);
    let hover = config.container_style_with_pseudo(&["card"], Some(PseudoClass::Hover));

    assert_ne!(base, hover);
    assert_eq!(
      hover.background,
      Some(iced::Background::Color(iced::Color::from_rgb(
        0.0, 0.0, 1.0
      )))
    );

    assert_eq!(hover.border.radius, iced::border::Radius::from(4.0));
  }

  #[test]
  fn cached_lookups_match_direct_icss_resolution() {
    let config = config_with(
      ".card { background-color: #ff0000; color: #00ff00; font-size: 13px; \
       padding-v: 7px; padding-h: 12px; gap: 8px; border-radius: 4px; \
       box-shadow: 0 2px 4px #000000; }",
    );
    let classes = &["card"];
    let direct = config.theme().resolve(classes, None);

    let cached = config.computed(classes);
    assert_eq!(cached.length("padding-v"), direct.length("padding-v"));
    assert_eq!(
      cached.length("border-radius"),
      direct.length("border-radius")
    );
    assert_eq!(
      cached
        .color("background-color")
        .map(|color| color.to_iced()),
      direct
        .color("background-color")
        .map(|color| color.to_iced())
    );

    let sizing = config.sizing(classes);
    let direct_sizing = config.theme().sizing(classes);
    assert_eq!(sizing.pad_v, direct_sizing.pad_v);
    assert_eq!(sizing.pad_h, direct_sizing.pad_h);
    assert_eq!(sizing.gap, direct_sizing.gap);
    assert_eq!(sizing.font_size, direct_sizing.font_size);
    assert_eq!(sizing.icon_size, direct_sizing.icon_size);

    let text = config.text_style(classes);
    assert_eq!(text.size, Some(13.0));
    assert_eq!(
      text.color,
      direct.color("color").map(|color| color.to_iced())
    );
    assert_eq!(text.weight, None);

    assert_eq!(
      config.color(classes, "color"),
      direct.color("color").map(|color| color.to_iced())
    );
    assert_eq!(config.color(classes, "missing"), None);

    assert_eq!(
      config.container_style(classes),
      crate::style::container_style(&direct).expect("container style")
    );
  }

  #[test]
  fn background_image_resolves_and_checks_existence() {
    let path = std::env::temp_dir().join("slowshell-background-image-test.png");
    std::fs::write(&path, b"not really an image").unwrap();

    let css = format!(".card {{ background-image: url(\"{}\"); }}", path.display());
    let config = config_with(&css);
    assert_eq!(config.background_image(&["card"]), Some(path.clone()));

    let missing = std::env::temp_dir().join("slowshell-background-image-missing.png");
    let css = format!(
      ".card {{ background-image: url(\"{}\"); }}",
      missing.display()
    );
    let config = config_with(&css);
    assert_eq!(config.background_image(&["card"]), None);

    let config = config_with(".card { background-image: none; }");
    assert_eq!(config.background_image(&["card"]), None);

    std::fs::remove_file(path).unwrap();
  }

  #[test]
  fn caches_reflect_stylesheet_rebuilds() {
    let mut config = config_with(".card { color: #ff0000; padding-v: 3px; }");
    let classes = &["card"];

    assert_eq!(
      config.color(classes, "color"),
      Some(iced::Color::from_rgb(1.0, 0.0, 0.0))
    );
    assert_eq!(config.sizing(classes).pad_v, 3.0);
    assert_eq!(config.text_style(classes).size, None);

    config.default_css = ".card { color: #0000ff; padding-v: 9px; font-size: 17px; }".into();
    config.rebuild_engine();

    assert_eq!(
      config.color(classes, "color"),
      Some(iced::Color::from_rgb(0.0, 0.0, 1.0))
    );
    assert_eq!(config.sizing(classes).pad_v, 9.0);
    assert_eq!(config.text_style(classes).size, Some(17.0));
  }

  #[test]
  fn fast_cache_verifies_class_contents() {
    let map: RefCell<FastMap<(usize, u32), u32>> = RefCell::new(CacheMap::default());
    let classes = ["menu", "row"];

    fast_insert(&map, fast_key(&classes), &classes, 7);
    assert_eq!(fast_get(&map, &fast_key(&classes), &classes), Some(7));

    {
      let mut map = map.borrow_mut();
      map.get_mut(&fast_key(&classes)).expect("entry").classes =
        vec![Ustr::from("other"), Ustr::from("row")].into_boxed_slice();
    }
    assert_eq!(fast_get(&map, &fast_key(&classes), &classes), None);

    let other = ["menu", "button"];
    assert_eq!(fast_get(&map, &fast_key(&other), &other), None);
  }

  #[test]
  fn container_style_is_order_independent() {
    let config = config_with(".card.compact { border-radius: 3px; }");
    assert_eq!(
      config.container_style(&["card", "compact"]),
      config.container_style(&["compact", "card"])
    );
  }
}

#[cfg(test)]
mod bench {
  use std::hint::black_box;
  use std::time::Instant;

  use super::*;

  const MENU_CSS: &str = r#"
.menu {
  background-color: var(--crust-a95);
  border-radius: 8px;
  border-width: 1px;
  border-color: var(--overlay-a15);
  padding-v: 16px;
  padding-h: 16px;
  gap: 12px;
  color: var(--text);
  font-size: 13px;
}
.menu.row {
  background-color: var(--mantle-a50);
  border-radius: 6px;
  border-width: 0px;
  border-color: var(--overlay-a15);
  padding-v: 16px;
  padding-h: 16px;
  gap: 8px;
}
.menu.row.compact { padding-v: 7px; padding-h: 12px; }
.menu.text-faded { color: var(--subtext); }
"#;

  fn bench_config() -> Config {
    let mut config = Config::default();
    config.default_css.push_str(MENU_CSS);
    config.rebuild_engine();
    config
  }

  fn time_generator(
    iterations: u32,
    closure: impl Fn(&iced::Theme) -> container::Style,
    iced_theme: &iced::Theme,
  ) -> f64 {
    let start = Instant::now();
    for _ in 0..iterations {
      black_box(closure(black_box(iced_theme)));
    }
    start.elapsed().as_nanos() as f64 / f64::from(iterations)
  }

  fn ns_per(iterations: u32, mut body: impl FnMut()) -> f64 {
    let start = Instant::now();
    for _ in 0..iterations {
      body();
    }
    start.elapsed().as_nanos() as f64 / f64::from(iterations)
  }

  #[test]
  #[ignore = "benchmark; run with: cargo test -p slowshell-config -- --ignored --nocapture"]
  fn style_resolution_bench() {
    const N: u32 = 200_000;

    let config = bench_config();
    let theme = config.theme();
    let iced_theme = iced::Theme::Dark;
    let classes: &[&str] = &["menu", "row", "compact"];

    black_box(theme.resolve(classes, None));
    black_box(config.container_style(classes));
    black_box(theme.container(classes)(&iced_theme));

    let resolve = ns_per(N, || {
      let computed = black_box(theme.resolve(black_box(classes), None));
      black_box(computed.color("background-color"));
    });

    let generator = time_generator(N, theme.container(classes), &iced_theme);

    let cached_style = config.container_style(classes);
    let style_copy = ns_per(N, || {
      black_box(cached_style);
    });

    let cache_lookup = ns_per(N, || {
      black_box(config.container_style(black_box(classes)));
    });

    let cached_closure = config.container_cached(classes);
    let closure_call = ns_per(N, || {
      black_box(cached_closure(&iced_theme));
    });

    let key_build = ns_per(N, || {
      black_box(class_key(black_box(classes)));
    });

    let key = class_key(classes);
    let cache = config.style_cache.containers.borrow();
    let map_get = ns_per(N, || {
      black_box(cache.get(black_box(&key)));
    });
    drop(cache);

    let sizing = ns_per(N, || {
      black_box(config.sizing(black_box(classes)));
    });

    let text_style = ns_per(N, || {
      black_box(config.text_style(black_box(classes)));
    });

    let text_build = ns_per(N, || {
      black_box(config.text(black_box("Wi-Fi"), black_box(classes)));
    });

    let color = ns_per(N, || {
      black_box(config.color(black_box(classes), "color"));
    });

    let cold = ns_per(10_000, || {
      let fresh = icss::Theme::load(&config.stylesheet()).expect("css parses");
      let computed = fresh.resolve(black_box(classes), None);
      black_box(computed.color("background-color"));
    });

    println!("style_resolution_bench (ns/op, {N} samples)");
    println!("  icss resolve (warm cache)      {resolve:8.0}");
    println!("  icss container generator call  {generator:8.0}");
    println!("  Config::container_style        {cache_lookup:8.0}");
    println!("  Config::sizing                 {sizing:8.0}");
    println!("  Config::text_style             {text_style:8.0}");
    println!("  Config::text (build widget)    {text_build:8.0}");
    println!("  Config::color                  {color:8.0}");
    println!("    of which key construction    {key_build:8.0}");
    println!("    of which hash map lookup     {map_get:8.0}");
    println!("  Config::container_cached call  {closure_call:8.0}");
    println!("  plain container::Style copy    {style_copy:8.0}");
    println!("  stylesheet load + resolve      {cold:8.0}");
  }
}
