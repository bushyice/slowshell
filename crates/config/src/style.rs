use std::sync::LazyLock;

use iced::Color;

pub(crate) static DEFAULT_STYLES: LazyLock<std::sync::Arc<std::sync::Mutex<Vec<String>>>> =
  LazyLock::new(|| std::sync::Arc::new(std::sync::Mutex::new(Vec::new())));

pub fn new_default_style(css: impl Into<String>) {
  DEFAULT_STYLES.lock().unwrap().push(css.into());
}

pub fn default_css() -> String {
  let styles = DEFAULT_STYLES.lock().unwrap();
  let mut css = String::new();
  for fragment in styles.iter() {
    css.push_str(fragment);
    if !css.ends_with('\n') {
      css.push('\n');
    }
  }
  css
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
      base: Color::from_rgb8(14, 28, 36),   // #0e1c24
      crust: Color::from_rgb8(6, 16, 22),   // #061016
      mantle: Color::from_rgb8(10, 21, 28), // #0a151c

      primary: Color::from_rgb8(130, 182, 204),   // #82b6cc
      secondary: Color::from_rgb8(162, 211, 226), // #a2d3e2

      green: Color::from_rgb8(143, 214, 207),  // #8fd6cf
      red: Color::from_rgb8(111, 168, 192),    // #6fa8c0
      blue: Color::from_rgb8(143, 214, 234),   // #8fd6ea
      yellow: Color::from_rgb8(159, 208, 224), // #9fd0e0
      orange: Color::from_rgb8(143, 198, 220), // #8fc6dc

      text: Color::from_rgb8(219, 234, 241),    // #dbeaf1
      subtext: Color::from_rgb8(176, 205, 217), // #b0cdd9
      overlay: Color::from_rgb8(143, 179, 194), // #8fb3c2
    }
  }
}

const ALPHAS: &[u8] = &[
  5, 10, 15, 18, 20, 25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 75, 80, 85, 90, 95,
];

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

  pub fn to_css(&self) -> String {
    let entries = [
      ("base", self.base),
      ("crust", self.crust),
      ("mantle", self.mantle),
      ("primary", self.primary),
      ("secondary", self.secondary),
      ("green", self.green),
      ("red", self.red),
      ("blue", self.blue),
      ("yellow", self.yellow),
      ("orange", self.orange),
      ("text", self.text),
      ("subtext", self.subtext),
      ("overlay", self.overlay),
    ];

    let mut body = String::new();
    for (name, color) in entries {
      body.push_str(&format!("  --{name}: {};\n", color_hex(color)));
      let [r, g, b, _] = color.into_rgba8();
      for alpha in ALPHAS {
        body.push_str(&format!(
          "  --{name}-a{alpha:02}: rgba({r}, {g}, {b}, {});\n",
          *alpha as f32 / 100.0
        ));
      }
    }

    format!(":root {{\n{body}}}\n")
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

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TextStyle {
  pub size: Option<f32>,
  pub color: Option<Color>,
  pub weight: Option<iced::font::Weight>,
}

pub fn container_style(
  computed: &icss::theme::resolve::computed::ComputedStyle,
) -> Option<iced::widget::container::Style> {
  let background = computed
    .color("background-color")
    .map(|color| iced::Background::Color(color.to_iced()));
  let text_color = computed.color("color").map(|color| color.to_iced());
  let border_width = computed.length("border-width").unwrap_or(0.0);
  let border_radius = computed.length("border-radius").unwrap_or(0.0);
  let border_color = computed
    .color("border-color")
    .map(|color| color.to_iced())
    .unwrap_or(Color::TRANSPARENT);
  let shadow = computed
    .shadow("box-shadow")
    .map(|(x, y, blur, color)| iced::Shadow {
      color: color.to_iced(),
      offset: iced::Vector::new(x, y),
      blur_radius: blur,
    })
    .unwrap_or_default();

  if background.is_none() && text_color.is_none() && border_width == 0.0 {
    return None;
  }

  Some(iced::widget::container::Style {
    background,
    text_color,
    border: iced::Border {
      color: border_color,
      width: border_width,
      radius: border_radius.into(),
    },
    shadow,
    snap: false,
  })
}

fn color_hex(color: Color) -> String {
  let [r, g, b, a] = color.into_rgba8();
  if a == 255 {
    format!("#{r:02x}{g:02x}{b:02x}")
  } else {
    format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
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

pub fn parse_image_path(value: &str) -> Option<String> {
  let value = value.trim();
  if value.is_empty() || value.eq_ignore_ascii_case("none") {
    return None;
  }

  let inner = if let Some(open) = value.find('(') {
    let rest = &value[open + 1..];
    let end = rest.rfind(')').unwrap_or(rest.len());
    rest[..end].trim()
  } else {
    value
  };

  let inner = inner.trim().trim_matches(|c| c == '"' || c == '\'');
  if inner.is_empty() || inner.eq_ignore_ascii_case("none") {
    return None;
  }

  Some(inner.to_string())
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
  fn alpha_variants_exist() {
    let css = Theme::default().to_css();
    assert!(css.contains("--base-a45:"));
    assert!(css.contains("--primary-a18:"));
  }

  #[test]
  fn image_paths_are_extracted_from_raw_values() {
    assert_eq!(parse_image_path("/a/b.png").as_deref(), Some("/a/b.png"));
    assert_eq!(
      parse_image_path("\"/a b.png\"").as_deref(),
      Some("/a b.png")
    );
    assert_eq!(
      parse_image_path("url(\"/a.png\")").as_deref(),
      Some("/a.png")
    );
    assert_eq!(
      parse_image_path("url ( \"/a.png\" )").as_deref(),
      Some("/a.png")
    );
    assert_eq!(
      parse_image_path("url ( \"/a(b).png\" )").as_deref(),
      Some("/a(b).png")
    );
    assert_eq!(
      parse_image_path("'~/watch.png'").as_deref(),
      Some("~/watch.png")
    );
    assert_eq!(parse_image_path("none"), None);
    assert_eq!(parse_image_path("NONE"), None);
    assert_eq!(parse_image_path(""), None);
    assert_eq!(parse_image_path("url()"), None);
  }

  #[test]
  fn default_css_is_plain_css() {
    let css = ".test { background-color: var(--primary); }";
    let theme = icss::Theme::load(&format!("{}\n{css}", Theme::default().to_css())).unwrap();
    let computed = theme.resolve(&["test"], None);
    assert!(computed.color("background-color").is_some());
  }
}
