use std::{
  collections::{HashMap, HashSet},
  path::{Path, PathBuf},
  sync::{Mutex, OnceLock},
};

use iced::{
  Color, Element, Length,
  widget::{image, svg, text},
};
use slowshell_core::types::Ustr;

pub const DEFAULT_ICON_COLOR: Color = Color::from_rgb(0.57, 0.6, 0.7);

static FALLBACK_GLYPH: &str = "\u{25CF}";
static ICON_THEME: OnceLock<String> = OnceLock::new();
static INSTALLED_THEMES: OnceLock<HashSet<String>> = OnceLock::new();

#[derive(Clone)]
enum IconAsset {
  Svg(svg::Handle),
  Png(image::Handle),
}

type IconAssetCache = Mutex<HashMap<(Ustr, u16), Option<IconAsset>>>;
static ICON_ASSET_CACHE: OnceLock<IconAssetCache> = OnceLock::new();

fn installed_themes() -> &'static HashSet<String> {
  INSTALLED_THEMES.get_or_init(|| freedesktop_icons::list_themes().into_iter().collect())
}

fn theme() -> &'static str {
  ICON_THEME.get_or_init(|| {
    let installed = installed_themes();
    let mut candidates: Vec<String> = Vec::new();

    if let Ok(theme) = std::env::var("SLOWSHELL_ICON_THEME")
      && !theme.trim().is_empty()
    {
      candidates.push(theme);
    }

    if let Some(theme) = {
      let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));

      if let Some(config_home) = config_home {
        if let Some(theme) = read_icon_theme_ini(&config_home.join("gtk-3.0/settings.ini"))
          .or_else(|| read_icon_theme_ini(&config_home.join("gtk-4.0/settings.ini")))
        {
          Some(theme)
        } else {
          None
        }
      } else if let Some(home) = std::env::var_os("HOME")
        && let Some(theme) = read_icon_theme_ini(&PathBuf::from(home).join(".gtkrc-2.0"))
      {
        Some(theme)
      } else {
        None
      }
    } {
      candidates.push(theme);
    }

    if let Some(theme) = first_installed(&candidates, installed) {
      return theme.clone();
    }

    if let Some(theme) = freedesktop_icons::default_theme_gtk()
      && installed.contains(&theme)
    {
      return theme;
    }

    for fallback in ["Adwaita", "hicolor"] {
      if installed.contains(fallback) {
        return fallback.into();
      }
    }

    candidates
      .into_iter()
      .find(|candidate| !candidate.trim().is_empty())
      .unwrap_or_else(|| "hicolor".into())
  })
}

fn first_installed<'a>(
  candidates: &'a [String],
  installed: &HashSet<String>,
) -> Option<&'a String> {
  candidates
    .iter()
    .find(|candidate| installed.contains(*candidate))
}

fn read_icon_theme_ini(path: &Path) -> Option<String> {
  let content = std::fs::read_to_string(path).ok()?;

  parse_icon_theme(&content)
}

fn parse_icon_theme(content: &str) -> Option<String> {
  for line in content.lines() {
    let line = line.trim();

    if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
      continue;
    }

    let Some((key, value)) = line.split_once('=') else {
      continue;
    };

    if key.trim() != "gtk-icon-theme-name" {
      continue;
    }

    let value = value.trim().trim_matches(|c| c == '"' || c == '\'').trim();
    if !value.is_empty() {
      return Some(value.to_string());
    }
  }

  None
}

fn lookup_w_fallback(name: &str, size: u16) -> Option<PathBuf> {
  let mut themes = vec![theme()];

  for fallback in ["Adwaita", "hicolor"] {
    if !themes.contains(&fallback) {
      themes.push(fallback);
    }
  }

  for theme in themes {
    let mut query = freedesktop_icons::lookup(name)
      .with_size(size)
      .with_theme(theme)
      .with_cache();

    if name.contains("symbolic") {
      query = query.force_svg();
    }

    if let Some(path) = query.find() {
      return Some(path);
    }
  }

  None
}

fn is_svg(path: &Path) -> bool {
  path
    .extension()
    .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
}

fn get_asset(name: &str, size: u16) -> Option<IconAsset> {
  let cache = ICON_ASSET_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
  let key = (Ustr::from(name), size);

  if let Ok(guard) = cache.lock()
    && let Some(asset) = guard.get(&key)
  {
    return asset.clone();
  }

  let asset = resolve_icon(name, size);

  if let Ok(mut guard) = cache.lock() {
    guard.insert(key, asset.clone());
  }

  asset
}

fn resolve_icon(name: &str, size: u16) -> Option<IconAsset> {
  if name.starts_with("slowshell-icon") {
    let path = Path::new(name);
    if is_svg(path) {
      return Some(IconAsset::Svg(
        builtin_svg(name).unwrap_or_else(|| svg::Handle::from_path(path)),
      ));
    }
    return Some(IconAsset::Png(
      builtin_png(name).unwrap_or_else(|| image::Handle::from_path(path)),
    ));
  }

  let path = lookup_w_fallback(name, size)?;
  if is_svg(&path) {
    Some(IconAsset::Svg(svg::Handle::from_path(&path)))
  } else {
    Some(IconAsset::Png(image::Handle::from_path(&path)))
  }
}

