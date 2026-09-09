use std::{
  collections::HashMap,
  path::PathBuf,
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

static IMG_HANDLE_CACHE: OnceLock<
  std::sync::Mutex<std::collections::HashMap<PathBuf, image::Handle>>,
> = OnceLock::new();

static SVG_HANDLE_CACHE: OnceLock<
  std::sync::Mutex<std::collections::HashMap<PathBuf, svg::Handle>>,
> = OnceLock::new();

static ICON_PATH_CACHE: OnceLock<Mutex<HashMap<(String, u16), Option<PathBuf>>>> = OnceLock::new();

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

  fn theme() -> &'static str {
    ICON_THEME.get_or_init(|| {
      std::env::var("SLOWSHELL_ICON_THEME")
        .ok()
        .or_else(freedesktop_icons::default_theme_gtk)
        .unwrap_or_else(|| "hicolor".into())
    })
  }

  fn find(&self, name: &str) -> Option<PathBuf> {
    if name.starts_with("slowshell-icon") {
      return Some(name.into());
    }

    let cache = ICON_PATH_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (name.to_string(), self.size);

    if let Ok(guard) = cache.lock() {
      if let Some(cached) = guard.get(&key) {
        return cached.clone();
      }
    }

    let mut query = freedesktop_icons::lookup(name)
      .with_size(self.size)
      .with_theme(Self::theme())
      .with_cache();

    if name.contains("symbolic") {
      query = query.force_svg();
    }

    let result = query.find();

    if let Ok(mut guard) = cache.lock() {
      guard.insert(key, result.clone());
    }

    result
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
    let path = self.names.iter().find_map(|name| self.find(name));

    match path {
      Some(path) => {
        let is_svg = path
          .extension()
          .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"));

        let size = Length::Fixed(self.size as f32);

        if is_svg {
          let cache = SVG_HANDLE_CACHE.get_or_init(|| {
            let mut map = HashMap::new();

            map.insert(
              "slowshell-icon.svg".into(),
              svg::Handle::from_memory(include_bytes!("../../../assets/icon.svg")),
            );

            map.insert(
              "slowshell-icon-shell.svg".into(),
              svg::Handle::from_memory(include_bytes!("../../../assets/icon-shell.svg")),
            );

            map.insert(
              "slowshell-icon-filled.svg".into(),
              svg::Handle::from_memory(include_bytes!("../../../assets/icon-filled.svg")),
            );

            std::sync::Mutex::new(map)
          });
          let handle = if let Ok(mut guard) = cache.lock() {
            guard
              .entry(path.clone())
              .or_insert_with(|| svg::Handle::from_path(&path))
              .clone()
          } else {
            svg::Handle::from_path(&path)
          };

          let mut widget = svg(handle).width(size).height(size);

          if let Some(color) = self.color {
            widget = widget.style(move |_, _| svg::Style { color: Some(color) });
          }

          widget.into()
        } else {
          let cache = IMG_HANDLE_CACHE.get_or_init(|| {
            let mut map = HashMap::new();

            // TODO: don't
            map.insert(
              "slowshell-icon.png".into(),
              image::Handle::from_bytes(include_bytes!("../../../assets/icon.png").to_vec()),
            );

            map.insert(
              "slowshell-icon-shell.png".into(),
              image::Handle::from_bytes(include_bytes!("../../../assets/icon-shell.png").to_vec()),
            );

            map.insert(
              "slowshell-icon-filled.png".into(),
              image::Handle::from_bytes(include_bytes!("../../../assets/icon-filled.png").to_vec()),
            );

            std::sync::Mutex::new(map)
          });
          let handle = if let Ok(mut guard) = cache.lock() {
            guard
              .entry(path.clone())
              .or_insert_with(|| image::Handle::from_path(&path))
              .clone()
          } else {
            image::Handle::from_path(&path)
          };

          image(handle).width(size).height(size).into()
        }
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
