use std::{
  any::{Any, TypeId},
  collections::HashMap,
  fs,
  path::{Path, PathBuf},
};

use iced::Font;
use kdl::{KdlDocument, KdlNode, KdlValue};
use slowshell_core::types::{ToUstr, Ustr};

use crate::style::{DEFAULT_STYLES, Style, Theme};

pub mod style;

const DEFAULT_FONT: &str = "Lexend";

pub struct Config {
  current_path: Option<PathBuf>,
  styles: HashMap<Ustr, Style>,
  pub theme: Theme,
  pub tick_interval: u64,
  font: Font,
  parsed: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
  pub wgpu_backend: Option<String>,
  config_parsers: Vec<ConfigParser>,
}

impl Config {
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

  pub fn from_path_or_default<P: AsRef<Path>>(
    path: Option<P>,
    config_parsers: Vec<ConfigParser>,
  ) -> Self {
    let current_path = path
      .and_then(|p| p.as_ref().to_owned().canonicalize().ok())
      .or_else(|| {
        let home = std::env::var_os("HOME")?;

        [
          PathBuf::from(&home).join(".config/slowshell"),
          PathBuf::from(&home).join(".local/share/slowshell"),
          PathBuf::from("/usr/share/slowshell"),
        ]
        .into_iter()
        .find(|p| p.join("config.kdl").exists())
        .and_then(|p| p.join("config.kdl").canonicalize().ok())
      });

    let mut config = Config {
      current_path,
      config_parsers,
      ..Default::default()
    };

    let _ = config.reload();
    config
  }

  pub fn reload(&mut self) -> anyhow::Result<()> {
    let mut new_styles = HashMap::new();
    let default_styles = DEFAULT_STYLES.lock().unwrap();
    for (k, v) in default_styles.iter() {
      new_styles.insert(k.clone(), v.clone());
    }
    drop(default_styles);

    let mut new_parsed = HashMap::new();
    let current_path = self.current_path.clone();

    if let Some(path) = &current_path {
      if !path.exists() {
        return Ok(());
      }

      let content = fs::read_to_string(path)?;
      let doc: KdlDocument = match content.parse() {
        Ok(doc) => doc,
        Err(e) => {
          return Err(anyhow::anyhow!("failed to parse config: {e}"));
        }
      };

      let nodes = doc.nodes();

      for deser in &self.config_parsers {
        match (deser.de)(nodes) {
          Ok(Some(parsed)) => {
            new_parsed.insert(deser.type_id, parsed);
          }
          Err(e) => {
            eprintln!("config parser error: {e}");
          }
          _ => {}
        }
      }

      self.styles = new_styles;
      self.theme = Theme::default();
      self.parsed = new_parsed;

      self.parse_theme(nodes, current_path.as_ref());
      self.parse_styles(nodes, current_path.as_ref());
      self.parse_font(nodes);
      self.parse_misc(nodes);
      return Ok(());
    }

    self.styles = new_styles;
    self.theme = Theme::default();
    self.parsed = new_parsed;
    Ok(())
  }

  fn parse_theme(&mut self, nodes: &[KdlNode], path: Option<&PathBuf>) {
    let Some(theme) = find_node(nodes, "theme") else {
      return;
    };

    if let Some(name) = str_arg(theme, 0) {
      if let Some(path) = path.and_then(|x| x.parent()) {
        let mut theme = path.join("themes").join(name);
        theme.add_extension("kdl");

        if let Ok(content) = fs::read_to_string(theme) {
          let doc: KdlDocument = match content.parse() {
            Ok(doc) => doc,
            Err(e) => {
              eprintln!("failed to parse theme: {e}");
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
        self.theme.apply(node_name(child), color);
      }
    }
  }

  fn parse_styles(&mut self, nodes: &[KdlNode], path: Option<&PathBuf>) {
    for node in nodes {
      if node_name(node) == "include-style" {
        if let (Some(cpath), Some(name)) = (path.and_then(|x| x.parent()), str_arg(node, 0)) {
          let style = cpath.join(name);

          if let Ok(content) = fs::read_to_string(style) {
            let doc: KdlDocument = match content.parse() {
              Ok(doc) => doc,
              Err(e) => {
                eprintln!("failed to parse style: {e}");
                return;
              }
            };

            self.parse_styles(doc.nodes(), path);
          }
        }
        continue;
      }
      if node_name(node) != "style" {
        continue;
      }
      let Some(name) = str_arg(node, 0) else {
        continue;
      };
      let mut style = Style::default();
      for child in node_children(node) {
        style.insert(node_name(child).to_string(), style_value(child));
      }
      self.insert_style(name.to_ustr(), style);
    }
  }

  fn parse_font(&mut self, nodes: &[KdlNode]) {
    let Some(name) = find_node(nodes, "font").and_then(|node| str_arg(node, 0)) else {
      return;
    };
    self.font = Font::with_name(Box::leak(name.to_string().into_boxed_str()));
  }

  fn parse_misc(&mut self, nodes: &[KdlNode]) {
    if let Some(int) = find_node(nodes, "tick-interval").and_then(|node| int_arg(node, 0)) {
      self.tick_interval = int as u64;
    }
    if let Some(backend) = find_node(nodes, "wgpu-backend").and_then(|node| str_arg(node, 0)) {
      self.wgpu_backend = Some(backend.into());
    }
  }

  pub fn style(&self, name: &str) -> Style {
    self.styles.get(name).cloned().unwrap_or_default()
  }

  pub fn style_any(&self, names: &[&str]) -> Style {
    for name in names {
      let style = self.style(name);
      if !style.is_empty() {
        return style;
      }
    }
    Style::default()
  }

  pub fn font(&self) -> Font {
    self.font
  }

  fn insert_style(&mut self, name: Ustr, style: Style) {
    if let Some(existing) = self.styles.get_mut(&name) {
      existing.extend(style);
    } else {
      self.styles.insert(name, style);
    }
  }

  pub fn typed<T: 'static>(&self) -> Option<&T> {
    self
      .parsed
      .get(&TypeId::of::<T>())
      .and_then(|x| x.downcast_ref::<T>())
  }
}

fn style_value(node: &KdlNode) -> style::StyleValue {
  use style::StyleValue;
  match value_arg(node, 0) {
    Some(KdlValue::Bool(b)) => StyleValue::Boolean(*b),
    Some(KdlValue::Integer(i)) => StyleValue::Integer(*i as i32),
    Some(KdlValue::Float(f)) => StyleValue::Float(*f as f32),
    Some(KdlValue::String(s)) => StyleValue::from(s.clone()),
    _ => StyleValue::from(String::new()),
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
    Self {
      styles: HashMap::new(),
      theme: Theme::default(),
      font: Font::with_name(DEFAULT_FONT),
      parsed: HashMap::new(),
      tick_interval: 3,
      wgpu_backend: None,
      current_path: None,
      config_parsers: Vec::new(),
    }
  }
}

pub const FONT: &[u8] = include_bytes!("../../../assets/lexend.ttf");

#[derive(Clone, Copy)]
pub struct ConfigParser {
  pub type_id: TypeId,
  pub de: fn(&[KdlNode]) -> anyhow::Result<Option<Box<dyn Any + Send + Sync>>>,
}