fn builtin_svg(name: &str) -> Option<svg::Handle> {
  Some(match name {
    "slowshell-icon.svg" => svg::Handle::from_memory(include_bytes!("../../../assets/icon.svg")),
    "slowshell-icon-shell.svg" => {
      svg::Handle::from_memory(include_bytes!("../../../assets/icon-shell.svg"))
    }
    "slowshell-icon-filled.svg" => {
      svg::Handle::from_memory(include_bytes!("../../../assets/icon-filled.svg"))
    }
    _ => return None,
  })
}

fn builtin_png(name: &str) -> Option<image::Handle> {
  Some(match name {
    "slowshell-icon.png" => {
      image::Handle::from_bytes(include_bytes!("../../../assets/icon.png").to_vec())
    }
    "slowshell-icon-shell.png" => {
      image::Handle::from_bytes(include_bytes!("../../../assets/icon-shell.png").to_vec())
    }
    "slowshell-icon-filled.png" => {
      image::Handle::from_bytes(include_bytes!("../../../assets/icon-filled.png").to_vec())
    }
    _ => return None,
  })
}

pub struct Icon<Message> {
  names: Box<[String]>,
  size: u16,
  color: Option<Color>,
  _message: std::marker::PhantomData<Message>,
}

impl<Message> Icon<Message> {
  pub fn new(name: impl Into<String>) -> Self {
    Self {
      names: vec![name.into()].into_boxed_slice(),
      size: 16,
      color: None,
      _message: std::marker::PhantomData,
    }
  }

  pub fn any(names: impl IntoIterator<Item = impl Into<String>>) -> Self {
    Self {
      names: names.into_iter().map(Into::into).collect(),
      size: 16,
      color: None,
      _message: std::marker::PhantomData,
    }
  }

  pub fn size(mut self, size: u16) -> Self {
    self.size = size;
    self
  }

  pub fn color(mut self, color: Color) -> Self {
    self.color = Some(color);
    self
  }

  pub fn resolved_name(&self) -> Option<&str> {
    self
      .names
      .iter()
      .find(|name| get_asset(name, self.size).is_some())
      .map(String::as_str)
  }

  pub fn from_bytes(id: i32, bytes: &[u8]) -> iced::widget::image::Handle {
    get_or_create_byte_icon(id, bytes)
  }

  pub fn from_pixmap(
    address: impl Into<Ustr>,
    w: u32,
    h: u32,
    rgba: &[u8],
  ) -> iced::widget::image::Handle {
    get_or_create_pixmap_handle(address.into(), w, h, rgba)
  }

  pub fn into_element(self) -> Element<'static, Message>
  where
    Message: 'static,
  {
    let asset = self
      .names
      .iter()
      .find_map(|name| get_asset(name, self.size));

    match asset {
      Some(IconAsset::Svg(handle)) => {
        let size = Length::Fixed(self.size as f32);
        let mut widget = svg(handle).width(size).height(size);

        if let Some(color) = self.color {
          widget = widget.style(move |_, _| svg::Style { color: Some(color) });
        }

        widget.into()
      }

      Some(IconAsset::Png(handle)) => {
        let size = Length::Fixed(self.size as f32);
        image(handle).width(size).height(size).into()
      }

      None => text(FALLBACK_GLYPH)
        .size(self.size as f32)
        .color(self.color.unwrap_or(DEFAULT_ICON_COLOR))
        .width(Length::Fixed(self.size as f32))
        .height(Length::Fixed(self.size as f32))
        .into(),
    }
  }
}

impl<Message> From<Icon<Message>> for Element<'static, Message>
where
  Message: 'static,
{
  fn from(icon: Icon<Message>) -> Self {
    icon.into_element()
  }
}

// stupid shit
static BYTE_CACHE: std::sync::OnceLock<
  std::sync::Mutex<HashMap<i32, iced::widget::image::Handle>>,
> = std::sync::OnceLock::new();

fn get_or_create_byte_icon(id: i32, bytes: &[u8]) -> iced::widget::image::Handle {
  let cache = BYTE_CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
  if let Ok(mut guard) = cache.lock() {
    if let Some(handle) = guard.get(&id) {
      return handle.clone();
    }
    let handle = iced::widget::image::Handle::from_bytes(bytes.to_vec());
    guard.insert(id, handle.clone());
    handle
  } else {
    iced::widget::image::Handle::from_bytes(bytes.to_vec())
  }
}

static PIXMAP_CACHE: std::sync::OnceLock<Mutex<HashMap<Ustr, (u32, u32, image::Handle)>>> =
  std::sync::OnceLock::new();

fn get_or_create_pixmap_handle(address: Ustr, w: u32, h: u32, rgba: &[u8]) -> image::Handle {
  let cache = PIXMAP_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
  if let Ok(mut guard) = cache.lock() {
    if let Some((cw, ch, handle)) = guard.get(&address) {
      if *cw == w && *ch == h {
        return handle.clone();
      }
    }
    let handle = image::Handle::from_rgba(w, h, rgba.to_vec());
    guard.insert(address, (w, h, handle.clone()));
    handle
  } else {
    image::Handle::from_rgba(w, h, rgba.to_vec())
  }
}
