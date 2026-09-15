use std::{collections::HashMap, sync::LazyLock};

use iced::Color;
use iced::widget::container;
use slowshell_core::types::Ustr;

#[derive(Debug, Clone)]
pub enum ColorValue {
  Transparent,
  Theme(String),
  Color(String), // don't use
}

#[derive(Debug, Clone)]
pub enum StyleValue {
  String(String),
  Integer(i32),
  Float(f32),
  Boolean(bool),
  Color(ColorValue),
}

impl StyleValue {
  pub fn as_bool(&self) -> bool {
    match self {
      Self::Boolean(b) => *b,
      Self::String(s) => s == "true",
      _ => false,
    }
  }

  pub fn as_str(&self) -> &str {
    match self {
      Self::String(s) => s,
      _ => "",
    }
  }

  pub fn as_int(&self) -> i32 {
    match self {
      Self::Integer(i) => *i,
      Self::Float(f) => *f as i32,
      _ => 0,
    }
  }

  pub fn as_float(&self) -> f32 {
    match self {
      Self::Integer(i) => *i as f32,
      Self::Float(f) => *f,
      _ => 0.,
    }
  }
}

impl From<i32> for StyleValue {
  fn from(value: i32) -> Self {
    StyleValue::Integer(value)
  }
}

impl From<f32> for StyleValue {
  fn from(value: f32) -> Self {
    StyleValue::Float(value)
  }
}

impl From<bool> for StyleValue {
  fn from(value: bool) -> Self {
    StyleValue::Boolean(value)
  }
}

impl From<&str> for StyleValue {
  fn from(value: &str) -> Self {
    StyleValue::from(value.to_string())
  }
}

impl From<String> for StyleValue {
  fn from(value: String) -> Self {
    if value.eq_ignore_ascii_case("transparent") {
      return StyleValue::Color(ColorValue::Transparent);
    }
    if value.starts_with('#') && parse_hex(&value).is_some() {
      return StyleValue::Color(ColorValue::Color(value));
    }
    StyleValue::String(value)
  }
}

impl From<ColorValue> for StyleValue {
  fn from(value: ColorValue) -> Self {
    StyleValue::Color(value)
  }
}

#[derive(Debug, Clone, Default)]
pub struct Style {
  map: HashMap<String, StyleValue>,
}

impl Style {
  pub fn insert(&mut self, key: impl Into<String>, value: impl Into<StyleValue>) {
    self.map.insert(key.into(), value.into());
  }

  pub fn get(&self, key: &str) -> Option<&StyleValue> {
    self.map.get(key)
  }

  pub fn entries(&self) -> impl Iterator<Item = (&str, &StyleValue)> {
    self.map.iter().map(|(key, value)| (key.as_str(), value))
  }

  pub fn extend(&mut self, other: Style) {
    self.map.extend(other.map);
  }

  pub fn is_empty(&self) -> bool {
    self.map.is_empty()
  }

  pub fn number(&self, key: &str) -> Option<f32> {
    match self.get(key)? {
      StyleValue::Integer(i) => Some(*i as f32),
      StyleValue::Float(f) => Some(*f),
      StyleValue::String(s) => s.parse().ok(),
      _ => None,
    }
  }

  pub fn bool(&self, key: &str) -> bool {
    self.get(key).map(StyleValue::as_bool).unwrap_or(false)
  }

  pub fn get_color(&self, theme: &Theme, key: &str) -> Option<Color> {
    let color = match self.get(key)? {
      StyleValue::Color(ColorValue::Transparent) => Color::TRANSPARENT,
      StyleValue::Color(ColorValue::Theme(name)) => theme.color(name)?,
      StyleValue::Color(ColorValue::Color(hex)) => parse_hex(hex)?,
      StyleValue::String(name) => theme.color(name)?,
      _ => return None,
    };

    Some(apply_opacity(color, self.number(&format!("{key}.opacity"))))
  }

  pub fn color(&self, theme: &Theme, key: &str, default: Color) -> Color {
    self.get_color(theme, key).unwrap_or(default)
  }

  pub fn padding(&self, default: impl Into<[f32; 2]>) -> [f32; 2] {
    if let Some(x) = self.number("padding.x") {
      let y = self.number("padding.y").unwrap_or(x);
      return [y, x];
    }
    if let Some(p) = self.number("padding") {
      return [p, p];
    }
    if let Some(StyleValue::String(s)) = self.get("padding") {
      let trimmed = s.trim();
      if let Some(rest) = trimmed.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        let mut parts = rest.split(',').filter_map(|p| p.trim().parse::<f32>().ok());
        if let (Some(x), Some(y)) = (parts.next(), parts.next()) {
          return [y, x];
        }
      }
      if let Ok(p) = trimmed.parse() {
        return [p, p];
      }
    }
    default.into()
  }

  pub fn container_style(&self, theme: &Theme) -> container::Style {
    let mut style = container::Style::default();

    if let Some(background) = self.get_color(theme, "background") {
      style.background = Some(background.into());
    }

    style.border.color = self.get_color(theme, "border.color").unwrap_or_default();
    style.border.width = self.number("border.width").unwrap_or(0.0);
    if let Some(radius) = self.number("radius") {
      style.border.radius = radius.into();
    }

    // if let Some(color) = self.get_color(theme, "shadow.color") {
    //   style.shadow.color = color;
    // }
    // style.shadow.blur_radius = self
    //   .number("shadow.blur")
    //   .unwrap_or(style.shadow.blur_radius);

    style
  }
}

#[macro_export]
macro_rules! style {
  ($($key:literal => $value:expr),* $(,)?) => {
    {
      let mut __style = $crate::style::Style::default();
      $(
        __style.insert($key, $value);
      )*
      __style
    }
  };
}

pub type StyleMap = Style;

pub(crate) static DEFAULT_STYLES: LazyLock<std::sync::Arc<std::sync::Mutex<HashMap<Ustr, Style>>>> =
  LazyLock::new(|| std::sync::Arc::new(std::sync::Mutex::new(Default::default())));

pub fn new_default_style(name: Ustr, style: Style) {
  DEFAULT_STYLES.lock().unwrap().insert(name, style);
}

fn apply_opacity(color: Color, opacity: Option<f32>) -> Color {
  match opacity {
    Some(op) => Color::from_rgba(color.r, color.g, color.b, (color.a * op).clamp(0.0, 1.0)),
    None => color,
  }
}

pub fn parse_hex(hex: &str) -> Option<Color> {
  let hex = hex.strip_prefix('#').unwrap_or(hex);
  let channels = match hex.len() {
    3 | 6 => 3,
    4 | 8 => 4,
    _ => return None,
  };
  let step = if hex.len() <= 4 { 1 } else { 2 };

  let mut nibbles = [0u8; 8];
  for (i, byte) in hex.bytes().enumerate() {
    nibbles[i] = match byte {
      b'0'..=b'9' => byte - b'0',
      b'a'..=b'f' => byte - b'a' + 10,
      b'A'..=b'F' => byte - b'A' + 10,
      _ => return None,
    };
  }

  let mut out = [0u8; 4];
  out[3] = 255;
  for i in 0..channels {
    if step == 1 {
      out[i] = nibbles[i] * 16 + nibbles[i];
    } else {
      out[i] = nibbles[2 * i] * 16 + nibbles[2 * i + 1];
    }
  }

  Some(Color::from_rgba8(
    out[0],
    out[1],
    out[2],
    out[3] as f32 / 255.0,
  ))
}

#[derive(Debug)]
pub struct Theme {
  pub base: Color,
  pub crust: Color,
  pub mantle: Color,

  pub primary: Color,
  pub secondary: Color,

  pub green: Color,
  pub red: Color,
  pub blue: Color,
  pub yellow: Color,
  pub orange: Color,

  pub text: Color,
  pub subtext: Color,
  pub overlay: Color,
}

impl Default for Theme {
  fn default() -> Self {
    Self {
      base: Color::from_rgb8(30, 30, 46),
      crust: Color::from_rgb8(17, 17, 27),
      mantle: Color::from_rgb8(24, 24, 37),

      primary: Color::from_rgb8(235, 160, 172),
      secondary: Color::from_rgb8(203, 166, 247),

      green: Color::from_rgb8(148, 226, 213),
      red: Color::from_rgb8(243, 139, 168),
      blue: Color::from_rgb8(137, 220, 235),
      yellow: Color::from_rgb8(249, 226, 175),
      orange: Color::from_rgb8(250, 179, 135),

      text: Color::from_rgb8(205, 214, 244),
      subtext: Color::from_rgb8(186, 194, 222),
      overlay: Color::from_rgb8(147, 153, 178),
    }
  }
}

impl Theme {
  pub fn color(&self, name: &str) -> Option<Color> {
    Some(match name {
      "base" => self.base,
      "crust" => self.crust,
      "mantle" => self.mantle,
      "primary" => self.primary,
      "secondary" => self.secondary,
      "green" => self.green,
      "red" => self.red,
      "blue" => self.blue,
      "yellow" => self.yellow,
      "orange" => self.orange,
      "text" => self.text,
      "subtext" => self.subtext,
      "overlay" => self.overlay,
      // base/0.2
      _ if let Some((name, value)) = name.split_once("/") => {
        let mut color = self.color(name)?;
        color.a = value.parse::<f32>().ok()?;
        color
      }
      _ => return None,
    })
  }

  pub fn overlay_opacity(&self, opacity: f32) -> Color {
    let mut overlay = self.overlay;
    overlay.a = opacity;
    overlay
  }

  pub(crate) fn apply(&mut self, key: &str, hex: &str) {
    if let Some(color) = parse_hex(hex) {
      match key {
        "base" => self.base = color,
        "crust" => self.crust = color,
        "mantle" => self.mantle = color,
        "primary" => self.primary = color,
        "secondary" => self.secondary = color,
        "green" => self.green = color,
        "red" => self.red = color,
        "blue" => self.blue = color,
        "yellow" => self.yellow = color,
        "orange" => self.orange = color,
        "text" => self.text = color,
        "subtext" => self.subtext = color,
        "overlay" => self.overlay = color,
        _ => {}
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn hex_parsing() {
    assert_eq!(parse_hex("#ff00ff"), Some(Color::from_rgb8(255, 0, 255)));
    assert_eq!(parse_hex("#fff"), Some(Color::from_rgb8(255, 255, 255)));
    assert_eq!(
      parse_hex("#ff00ff80"),
      Some(Color::from_rgba8(255, 0, 255, 128.0 / 255.0))
    );
    assert_eq!(parse_hex("nope"), None);
  }

  #[test]
  fn style_mining() {
    let style = style! {
      "background" => "primary",
      "padding.x" => 8,
      "padding.y" => 4,
    };
    let theme = Theme::default();
    assert_eq!(style.padding([0.0, 0.0]), [4.0, 8.0]);
    assert_eq!(
      style.color(&theme, "background", Color::BLACK),
      theme.primary
    );
  }
}
