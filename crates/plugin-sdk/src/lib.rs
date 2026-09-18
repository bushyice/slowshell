use core::ffi::c_void;
use std::any::Any;
use std::collections::HashMap;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Mutex, OnceLock};

pub use slowshell_plugin::*;

pub use kdl;

#[cfg(feature = "test-host")]
pub mod test_host;

static API: AtomicPtr<SlHostApi> = AtomicPtr::new(core::ptr::null_mut());

#[doc(hidden)]
pub fn set_api(api: *const SlHostApi) {
  API.store(api as *mut SlHostApi, Ordering::Relaxed);
}

fn api() -> *const SlHostApi {
  API.load(Ordering::Relaxed)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
  pub r: f32,
  pub g: f32,
  pub b: f32,
  pub a: f32,
}

impl Color {
  pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);
  pub const BLACK: Self = Self::rgb(0.0, 0.0, 0.0);
  pub const TRANSPARENT: Self = Self::rgba(0.0, 0.0, 0.0, 0.0);

  pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
    Self { r, g, b, a: 1.0 }
  }

  pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
    Self { r, g, b, a }
  }
}

impl From<Color> for SlColor {
  fn from(color: Color) -> Self {
    SlColor {
      r: color.r,
      g: color.g,
      b: color.b,
      a: color.a,
    }
  }
}

impl Default for Color {
  fn default() -> Self {
    Self::TRANSPARENT
  }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Border {
  pub width: f32,
  pub radius: f32,
  pub color: Color,
}

impl From<&Border> for SlBorder {
  fn from(border: &Border) -> Self {
    SlBorder {
      width: border.width,
      radius: border.radius,
      color: border.color.into(),
    }
  }
}

#[derive(Clone, Debug, Default)]
pub struct Style {
  pub background: Option<Color>,
  pub border: Option<Border>,
  pub text_color: Option<Color>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Padding {
  pub top: f32,
  pub right: f32,
  pub bottom: f32,
  pub left: f32,
}

impl Padding {
  pub const ZERO: Self = Self {
    top: 0.0,
    right: 0.0,
    bottom: 0.0,
    left: 0.0,
  };

  pub const fn all(value: f32) -> Self {
    Self {
      top: value,
      right: value,
      bottom: value,
      left: value,
    }
  }

  fn to_array(&self) -> [f32; 4] {
    [self.top, self.right, self.bottom, self.left]
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
  Start,
  Center,
  End,
  Fill,
}

impl Align {
  fn to_u32(&self) -> u32 {
    match self {
      Align::Start => sl_align::START,
      Align::Center => sl_align::CENTER,
      Align::End => sl_align::END,
      Align::Fill => sl_align::FILL,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Length {
  Shrink,
  Fill,
  Fixed(f32),
}

impl Length {
  fn to_sl(self) -> SlLength {
    match self {
      Length::Shrink => SlLength {
        unit: sl_length_unit::SHRINK,
        value: 0.0,
      },
      Length::Fill => SlLength {
        unit: sl_length_unit::FILL,
        value: 0.0,
      },
      Length::Fixed(value) => SlLength {
        unit: sl_length_unit::FIXED,
        value,
      },
    }
  }
}

#[derive(Clone, Copy, Debug)]
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
    let rgb =
      |r: u8, g: u8, b: u8| Color::rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    Self {
      base: rgb(14, 28, 36),
      crust: rgb(6, 16, 22),
      mantle: rgb(10, 21, 28),
      primary: rgb(130, 182, 204),
      secondary: rgb(162, 211, 226),
      green: rgb(143, 214, 207),
      red: rgb(111, 168, 192),
      blue: rgb(143, 214, 234),
      yellow: rgb(159, 208, 224),
      orange: rgb(143, 198, 220),
      text: rgb(219, 234, 241),
      subtext: rgb(176, 205, 217),
      overlay: rgb(143, 179, 194),
    }
  }
}

impl Theme {
  pub fn color(&self, name: &str) -> Option<Color> {
    let color = match name {
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
      _ if let Some((name, value)) = name.split_once("/") => {
        let mut color = self.color(name)?;
        color.a = value.parse::<f32>().ok()?;
        return Some(color);
      }
      _ => return None,
    };
    Some(color)
  }

  fn from_sl(theme: SlTheme) -> Self {
    let color = |color: SlColor| Color {
      r: color.r,
      g: color.g,
      b: color.b,
      a: color.a,
    };
    Self {
      base: color(theme.base),
      crust: color(theme.crust),
      mantle: color(theme.mantle),
      primary: color(theme.primary),
      secondary: color(theme.secondary),
      green: color(theme.green),
      red: color(theme.red),
      blue: color(theme.blue),
      yellow: color(theme.yellow),
      orange: color(theme.orange),
      text: color(theme.text),
      subtext: color(theme.subtext),
      overlay: color(theme.overlay),
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ColorValue {
  Transparent,
  Theme(String),
  Hex(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum StyleValue {
  Color(ColorValue),
  Integer(i32),
  Float(f32),
  String(String),
  Boolean(bool),
}

impl StyleValue {
  fn as_bool(&self) -> bool {
    match self {
      Self::Boolean(b) => *b,
      Self::String(s) => s == "true",
      _ => false,
    }
  }
}

impl From<ColorValue> for StyleValue {
  fn from(value: ColorValue) -> Self {
    StyleValue::Color(value)
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
      return StyleValue::Color(ColorValue::Hex(value));
    }
    StyleValue::String(value)
  }
}

#[derive(Clone, Debug, Default)]
pub struct StyleSheet {
  values: Vec<(String, StyleValue)>,
}

impl StyleSheet {
  pub fn insert(&mut self, key: impl Into<String>, value: impl Into<StyleValue>) {
    let key = key.into();
    let value = value.into();
    if let Some(entry) = self.values.iter_mut().find(|(k, _)| *k == key) {
      entry.1 = value;
    } else {
      self.values.push((key, value));
    }
  }

  pub fn get(&self, key: &str) -> Option<&StyleValue> {
    self
      .values
      .iter()
      .find(|(k, _)| k == key)
      .map(|(_, value)| value)
  }

  pub fn is_empty(&self) -> bool {
    self.values.is_empty()
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
      StyleValue::Color(ColorValue::Hex(hex)) => parse_hex(hex)?,
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

  fn from_sl(sheet: &SlStyleSheet) -> Self {
    if sheet.entries.is_null() {
      return Self::default();
    }
    let entries = unsafe { core::slice::from_raw_parts(sheet.entries, sheet.count as usize) };
    let mut values = Vec::with_capacity(entries.len());
    for entry in entries {
      let Some(key) = (unsafe { entry.key.as_str() }) else {
        continue;
      };
      let value = match entry.kind {
        sl_style_value_kind::COLOR => match entry.color_kind {
          sl_style_color_kind::THEME => StyleValue::Color(ColorValue::Theme(
            unsafe { entry.text.as_str() }.unwrap_or("").to_owned(),
          )),
          sl_style_color_kind::HEX => StyleValue::Color(ColorValue::Hex(
            unsafe { entry.text.as_str() }.unwrap_or("").to_owned(),
          )),
          _ => StyleValue::Color(ColorValue::Transparent),
        },
        sl_style_value_kind::INTEGER => StyleValue::Integer(entry.integer),
        sl_style_value_kind::FLOAT => StyleValue::Float(entry.float_value),
        sl_style_value_kind::BOOLEAN => StyleValue::Boolean(entry.boolean),
        _ => StyleValue::String(unsafe { entry.text.as_str() }.unwrap_or("").to_owned()),
      };
      values.push((key.to_owned(), value));
    }
    Self { values }
  }

  fn to_sl_entries(&self) -> (Vec<String>, Vec<SlStyleSheetEntry>) {
    let mut pool: Vec<String> = Vec::with_capacity(self.values.len() * 2);
    for (key, value) in &self.values {
      pool.push(key.clone());
      match value {
        StyleValue::String(s) => pool.push(s.clone()),
        StyleValue::Color(ColorValue::Theme(name)) | StyleValue::Color(ColorValue::Hex(name)) => {
          pool.push(name.clone())
        }
        _ => {}
      }
    }

    let mut entries: Vec<SlStyleSheetEntry> = Vec::with_capacity(self.values.len());
    let mut slot = 0usize;
    for (_, value) in &self.values {
      let key_sl = SlStr {
        ptr: pool[slot].as_ptr(),
        len: pool[slot].len(),
      };
      slot += 1;

      let mut entry = SlStyleSheetEntry {
        key: key_sl,
        ..SlStyleSheetEntry::default()
      };
      match value {
        StyleValue::String(_s) => {
          entry.kind = sl_style_value_kind::STRING;
          entry.text = SlStr {
            ptr: pool[slot].as_ptr(),
            len: pool[slot].len(),
          };
          slot += 1;
        }
        StyleValue::Integer(i) => {
          entry.kind = sl_style_value_kind::INTEGER;
          entry.integer = *i;
        }
        StyleValue::Float(f) => {
          entry.kind = sl_style_value_kind::FLOAT;
          entry.float_value = *f;
        }
        StyleValue::Boolean(b) => {
          entry.kind = sl_style_value_kind::BOOLEAN;
          entry.boolean = *b;
        }
        StyleValue::Color(color) => {
          entry.kind = sl_style_value_kind::COLOR;
          match color {
            ColorValue::Transparent => entry.color_kind = sl_style_color_kind::TRANSPARENT,
            ColorValue::Theme(_name) => {
              entry.color_kind = sl_style_color_kind::THEME;
              entry.text = SlStr {
                ptr: pool[slot].as_ptr(),
                len: pool[slot].len(),
              };
              slot += 1;
            }
            ColorValue::Hex(_name) => {
              entry.color_kind = sl_style_color_kind::HEX;
              entry.text = SlStr {
                ptr: pool[slot].as_ptr(),
                len: pool[slot].len(),
              };
              slot += 1;
            }
          }
        }
      }
      entries.push(entry);
    }
    (pool, entries)
  }
}

fn parse_hex(hex: &str) -> Option<Color> {
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

  Some(Color {
    r: out[0] as f32 / 255.0,
    g: out[1] as f32 / 255.0,
    b: out[2] as f32 / 255.0,
    a: out[3] as f32 / 255.0,
  })
}

fn apply_opacity(color: Color, opacity: Option<f32>) -> Color {
  match opacity {
    Some(op) => Color {
      r: color.r,
      g: color.g,
      b: color.b,
      a: (color.a * op).clamp(0.0, 1.0),
    },
    None => color,
  }
}

#[derive(Clone, Copy)]
pub struct Context {
  api: *const SlHostApi,
  ctx: *mut c_void,
  opts: *mut c_void,
}

impl Context {
  #[doc(hidden)]
  pub fn new(api: *const SlHostApi, ctx: *mut c_void) -> Self {
    Self {
      api,
      ctx,
      opts: core::ptr::null_mut(),
    }
  }

  #[doc(hidden)]
  pub fn with_opts(api: *const SlHostApi, ctx: *mut c_void, opts: *mut c_void) -> Self {
    Self { api, ctx, opts }
  }

  pub fn options(&self) -> Options<'_> {
    Options { ctx: self }
  }

  pub fn log(&self, level: i32, message: &str) {
    if self.api.is_null() {
      return;
    }
    if let Some(log) = unsafe { (*self.api).log } {
      unsafe { log(self.ctx, level, SlStr::from_str(message)) };
    }
  }

  pub fn error(&self, code: u32, message: &str) {
    if self.api.is_null() {
      return;
    }
    if let Some(report) = unsafe { (*self.api).plugin_error } {
      unsafe { report(self.ctx, code, SlStr::from_str(message)) };
    }
  }

  pub fn set_interval(&self, millis: u32, repeating: bool, tag: u32) -> i32 {
    if self.api.is_null() {
      return -1;
    }
    if let Some(set_interval) = unsafe { (*self.api).set_interval } {
      unsafe { set_interval(self.ctx, millis, repeating, tag) }
    } else {
      -1
    }
  }

  pub fn register_fd(&self, fd: i32, action: u32) -> i32 {
    if self.api.is_null() {
      return -1;
    }
    if let Some(register_fd) = unsafe { (*self.api).register_fd } {
      unsafe { register_fd(self.ctx, fd, action) }
    } else {
      -1
    }
  }

  pub fn register_compositor_fd(&self, fd: i32, flags: u32) -> i32 {
    if self.api.is_null() {
      return -1;
    }
    if let Some(register) = unsafe { (*self.api).compositor_register_fd } {
      unsafe { register(self.ctx, fd, flags) }
    } else {
      -1
    }
  }

  pub fn request_redraw(&self, window: u64) {
    if self.api.is_null() {
      return;
    }
    if let Some(request_redraw) = unsafe { (*self.api).request_redraw } {
      unsafe { request_redraw(self.ctx, window) };
    }
  }

  pub fn canvas(&self, width: u32, height: u32) -> Option<&'static mut [u8]> {
    if self.api.is_null() {
      return None;
    }
    let canvas = unsafe { (*self.api).canvas }?;
    let ptr = unsafe { canvas(self.ctx, width, height) };
    if ptr.is_null() {
      return None;
    }
    let len = width as usize * height as usize * 4;
    Some(unsafe { std::slice::from_raw_parts_mut(ptr, len) })
  }

  pub fn config_str(&self, key: &str) -> Option<String> {
    if self.api.is_null() {
      return None;
    }
    let get = unsafe { (*self.api).config_get_str }?;
    let mut out = SlStr::EMPTY;
    let rc = unsafe { get(self.ctx, SlStr::from_str(key), &mut out) };
    if rc != 0 {
      return None;
    }
    unsafe { out.as_str() }.map(str::to_owned)
  }

  pub fn config_f64(&self, key: &str) -> Option<f64> {
    if self.api.is_null() {
      return None;
    }
    let get = unsafe { (*self.api).config_get_f64 }?;
    let mut out = 0.0;
    if unsafe { get(self.ctx, SlStr::from_str(key), &mut out) } == 0 {
      Some(out)
    } else {
      None
    }
  }

  pub fn config_i64(&self, key: &str) -> Option<i64> {
    if self.api.is_null() {
      return None;
    }
    let get = unsafe { (*self.api).config_get_i64 }?;
    let mut out = 0;
    if unsafe { get(self.ctx, SlStr::from_str(key), &mut out) } == 0 {
      Some(out)
    } else {
      None
    }
  }

  pub fn config_bool(&self, key: &str) -> Option<bool> {
    if self.api.is_null() {
      return None;
    }
    let get = unsafe { (*self.api).config_get_bool }?;
    let mut out = false;
    if unsafe { get(self.ctx, SlStr::from_str(key), &mut out) } == 0 {
      Some(out)
    } else {
      None
    }
  }

  pub fn style(&self, name: &str) -> StyleSheet {
    if self.api.is_null() {
      return StyleSheet::default();
    }
    let get = unsafe { (*self.api).style_get }.unwrap_or(host_style_get_unavailable);
    let mut out = SlStyleSheet::default();
    if unsafe { get(self.ctx, SlStr::from_str(name), &mut out) } == 0 {
      StyleSheet::from_sl(&out)
    } else {
      StyleSheet::default()
    }
  }

  pub fn theme(&self) -> Theme {
    if self.api.is_null() {
      return Theme::default();
    }
    let get = unsafe { (*self.api).theme_get }.unwrap_or(host_theme_get_unavailable);
    let mut out = SlTheme::default();
    if unsafe { get(self.ctx, &mut out) } == 0 {
      Theme::from_sl(out)
    } else {
      Theme::default()
    }
  }

  pub fn compositor_state(&self) -> Option<CompositorState> {
    if self.api.is_null() {
      return None;
    }
    let get = unsafe { (*self.api).compositor_state_get }?;
    let mut out = SlCompositorState::default();
    if unsafe { get(self.ctx, &mut out) } != 0 {
      return None;
    }
    Some(unsafe { CompositorState::from_sl(&out) })
  }

  pub fn dispatch(&self, command: &str) -> bool {
    if self.api.is_null() {
      return false;
    }
    let Some(dispatch) = (unsafe { (*self.api).dispatch }) else {
      return false;
    };
    (unsafe { dispatch(self.ctx, SlStr::from_str(command)) }) == 0
  }

  pub fn persistence_get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
    let raw = self.persistence_raw(key)?;

    match ron::from_str::<T>(&raw) {
      Ok(value) => Some(value),
      Err(e) => {
        self.log(
          sl_log_level::WARN,
          &format!("[persistence] failed to decode {key:?}: {e}"),
        );
        None
      }
    }
  }

  pub fn persistence_open_or_get<T>(&self, key: &str, default: T) -> T
  where
    T: serde::Serialize + serde::de::DeserializeOwned,
  {
    match self.persistence_get(key) {
      Some(value) => value,
      None => {
        self.persistence_set(key, &default);
        default
      }
    }
  }

  pub fn persistence_set<T: serde::Serialize>(&self, key: &str, value: &T) -> bool {
    if self.api.is_null() {
      return false;
    }
    let Some(set) = (unsafe { (*self.api).persistence_set }) else {
      return false;
    };
    let Ok(raw) = ron::ser::to_string(value) else {
      return false;
    };
    (unsafe { set(self.ctx, SlStr::from_str(key), SlStr::from_str(&raw)) }) == 0
  }

  pub fn persistence_remove(&self, key: &str) -> bool {
    if self.api.is_null() {
      return false;
    }
    let Some(remove) = (unsafe { (*self.api).persistence_remove }) else {
      return false;
    };
    (unsafe { remove(self.ctx, SlStr::from_str(key)) }) == 0
  }

  pub fn persistence_has(&self, key: &str) -> bool {
    self.persistence_raw(key).is_some()
  }

  pub fn persistence_get_raw(&self, key: &str) -> Option<String> {
    self.persistence_raw(key)
  }

  pub fn persistence_set_raw(&self, key: &str, value: &str) -> bool {
    if self.api.is_null() {
      return false;
    }
    let Some(set) = (unsafe { (*self.api).persistence_set }) else {
      return false;
    };
    (unsafe { set(self.ctx, SlStr::from_str(key), SlStr::from_str(value)) }) == 0
  }

  fn persistence_raw(&self, key: &str) -> Option<String> {
    if self.api.is_null() {
      return None;
    }
    let get = unsafe { (*self.api).persistence_get }?;
    let mut out = SlStr::EMPTY;
    if unsafe { get(self.ctx, SlStr::from_str(key), &mut out) } != 0 {
      return None;
    }
    unsafe { out.as_str() }.map(str::to_owned)
  }

  pub fn audio_state(&self) -> Option<AudioState> {
    if self.api.is_null() {
      return None;
    }
    let get = unsafe { (*self.api).audio_state_get }?;
    let mut out = SlAudioState::default();
    if unsafe { get(self.ctx, &mut out) } != 0 {
      return None;
    }
    Some(unsafe { AudioState::from_sl(&out) })
  }

  pub fn bluetooth_state(&self) -> Option<BluetoothState> {
    if self.api.is_null() {
      return None;
    }
    let get = unsafe { (*self.api).bluetooth_state_get }?;
    let mut out = SlBluetoothState::default();
    if unsafe { get(self.ctx, &mut out) } != 0 {
      return None;
    }
    Some(unsafe { BluetoothState::from_sl(&out) })
  }

  pub fn network_state(&self) -> Option<NetworkState> {
    if self.api.is_null() {
      return None;
    }
    let get = unsafe { (*self.api).network_state_get }?;
    let mut out = SlNetworkState::default();
    if unsafe { get(self.ctx, &mut out) } != 0 {
      return None;
    }
    Some(unsafe { NetworkState::from_sl(&out) })
  }

  pub fn system_state(&self) -> Option<SystemState> {
    if self.api.is_null() {
      return None;
    }
    let get = unsafe { (*self.api).system_state_get }?;
    let mut out = SlSystemState::default();
    if unsafe { get(self.ctx, &mut out) } != 0 {
      return None;
    }
    Some(unsafe { SystemState::from_sl(&out) })
  }

  pub fn tray_state(&self) -> Option<TrayState> {
    if self.api.is_null() {
      return None;
    }
    let get = unsafe { (*self.api).tray_state_get }?;
    let mut out = SlTrayState::default();
    if unsafe { get(self.ctx, &mut out) } != 0 {
      return None;
    }
    Some(unsafe { TrayState::from_sl(&out) })
  }

  pub fn power_state(&self) -> Option<PowerState> {
    if self.api.is_null() {
      return None;
    }
    let get = unsafe { (*self.api).power_state_get }?;
    let mut out = SlPowerState::default();
    if unsafe { get(self.ctx, &mut out) } != 0 {
      return None;
    }
    Some(unsafe { PowerState::from_sl(&out) })
  }

  pub fn set_compositor_state(&self, state: &CompositorState) {
    if self.api.is_null() {
      return;
    }
    let Some(set) = (unsafe { (*self.api).set_compositor_state }) else {
      return;
    };
    let abi = compositor_state_abi(state);
    unsafe { set(self.ctx, &abi.state) };
    let _ = &abi;
  }

  pub fn notify(&self, notification: &Notification) -> Option<u32> {
    if self.api.is_null() {
      return None;
    }
    let notify = unsafe { (*self.api).notify }?;
    let abi = notification_abi(notification);
    let id = unsafe { notify(self.ctx, &abi.notification) };
    let _ = &abi;
    if id < 0 { None } else { Some(id as u32) }
  }

  pub fn config<T: Send + Sync + Clone + 'static>(&self, name: &str) -> Option<T> {
    self.with_config(name, |value: &T| value.clone())
  }

  pub fn with_config<T: Send + Sync + 'static, R>(
    &self,
    name: &str,
    f: impl FnOnce(&T) -> R,
  ) -> Option<R> {
    let values = config_values().lock().unwrap();
    values
      .get(name)
      .and_then(|value| value.downcast_ref::<T>())
      .map(f)
  }

  pub fn store(&self) -> &'static Store {
    store()
  }

  pub fn register_registry<T: Send + Sync + 'static>(&self, key: usize, value: T) -> bool {
    if self.api.is_null() {
      return false;
    }
    let Some(register) = (unsafe { (*self.api).registry_register }) else {
      return false;
    };
    let pointer = Box::into_raw(Box::new(value)) as *mut c_void;
    if unsafe { register(self.ctx, key, pointer) } == 0 {
      true
    } else {
      unsafe { drop(Box::from_raw(pointer as *mut T)) };
      false
    }
  }

  pub fn registry_ptr(&self, key: usize) -> *mut c_void {
    if self.api.is_null() {
      return core::ptr::null_mut();
    }
    match unsafe { (*self.api).registry_get } {
      Some(get) => unsafe { get(self.ctx, key) },
      None => core::ptr::null_mut(),
    }
  }

  pub fn with_registry<T: Send + Sync + 'static, R>(
    &self,
    key: usize,
    f: impl FnOnce(&mut T) -> R,
  ) -> Option<R> {
    let pointer = self.registry_ptr(key);
    if pointer.is_null() {
      return None;
    }
    Some(f(unsafe { &mut *(pointer as *mut T) }))
  }

  pub fn remove_registry<T: Send + Sync + 'static>(&self, key: usize) -> Option<T> {
    if self.api.is_null() {
      return None;
    }
    let remove = unsafe { (*self.api).registry_remove }?;
    let pointer = unsafe { remove(self.ctx, key) };
    if pointer.is_null() {
      None
    } else {
      Some(unsafe { *Box::from_raw(pointer as *mut T) })
    }
  }

  pub fn subscribe_registry<F>(&self, key: usize, f: F)
  where
    F: Fn(&Context, *mut c_void) + Send + Sync + 'static,
  {
    registry_subscribers()
      .lock()
      .unwrap()
      .insert(key, Box::new(f));

    if self.api.is_null() {
      return;
    }
    if let Some(subscribe) = unsafe { (*self.api).registry_subscribe } {
      unsafe { subscribe(self.ctx, key, sdk_registry_notify) };
    }
  }
}

type RegistrySubscriber = Box<dyn Fn(&Context, *mut c_void) + Send + Sync>;

fn registry_subscribers() -> &'static Mutex<HashMap<usize, RegistrySubscriber>> {
  static SUBSCRIBERS: OnceLock<Mutex<HashMap<usize, RegistrySubscriber>>> = OnceLock::new();
  SUBSCRIBERS.get_or_init(|| Mutex::new(HashMap::new()))
}

unsafe extern "C" fn sdk_registry_notify(ctx: *mut c_void, key: usize, value: *mut c_void) {
  let subscribers = registry_subscribers().lock().unwrap();
  if let Some(subscriber) = subscribers.get(&key) {
    let context = Context::new(api(), ctx);
    subscriber(&context, value);
  }
}

pub struct Options<'a> {
  ctx: &'a Context,
}

impl Options<'_> {
  pub fn str(&self, key: &str) -> Option<String> {
    if self.ctx.api.is_null() || self.ctx.opts.is_null() {
      return None;
    }
    let get = unsafe { (*self.ctx.api).get_str }?;
    let mut out = SlStr::EMPTY;
    if unsafe { get(self.ctx.opts, SlStr::from_str(key), &mut out) } == 0 {
      unsafe { out.as_str() }.map(str::to_owned)
    } else {
      None
    }
  }

  pub fn f64(&self, key: &str) -> Option<f64> {
    if self.ctx.api.is_null() || self.ctx.opts.is_null() {
      return None;
    }
    let get = unsafe { (*self.ctx.api).get_f64 }?;
    let mut out = 0.0;
    if unsafe { get(self.ctx.opts, SlStr::from_str(key), &mut out) } == 0 {
      Some(out)
    } else {
      None
    }
  }

  pub fn i64(&self, key: &str) -> Option<i64> {
    if self.ctx.api.is_null() || self.ctx.opts.is_null() {
      return None;
    }
    let get = unsafe { (*self.ctx.api).get_i64 }?;
    let mut out = 0;
    if unsafe { get(self.ctx.opts, SlStr::from_str(key), &mut out) } == 0 {
      Some(out)
    } else {
      None
    }
  }

  pub fn bool(&self, key: &str) -> Option<bool> {
    if self.ctx.api.is_null() || self.ctx.opts.is_null() {
      return None;
    }
    let get = unsafe { (*self.ctx.api).get_bool }?;
    let mut out = false;
    if unsafe { get(self.ctx.opts, SlStr::from_str(key), &mut out) } == 0 {
      Some(out)
    } else {
      None
    }
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Monitor {
  pub name: String,
  pub width: u32,
  pub height: u32,
  pub scale: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Workspace {
  pub id: u64,
  pub idx: u8,
  pub name: Option<String>,
  pub output: Option<String>,
  pub is_active: bool,
  pub is_focused: bool,
  pub is_urgent: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Window {
  pub id: u64,
  pub title: String,
  pub class: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompositorState {
  pub monitors: Vec<Monitor>,
  pub workspaces: Vec<Workspace>,
  pub active_window: Option<Window>,
  pub overview_active: bool,
}

impl CompositorState {
  unsafe fn from_sl(state: &SlCompositorState) -> Self {
    let monitors = if state.monitors.is_null() {
      Vec::new()
    } else {
      (0..state.monitor_count)
        .map(|index| {
          let monitor = unsafe { &*state.monitors.add(index) };
          Monitor {
            name: unsafe { monitor.name.as_str() }
              .unwrap_or_default()
              .to_owned(),
            width: monitor.width,
            height: monitor.height,
            scale: monitor.scale,
          }
        })
        .collect()
    };

    let workspaces = if state.workspaces.is_null() {
      Vec::new()
    } else {
      (0..state.workspace_count)
        .map(|index| {
          let workspace = unsafe { &*state.workspaces.add(index) };
          Workspace {
            id: workspace.id,
            idx: workspace.idx,
            name: unsafe { workspace.name.as_str() }.map(str::to_owned),
            output: unsafe { workspace.output.as_str() }.map(str::to_owned),
            is_active: workspace.is_active,
            is_focused: workspace.is_focused,
            is_urgent: workspace.is_urgent,
          }
        })
        .collect()
    };

    let active_window = if state.active_window.is_null() {
      None
    } else {
      let window = unsafe { &*state.active_window };
      Some(Window {
        id: window.id,
        title: unsafe { window.title.as_str() }
          .unwrap_or_default()
          .to_owned(),
        class: unsafe { window.wclass.as_str() }
          .unwrap_or_default()
          .to_owned(),
      })
    };

    CompositorState {
      monitors,
      workspaces,
      active_window,
      overview_active: state.overview_active,
    }
  }
}

struct SlCompositorStateAbi {
  #[allow(dead_code)]
  strings: Vec<Box<[u8]>>,
  #[allow(dead_code)]
  monitors: Vec<SlMonitor>,
  #[allow(dead_code)]
  workspaces: Vec<SlWorkspace>,
  #[allow(dead_code)]
  window: Option<SlWindow>,
  state: SlCompositorState,
}

fn compositor_state_abi(state: &CompositorState) -> SlCompositorStateAbi {
  fn intern(strings: &mut Vec<Box<[u8]>>, value: Option<&str>) -> SlStr {
    match value {
      Some(value) => {
        let bytes = value.as_bytes().to_vec().into_boxed_slice();
        let string = SlStr {
          ptr: bytes.as_ptr(),
          len: bytes.len(),
        };
        strings.push(bytes);
        string
      }
      None => SlStr::EMPTY,
    }
  }

  let mut strings = Vec::new();

  let mut monitors = Vec::with_capacity(state.monitors.len());
  for monitor in &state.monitors {
    let name = intern(&mut strings, Some(&monitor.name));
    monitors.push(SlMonitor {
      name,
      width: monitor.width,
      height: monitor.height,
      scale: monitor.scale,
    });
  }

  let mut workspaces = Vec::with_capacity(state.workspaces.len());
  for workspace in &state.workspaces {
    let output = intern(&mut strings, workspace.output.as_deref());
    let name = intern(&mut strings, workspace.name.as_deref());
    workspaces.push(SlWorkspace {
      id: workspace.id,
      idx: workspace.idx,
      output,
      name,
      is_active: workspace.is_active,
      is_focused: workspace.is_focused,
      is_urgent: workspace.is_urgent,
    });
  }

  let window = state.active_window.as_ref().map(|window| SlWindow {
    id: window.id,
    title: intern(&mut strings, Some(&window.title)),
    wclass: intern(&mut strings, Some(&window.class)),
  });

  let mut abi = SlCompositorStateAbi {
    strings,
    monitors,
    workspaces,
    window,
    state: SlCompositorState::default(),
  };
  abi.state = SlCompositorState {
    monitors: abi.monitors.as_ptr(),
    monitor_count: abi.monitors.len(),
    workspaces: abi.workspaces.as_ptr(),
    workspace_count: abi.workspaces.len(),
    active_window: abi
      .window
      .as_ref()
      .map_or(core::ptr::null(), |window| window as *const SlWindow),
    overview_active: state.overview_active,
  };
  abi
}

unsafe fn sl_str(value: &SlStr) -> String {
  unsafe { value.as_str() }.unwrap_or_default().to_owned()
}

fn sl_opt_str_ref(value: &SlStr) -> Option<String> {
  unsafe { value.as_str() }.map(str::to_owned)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioSink {
  pub id: u32,
  pub name: String,
  pub description: String,
  pub volume: f32,
  pub muted: bool,
  pub is_default: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MprisPlayer {
  pub identity: String,
  pub title: String,
  pub artist: String,
  pub album: String,
  pub art_url: Option<String>,
  pub playback_status: String,
  pub can_play_pause: bool,
  pub can_go_next: bool,
  pub can_go_previous: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioState {
  pub volume: f32,
  pub muted: bool,
  pub default_sink: Option<String>,
  pub sinks: Vec<AudioSink>,
  pub player: Option<MprisPlayer>,
}

impl AudioState {
  unsafe fn from_sl(state: &SlAudioState) -> Self {
    let sinks = if state.sinks.is_null() {
      Vec::new()
    } else {
      (0..state.sink_count)
        .map(|index| {
          let sink = unsafe { &*state.sinks.add(index) };
          AudioSink {
            id: sink.id,
            name: unsafe { sl_str(&sink.name) },
            description: unsafe { sl_str(&sink.description) },
            volume: sink.volume,
            muted: sink.muted,
            is_default: sink.is_default,
          }
        })
        .collect()
    };

    let player = state.has_player.then(|| MprisPlayer {
      identity: unsafe { sl_str(&state.player.identity) },
      title: unsafe { sl_str(&state.player.title) },
      artist: unsafe { sl_str(&state.player.artist) },
      album: unsafe { sl_str(&state.player.album) },
      art_url: sl_opt_str_ref(&state.player.art_url),
      playback_status: unsafe { sl_str(&state.player.playback_status) },
      can_play_pause: state.player.can_play_pause,
      can_go_next: state.player.can_go_next,
      can_go_previous: state.player.can_go_previous,
    });

    AudioState {
      volume: state.volume,
      muted: state.muted,
      default_sink: sl_opt_str_ref(&state.default_sink),
      sinks,
      player,
    }
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BluetoothDevice {
  pub address: String,
  pub name: String,
  pub icon: Option<String>,
  pub paired: bool,
  pub connected: bool,
  pub trusted: bool,
  pub battery: Option<u8>,
  pub rssi: Option<i16>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BluetoothState {
  pub powered: bool,
  pub discovering: bool,
  pub adapter_name: Option<String>,
  pub devices: Vec<BluetoothDevice>,
}

impl BluetoothState {
  unsafe fn from_sl(state: &SlBluetoothState) -> Self {
    let devices = if state.devices.is_null() {
      Vec::new()
    } else {
      (0..state.device_count)
        .map(|index| {
          let device = unsafe { &*state.devices.add(index) };
          BluetoothDevice {
            address: unsafe { sl_str(&device.address) },
            name: unsafe { sl_str(&device.name) },
            icon: sl_opt_str_ref(&device.icon),
            paired: device.paired,
            connected: device.connected,
            trusted: device.trusted,
            battery: (device.battery >= 0).then_some(device.battery as u8),
            rssi: (device.rssi != i32::MIN).then_some(device.rssi as i16),
          }
        })
        .collect()
    };

    BluetoothState {
      powered: state.powered,
      discovering: state.discovering,
      adapter_name: sl_opt_str_ref(&state.adapter_name),
      devices,
    }
  }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccessPoint {
  pub ssid: String,
  pub signal: u8,
  pub secured: bool,
  pub in_use: bool,
  pub saved: bool,
}

impl AccessPoint {
  unsafe fn from_sl(ap: &SlAccessPoint) -> Self {
    AccessPoint {
      ssid: unsafe { sl_str(&ap.ssid) },
      signal: ap.signal,
      secured: ap.secured,
      in_use: ap.in_use,
      saved: ap.saved,
    }
  }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ethernet {
  pub iface: String,
  pub speed: u32,
  pub carrier: bool,
  pub connected: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConnectionInfo {
  pub label: String,
  pub mac: String,
  pub is_wifi: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetworkState {
  pub wifi_enabled: bool,
  pub wifi_hardware_enabled: bool,
  pub scanning: bool,
  pub busy: bool,
  pub wifi_connected: Option<AccessPoint>,
  pub ethernet: Option<Ethernet>,
  pub networks: Vec<AccessPoint>,
  pub connected: Option<ConnectionInfo>,
}

impl NetworkState {
  unsafe fn from_sl(state: &SlNetworkState) -> Self {
    let networks = if state.networks.is_null() {
      Vec::new()
    } else {
      (0..state.network_count)
        .map(|index| unsafe { AccessPoint::from_sl(&*state.networks.add(index)) })
        .collect()
    };

    NetworkState {
      wifi_enabled: state.wifi_enabled,
      wifi_hardware_enabled: state.wifi_hardware_enabled,
      scanning: state.scanning,
      busy: state.busy,
      wifi_connected: state
        .has_wifi_connected
        .then(|| unsafe { AccessPoint::from_sl(&state.wifi_connected) }),
      ethernet: state.has_ethernet.then(|| Ethernet {
        iface: unsafe { sl_str(&state.ethernet.iface) },
        speed: state.ethernet.speed,
        carrier: state.ethernet.carrier,
        connected: state.ethernet.connected,
      }),
      networks,
      connected: state.has_connected.then(|| ConnectionInfo {
        label: unsafe { sl_str(&state.connected.label) },
        mac: unsafe { sl_str(&state.connected.mac) },
        is_wifi: state.connected.is_wifi,
      }),
    }
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProcessInfo {
  pub pid: u32,
  pub name: String,
  pub cpu_usage: f32,
  pub memory: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SystemState {
  pub cpu_usage: f32,
  pub mem_usage: f32,
  pub mem_total: u64,
  pub mem_used: u64,
  pub temperature: Option<f32>,
  pub load: [f32; 3],
  pub network_state: u16,
  pub network_rx: u64,
  pub network_tx: u64,
  pub processes: Vec<ProcessInfo>,
}

impl SystemState {
  unsafe fn from_sl(state: &SlSystemState) -> Self {
    let processes = if state.top_processes.is_null() {
      Vec::new()
    } else {
      (0..state.process_count)
        .map(|index| {
          let process = unsafe { &*state.top_processes.add(index) };
          ProcessInfo {
            pid: process.pid,
            name: unsafe { sl_str(&process.name) },
            cpu_usage: process.cpu_usage,
            memory: process.memory,
          }
        })
        .collect()
    };

    SystemState {
      cpu_usage: state.cpu_usage,
      mem_usage: state.mem_usage,
      mem_total: state.mem_total,
      mem_used: state.mem_used,
      temperature: state.has_temperature.then_some(state.temperature),
      load: state.load,
      network_state: state.network_state,
      network_rx: state.network_rx,
      network_tx: state.network_tx,
      processes,
    }
  }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayItem {
  pub address: String,
  pub title: String,
  pub icon_name: Option<String>,
  pub menu_path: Option<String>,
  pub pixmap: Option<(u32, u32, Vec<u8>)>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayState {
  pub items: Vec<TrayItem>,
}

impl TrayState {
  unsafe fn from_sl(state: &SlTrayState) -> Self {
    let items = if state.items.is_null() {
      Vec::new()
    } else {
      (0..state.item_count)
        .map(|index| {
          let item = unsafe { &*state.items.add(index) };
          let pixmap = if item.has_pixmap {
            unsafe { item.pixmap.as_bytes() }
              .map(|bytes| (item.pixmap_width, item.pixmap_height, bytes.to_vec()))
          } else {
            None
          };
          TrayItem {
            address: unsafe { sl_str(&item.address) },
            title: unsafe { sl_str(&item.title) },
            icon_name: sl_opt_str_ref(&item.icon_name),
            menu_path: sl_opt_str_ref(&item.menu_path),
            pixmap,
          }
        })
        .collect()
    };

    TrayState { items }
  }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PowerState {
  pub percent: Option<u8>,
  pub charging: bool,
  pub status: String,
  pub health: Option<u8>,
  pub energy_now_wh: Option<f32>,
  pub energy_full_wh: Option<f32>,
  pub power_w: Option<f32>,
  pub time_remaining: Option<String>,
  pub active_profile: Option<String>,
  pub available_profiles: Vec<String>,
  pub brightness_percent: u8,
  pub brightness_max: u32,
  pub brightness_current: u32,
  pub device_name: String,
}

impl PowerState {
  unsafe fn from_sl(state: &SlPowerState) -> Self {
    let available_profiles = if state.available_profiles.is_null() {
      Vec::new()
    } else {
      (0..state.available_profile_count)
        .map(|index| unsafe { sl_str(&*state.available_profiles.add(index)) })
        .collect()
    };

    PowerState {
      percent: state.has_percent.then_some(state.percent),
      charging: state.charging,
      status: unsafe { sl_str(&state.status) },
      health: state.has_health.then_some(state.health),
      energy_now_wh: state.has_energy_now.then_some(state.energy_now_wh),
      energy_full_wh: state.has_energy_full.then_some(state.energy_full_wh),
      power_w: state.has_power.then_some(state.power_w),
      time_remaining: sl_opt_str_ref(&state.time_remaining),
      active_profile: sl_opt_str_ref(&state.active_profile),
      available_profiles,
      brightness_percent: state.brightness_percent,
      brightness_max: state.brightness_max,
      brightness_current: state.brightness_current,
      device_name: unsafe { sl_str(&state.device_name) },
    }
  }
}

#[derive(Clone, Debug)]
pub struct NotificationAction {
  pub label: String,
  pub command: String,
}

#[derive(Clone, Debug)]
pub struct Notification {
  pub summary: String,
  pub body: String,
  pub app_name: Option<String>,
  pub app_icon: Option<String>,
  pub urgency: u8,
  pub timeout_ms: i32,
  pub actions: Vec<NotificationAction>,
}

impl Default for Notification {
  fn default() -> Self {
    Self {
      summary: String::new(),
      body: String::new(),
      app_name: None,
      app_icon: None,
      urgency: 1,
      timeout_ms: -1,
      actions: Vec::new(),
    }
  }
}

impl Notification {
  pub fn new(summary: impl Into<String>, body: impl Into<String>) -> Self {
    Self {
      summary: summary.into(),
      body: body.into(),
      ..Default::default()
    }
  }

  pub fn app_name(mut self, app_name: impl Into<String>) -> Self {
    self.app_name = Some(app_name.into());
    self
  }

  pub fn icon(mut self, icon: impl Into<String>) -> Self {
    self.app_icon = Some(icon.into());
    self
  }

  pub fn urgency(mut self, urgency: u8) -> Self {
    self.urgency = urgency;
    self
  }

  pub fn timeout(mut self, millis: i32) -> Self {
    self.timeout_ms = millis;
    self
  }

  pub fn action(mut self, label: impl Into<String>, command: impl Into<String>) -> Self {
    self.actions.push(NotificationAction {
      label: label.into(),
      command: command.into(),
    });
    self
  }
}

struct SlNotificationAbi {
  #[allow(dead_code)]
  strings: Vec<Box<[u8]>>,
  #[allow(dead_code)]
  actions: Vec<SlNotificationAction>,
  notification: SlNotification,
}

fn notification_abi(notification: &Notification) -> SlNotificationAbi {
  let mut strings: Vec<Box<[u8]>> = Vec::new();

  fn intern(strings: &mut Vec<Box<[u8]>>, value: Option<&str>) -> SlStr {
    match value {
      Some(value) => {
        let bytes = value.as_bytes().to_vec().into_boxed_slice();
        let string = SlStr {
          ptr: bytes.as_ptr(),
          len: bytes.len(),
        };
        strings.push(bytes);
        string
      }
      None => SlStr::EMPTY,
    }
  }

  let summary = intern(&mut strings, Some(&notification.summary));
  let body = intern(&mut strings, Some(&notification.body));
  let app_name = intern(&mut strings, notification.app_name.as_deref());
  let app_icon = intern(&mut strings, notification.app_icon.as_deref());

  let mut actions = Vec::with_capacity(notification.actions.len());
  for action in &notification.actions {
    let command = intern(&mut strings, Some(&action.command));
    let label = intern(&mut strings, Some(&action.label));
    actions.push(SlNotificationAction { command, label });
  }

  let notification = SlNotification {
    summary,
    body,
    app_name,
    app_icon,
    urgency: notification.urgency,
    timeout_ms: notification.timeout_ms,
    replaces_id: 0,
    actions: if actions.is_empty() {
      core::ptr::null()
    } else {
      actions.as_ptr()
    },
    action_count: actions.len(),
  };

  SlNotificationAbi {
    strings,
    actions,
    notification,
  }
}

type ConfigValue = Box<dyn Any + Send + Sync>;
type ConfigParseFn = Box<dyn Fn(Option<&kdl::KdlNode>) -> Result<(), String> + Send + Sync>;

fn config_parsers() -> &'static Mutex<HashMap<String, ConfigParseFn>> {
  static PARSERS: OnceLock<Mutex<HashMap<String, ConfigParseFn>>> = OnceLock::new();
  PARSERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn config_values() -> &'static Mutex<HashMap<String, ConfigValue>> {
  static VALUES: OnceLock<Mutex<HashMap<String, ConfigValue>>> = OnceLock::new();
  VALUES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn store_config_value<T: Send + Sync + 'static>(key: &str, value: T) {
  config_values()
    .lock()
    .unwrap()
    .insert(key.to_owned(), Box::new(value));
}

unsafe extern "C" fn sdk_config_parser_callback(
  ctx: *mut c_void,
  name: SlStr,
  block: SlStr,
) -> i32 {
  let Some(name) = (unsafe { name.as_str() }) else {
    return 0;
  };

  let node = unsafe { block.as_str() }
    .and_then(|text| text.parse::<kdl::KdlDocument>().ok())
    .and_then(|document| document.nodes().first().cloned());

  let result = {
    let parsers = config_parsers().lock().unwrap();
    match parsers.get(name) {
      Some(parse) => parse(node.as_ref()),
      None => Ok(()),
    }
  };

  match result {
    Ok(()) => 0,
    Err(message) => {
      let api = api();
      if !api.is_null()
        && let Some(report) = unsafe { (*api).plugin_error }
      {
        unsafe { report(ctx, 1, SlStr::from_str(&message)) };
      }
      1
    }
  }
}

#[derive(Default)]
pub struct Store {
  entries: Mutex<HashMap<usize, Entry>>,
}

enum Entry {
  Typed(Box<dyn Any + Send + Sync>),
  Raw(*mut c_void),
}

unsafe impl Send for Entry {}
unsafe impl Sync for Entry {}

pub fn store() -> &'static Store {
  static STORE: OnceLock<Store> = OnceLock::new();
  STORE.get_or_init(Store::default)
}

impl Store {
  pub fn insert<T: Send + Sync + 'static>(&self, key: usize, value: T) -> Option<T> {
    let previous = self
      .entries
      .lock()
      .unwrap()
      .insert(key, Entry::Typed(Box::new(value)));

    match previous {
      Some(Entry::Typed(boxed)) => boxed.downcast::<T>().ok().map(|boxed| *boxed),
      _ => None,
    }
  }

  pub fn with<T: Send + Sync + 'static, R>(
    &self,
    key: usize,
    f: impl FnOnce(&mut T) -> R,
  ) -> Option<R> {
    let mut entries = self.entries.lock().unwrap();
    match entries.get_mut(&key) {
      Some(Entry::Typed(boxed)) => boxed.downcast_mut::<T>().map(f),
      _ => None,
    }
  }

  pub fn remove<T: Send + Sync + 'static>(&self, key: usize) -> Option<T> {
    let mut entries = self.entries.lock().unwrap();
    if !matches!(entries.get(&key), Some(Entry::Typed(boxed)) if boxed.is::<T>()) {
      return None;
    }
    match entries.remove(&key) {
      Some(Entry::Typed(boxed)) => boxed.downcast::<T>().ok().map(|boxed| *boxed),
      _ => None,
    }
  }

  pub fn contains(&self, key: usize) -> bool {
    self.entries.lock().unwrap().contains_key(&key)
  }

  pub fn set_raw(&self, key: usize, value: *mut c_void) {
    self.entries.lock().unwrap().insert(key, Entry::Raw(value));
  }

  pub fn raw(&self, key: usize) -> Option<*mut c_void> {
    match self.entries.lock().unwrap().get(&key) {
      Some(Entry::Raw(value)) => Some(*value),
      _ => None,
    }
  }

  pub fn remove_raw(&self, key: usize) -> Option<*mut c_void> {
    match self.entries.lock().unwrap().remove(&key) {
      Some(Entry::Raw(value)) => Some(value),
      _ => None,
    }
  }
}

unsafe extern "C" fn host_style_get_unavailable(
  _ctx: *mut c_void,
  _name: SlStr,
  _out: *mut SlStyleSheet,
) -> i32 {
  -1
}

unsafe extern "C" fn host_theme_get_unavailable(_ctx: *mut c_void, _out: *mut SlTheme) -> i32 {
  -1
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
  Tick { tag: u32, fd: i32 },
  Fd { action: u32, fd: i32 },
  ConfigReload,
  CompositorUpdate,
  Frame,
  Other(u32),
}

impl Event {
  fn from_sl(event: &SlEvent) -> Event {
    match event.kind {
      sl_event_kind::TICK => Event::Tick {
        tag: event.action,
        fd: event.fd,
      },
      sl_event_kind::FD => Event::Fd {
        action: event.action,
        fd: event.fd,
      },
      sl_event_kind::CONFIG_RELOAD => Event::ConfigReload,
      sl_event_kind::COMPOSITOR_UPDATE => Event::CompositorUpdate,
      sl_event_kind::FRAME => Event::Frame,
      other => Event::Other(other),
    }
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EventMask(pub u32);

impl EventMask {
  pub const NONE: Self = Self(0);
  pub const TICK: Self = Self(sl_event_mask::TICK);
  pub const FD: Self = Self(sl_event_mask::FD);
  pub const CONFIG_RELOAD: Self = Self(sl_event_mask::CONFIG_RELOAD);
  pub const COMPOSITOR_UPDATE: Self = Self(sl_event_mask::COMPOSITOR_UPDATE);
  pub const FRAME: Self = Self(sl_event_mask::FRAME);

  pub const fn union(self, other: Self) -> Self {
    Self(self.0 | other.0)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemEffect {
  None,
  Redraw,
  Subscribe,
  Hide,
  Show,
  ReallyHide,
  Destroy,
  ReallyDestroy,
  Custom([usize; 4]),
}

impl ItemEffect {
  fn to_sl(self) -> SlEffect {
    let code = match self {
      ItemEffect::None => sl_effect::NONE,
      ItemEffect::Redraw => sl_effect::REDRAW,
      ItemEffect::Subscribe => sl_effect::SUBSCRIBE,
      ItemEffect::Hide => sl_effect::HIDE,
      ItemEffect::Show => sl_effect::SHOW,
      ItemEffect::ReallyHide => sl_effect::REALLY_HIDE,
      ItemEffect::Destroy => sl_effect::DESTROY,
      ItemEffect::ReallyDestroy => sl_effect::REALLY_DESTROY,
      ItemEffect::Custom(_) => sl_effect::CUSTOM,
    };
    let custom = match self {
      ItemEffect::Custom(value) => value,
      _ => [0; 4],
    };
    SlEffect { code, custom }
  }
}

fn effect_from_sl(code: u32, custom: [usize; 4]) -> ItemEffect {
  match code {
    sl_effect::REDRAW => ItemEffect::Redraw,
    sl_effect::SUBSCRIBE => ItemEffect::Subscribe,
    sl_effect::HIDE => ItemEffect::Hide,
    sl_effect::SHOW => ItemEffect::Show,
    sl_effect::REALLY_HIDE => ItemEffect::ReallyHide,
    sl_effect::DESTROY => ItemEffect::Destroy,
    sl_effect::REALLY_DESTROY => ItemEffect::ReallyDestroy,
    sl_effect::CUSTOM => ItemEffect::Custom(custom),
    _ => ItemEffect::None,
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
  Noop,
  Effect(ItemEffect),
  Action(String),
  EffectAction(ItemEffect, String),
}

impl Message {
  fn from_sl(message: &SlItemMessage) -> Message {
    let name = || {
      unsafe { message.name.as_str() }
        .unwrap_or_default()
        .to_owned()
    };
    match message.kind {
      sl_message_kind::EFFECT => Message::Effect(effect_from_sl(message.effect, message.custom)),
      sl_message_kind::ACTION => Message::Action(name()),
      sl_message_kind::EFFECT_ACTION => {
        Message::EffectAction(effect_from_sl(message.effect, message.custom), name())
      }
      _ => Message::Noop,
    }
  }
}

#[derive(Clone)]
pub enum Image {
  Rgba {
    cache_key: u64,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
  },
  Encoded {
    cache_key: u64,
    bytes: Vec<u8>,
  },
  Path(String),
}

impl Image {
  pub fn rgba(width: u32, height: u32, pixels: impl Into<Vec<u8>>) -> Image {
    Image::Rgba {
      cache_key: 0,
      width,
      height,
      pixels: pixels.into(),
    }
  }

  pub fn rgba_cached(cache_key: u64, width: u32, height: u32, pixels: impl Into<Vec<u8>>) -> Image {
    Image::Rgba {
      cache_key,
      width,
      height,
      pixels: pixels.into(),
    }
  }

  pub fn encoded(bytes: impl Into<Vec<u8>>) -> Image {
    Image::Encoded {
      cache_key: 0,
      bytes: bytes.into(),
    }
  }

  pub fn encoded_cached(cache_key: u64, bytes: impl Into<Vec<u8>>) -> Image {
    Image::Encoded {
      cache_key,
      bytes: bytes.into(),
    }
  }

  pub fn path(path: impl Into<String>) -> Image {
    Image::Path(path.into())
  }
}

pub enum Node {
  Row {
    children: Vec<Node>,
    spacing: f32,
    align: Align,
  },
  Column {
    children: Vec<Node>,
    spacing: f32,
    align: Align,
  },
  Container {
    child: Box<Node>,
    padding: Padding,
    align_x: Align,
    align_y: Align,
    style: Option<Style>,
    width: Length,
    height: Length,
  },
  Clickable {
    child: Box<Node>,
    action: Option<String>,
    effect: Option<[usize; 4]>,
  },
  Scrollable {
    child: Box<Node>,
    width: Length,
    height: Length,
  },
  Icon {
    name: String,
    size: f32,
    color: Option<Color>,
  },
  Text {
    content: String,
    size: f32,
    color: Option<Color>,
  },
  Progress {
    value: f32,
    color: Option<Color>,
  },
  Image {
    image: Image,
    width: Length,
    height: Length,
  },
  Canvas {
    cache_key: u64,
    pixel_width: u32,
    pixel_height: u32,
    width: Length,
    height: Length,
    draw: Box<dyn Fn(&mut [u8])>,
  },
}

impl Node {
  pub fn row(children: Vec<Node>) -> Node {
    Node::Row {
      children,
      spacing: 4.0,
      align: Align::Start,
    }
  }

  pub fn column(children: Vec<Node>) -> Node {
    Node::Column {
      children,
      spacing: 4.0,
      align: Align::Start,
    }
  }

  pub fn container(child: Node) -> Node {
    Node::Container {
      child: Box::new(child),
      padding: Padding::ZERO,
      align_x: Align::Start,
      align_y: Align::Start,
      style: None,
      width: Length::Shrink,
      height: Length::Shrink,
    }
  }

  pub fn scrollable(child: Node) -> Node {
    Node::Scrollable {
      child: Box::new(child),
      width: Length::Shrink,
      height: Length::Shrink,
    }
  }

  pub fn clickable(child: Node) -> Node {
    Node::Clickable {
      child: Box::new(child),
      action: None,
      effect: None,
    }
  }

  pub fn icon(name: impl Into<String>) -> Node {
    Node::Icon {
      name: name.into(),
      size: 16.0,
      color: None,
    }
  }

  pub fn text(content: impl Into<String>) -> Node {
    Node::Text {
      content: content.into(),
      size: 12.0,
      color: None,
    }
  }

  pub fn progress(value: f32) -> Node {
    Node::Progress { value, color: None }
  }

  pub fn image(image: Image) -> Node {
    Node::Image {
      image,
      width: Length::Shrink,
      height: Length::Shrink,
    }
  }

  pub fn canvas(width: u32, height: u32, draw: impl Fn(&mut [u8]) + 'static) -> Node {
    Node::Canvas {
      cache_key: 0,
      pixel_width: width,
      pixel_height: height,
      width: Length::Shrink,
      height: Length::Shrink,
      draw: Box::new(draw),
    }
  }

  pub fn canvas_cached(
    cache_key: u64,
    width: u32,
    height: u32,
    draw: impl Fn(&mut [u8]) + 'static,
  ) -> Node {
    Node::Canvas {
      cache_key,
      pixel_width: width,
      pixel_height: height,
      width: Length::Shrink,
      height: Length::Shrink,
      draw: Box::new(draw),
    }
  }

  pub fn cache_key(mut self, key: u64) -> Node {
    match &mut self {
      Node::Canvas { cache_key, .. } => *cache_key = key,
      Node::Image {
        image: Image::Rgba { cache_key, .. } | Image::Encoded { cache_key, .. },
        ..
      } => *cache_key = key,
      _ => {}
    }
    self
  }

  pub fn size(mut self, size: f32) -> Node {
    match &mut self {
      Node::Icon { size: target, .. } | Node::Text { size: target, .. } => *target = size,
      _ => {}
    }
    self
  }

  pub fn color(mut self, color: Color) -> Node {
    match &mut self {
      Node::Icon { color: target, .. }
      | Node::Text { color: target, .. }
      | Node::Progress { color: target, .. } => *target = Some(color),
      _ => {}
    }
    self
  }

  pub fn width(mut self, width: Length) -> Node {
    match &mut self {
      Node::Container { width: target, .. }
      | Node::Scrollable { width: target, .. }
      | Node::Image { width: target, .. }
      | Node::Canvas { width: target, .. } => *target = width,
      _ => {}
    }
    self
  }

  pub fn height(mut self, height: Length) -> Node {
    match &mut self {
      Node::Container { height: target, .. }
      | Node::Scrollable { height: target, .. }
      | Node::Image { height: target, .. }
      | Node::Canvas { height: target, .. } => *target = height,
      _ => {}
    }
    self
  }

  pub fn action(self, command: impl Into<String>) -> Node {
    match self {
      Node::Clickable {
        child,
        effect,
        action: _,
      } => Node::Clickable {
        child,
        action: Some(command.into()),
        effect,
      },
      other => Node::Clickable {
        child: Box::new(other),
        action: Some(command.into()),
        effect: None,
      },
    }
  }

  pub fn effect(self, value: [usize; 4]) -> Node {
    match self {
      Node::Clickable {
        child,
        action,
        effect: _,
      } => Node::Clickable {
        child,
        action,
        effect: Some(value),
      },
      other => Node::Clickable {
        child: Box::new(other),
        action: None,
        effect: Some(value),
      },
    }
  }
}

struct NodeArena {
  nodes: Vec<SlNode>,
  strings: Vec<String>,
  images: Vec<Vec<u8>>,
}

impl NodeArena {
  fn new() -> Self {
    Self {
      nodes: Vec::new(),
      strings: Vec::new(),
      images: Vec::new(),
    }
  }

  fn clear(&mut self) {
    self.nodes.clear();
    self.strings.clear();
    self.images.clear();
  }

  fn stash(&mut self, text: &str) -> SlStr {
    self.strings.push(text.to_owned());
    let stored = self.strings.last().expect("just pushed");
    SlStr {
      ptr: stored.as_ptr(),
      len: stored.len(),
    }
  }

  fn push(&mut self, node: SlNode) -> *const SlNode {
    let index = self.nodes.len();
    self.nodes.push(node);
    unsafe { self.nodes.as_ptr().add(index) }
  }

  fn total_slots(node: &Node) -> usize {
    match node {
      Node::Row { children, .. } | Node::Column { children, .. } => {
        1 + children.len() + children.iter().map(NodeArena::total_slots).sum::<usize>()
      }
      Node::Container { child, .. }
      | Node::Clickable { child, .. }
      | Node::Scrollable { child, .. } => 1 + NodeArena::total_slots(child),
      _ => 1,
    }
  }

  fn materialize(&mut self, node: &Node, ctx: &Context) -> *const SlNode {
    self.nodes.reserve(NodeArena::total_slots(node));
    match node {
      Node::Row {
        children,
        spacing,
        align,
      }
      | Node::Column {
        children,
        spacing,
        align,
      } => {
        let base = self.nodes.len();
        self.nodes.push(SlNode::default());

        let roots: Vec<*const SlNode> = children
          .iter()
          .map(|child| self.materialize(child, ctx))
          .collect();
        let block_start = self.nodes.len();
        for &root in &roots {
          self.nodes.push(unsafe { *root });
        }

        let kind = if matches!(node, Node::Row { .. }) {
          sl_node_kind::ROW
        } else {
          sl_node_kind::COLUMN
        };
        self.nodes[base] = SlNode {
          kind,
          spacing: *spacing,
          align_y: if matches!(node, Node::Row { .. }) {
            align.to_u32()
          } else {
            0
          },
          align_x: if matches!(node, Node::Column { .. }) {
            align.to_u32()
          } else {
            0
          },
          children: if children.is_empty() {
            core::ptr::null()
          } else {
            unsafe { self.nodes.as_ptr().add(block_start) }
          },
          child_count: children.len(),
          ..SlNode::default()
        };
        unsafe { self.nodes.as_ptr().add(base) }
      }

      Node::Container {
        child,
        padding,
        align_x,
        align_y,
        style,
        width,
        height,
      } => {
        let base = self.nodes.len();
        self.nodes.push(SlNode::default());
        self.materialize(child, ctx);

        self.nodes[base] = SlNode {
          kind: sl_node_kind::CONTAINER,
          style: style_to_sl(style),
          padding: padding.to_array(),
          align_x: align_x.to_u32(),
          align_y: align_y.to_u32(),
          children: unsafe { self.nodes.as_ptr().add(base + 1) },
          child_count: 1,
          width: width.to_sl(),
          height: height.to_sl(),
          ..SlNode::default()
        };
        unsafe { self.nodes.as_ptr().add(base) }
      }

      Node::Clickable {
        child,
        action,
        effect,
      } => {
        let base = self.nodes.len();
        self.nodes.push(SlNode::default());
        self.materialize(child, ctx);

        let action = match action {
          Some(action) => self.stash(action),
          None => SlStr::EMPTY,
        };
        let (has_effect, effect) = match effect {
          Some(effect) => (true, *effect),
          None => (false, [0; 4]),
        };

        self.nodes[base] = SlNode {
          kind: sl_node_kind::CONTAINER,
          action,
          children: unsafe { self.nodes.as_ptr().add(base + 1) },
          child_count: 1,
          has_effect,
          effect,
          ..SlNode::default()
        };
        unsafe { self.nodes.as_ptr().add(base) }
      }

      Node::Scrollable {
        child,
        width,
        height,
      } => {
        let base = self.nodes.len();
        self.nodes.push(SlNode::default());
        self.materialize(child, ctx);

        self.nodes[base] = SlNode {
          kind: sl_node_kind::SCROLLABLE,
          children: unsafe { self.nodes.as_ptr().add(base + 1) },
          child_count: 1,
          width: width.to_sl(),
          height: height.to_sl(),
          ..SlNode::default()
        };
        unsafe { self.nodes.as_ptr().add(base) }
      }

      Node::Icon { name, size, color } => {
        let text = self.stash(name);
        self.push(SlNode {
          kind: sl_node_kind::ICON,
          text,
          size: *size,
          style: color_style(color.as_ref()),
          ..SlNode::default()
        })
      }

      Node::Text {
        content,
        size,
        color,
      } => {
        let text = self.stash(content);
        self.push(SlNode {
          kind: sl_node_kind::TEXT,
          text,
          size: *size,
          style: color_style(color.as_ref()),
          ..SlNode::default()
        })
      }

      Node::Progress { value, color } => self.push(SlNode {
        kind: sl_node_kind::PROGRESS,
        value: *value,
        style: color_style(color.as_ref()),
        ..SlNode::default()
      }),

      Node::Image {
        image,
        width: layout_width,
        height: layout_height,
      } => {
        let (source, data, width, height, cache_key) = match image {
          Image::Rgba {
            cache_key,
            width: pixel_width,
            height: pixel_height,
            pixels,
          } => {
            self.images.push(pixels.clone());
            let pixels = self.images.last().expect("just pushed");
            (
              sl_image_source::RAW_RGBA,
              SlStr {
                ptr: pixels.as_ptr(),
                len: pixels.len(),
              },
              *pixel_width,
              *pixel_height,
              *cache_key,
            )
          }
          Image::Encoded { cache_key, bytes } => {
            self.images.push(bytes.clone());
            let bytes = self.images.last().expect("just pushed");
            (
              sl_image_source::ENCODED,
              SlStr {
                ptr: bytes.as_ptr(),
                len: bytes.len(),
              },
              0,
              0,
              *cache_key,
            )
          }
          Image::Path(path) => (sl_image_source::PATH, self.stash(path), 0, 0, 0),
        };
        self.push(SlNode {
          kind: sl_node_kind::IMAGE,
          image: SlImage {
            source,
            data,
            width,
            height,
            cache_key,
          },
          width: layout_width.to_sl(),
          height: layout_height.to_sl(),
          ..SlNode::default()
        })
      }

      Node::Canvas {
        cache_key,
        pixel_width,
        pixel_height,
        width,
        height,
        draw,
      } => {
        let mut node = SlNode {
          kind: sl_node_kind::CANVAS,
          canvas: SlCanvas {
            width: *pixel_width,
            height: *pixel_height,
            stride: 0,
            data: core::ptr::null_mut(),
            cache_key: *cache_key,
          },
          width: width.to_sl(),
          height: height.to_sl(),
          ..SlNode::default()
        };
        if let Some(buffer) = ctx.canvas(*pixel_width, *pixel_height) {
          (draw)(buffer);
          node.canvas.data = buffer.as_mut_ptr();
        }
        self.push(node)
      }
    }
  }
}

fn color_to_sl(color: &Color) -> SlColor {
  (*color).into()
}

fn color_style(color: Option<&Color>) -> SlStyle {
  match color {
    Some(color) => SlStyle {
      text_color: color_to_sl(color),
      has_text_color: true,
      ..SlStyle::default()
    },
    None => SlStyle::default(),
  }
}

fn style_to_sl(style: &Option<Style>) -> SlStyle {
  match style {
    Some(style) => SlStyle {
      background: style
        .background
        .map(|color| color_to_sl(&color))
        .unwrap_or_default(),
      has_background: style.background.is_some(),
      border: style.border.as_ref().map(Into::into).unwrap_or_default(),
      has_border: style.border.is_some(),
      text_color: style
        .text_color
        .map(|color| color_to_sl(&color))
        .unwrap_or_default(),
      has_text_color: style.text_color.is_some(),
    },
    None => SlStyle::default(),
  }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderableSettings {
  pub wrap_popup: bool,
  pub width: f32,
  pub height: f32,
}

impl RenderableSettings {
  pub const fn popup(width: f32, height: f32) -> Self {
    Self {
      wrap_popup: true,
      width,
      height,
    }
  }

  fn to_sl(&self) -> SlRenderableSettings {
    SlRenderableSettings {
      wrap_popup: self.wrap_popup,
      width: self.width,
      height: self.height,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
  Background,
  Bottom,
  Top,
  Overlay,
}

impl Layer {
  fn to_u32(&self) -> u32 {
    match self {
      Layer::Background => sl_layer::BACKGROUND,
      Layer::Bottom => sl_layer::BOTTOM,
      Layer::Top => sl_layer::TOP,
      Layer::Overlay => sl_layer::OVERLAY,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anchor(u32);

impl Anchor {
  pub const TOP: Self = Self(sl_anchor::TOP);
  pub const BOTTOM: Self = Self(sl_anchor::BOTTOM);
  pub const LEFT: Self = Self(sl_anchor::LEFT);
  pub const RIGHT: Self = Self(sl_anchor::RIGHT);
  pub const NONE: Self = Self(0);

  pub fn contains(self, other: Self) -> bool {
    self.0 & other.0 == other.0
  }

  pub fn union(self, other: Self) -> Self {
    Self(self.0 | other.0)
  }
}

impl core::ops::BitOr for Anchor {
  type Output = Anchor;

  fn bitor(self, rhs: Anchor) -> Anchor {
    Self(self.0 | rhs.0)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardInteractivity {
  None,
  OnDemand,
  Exclusive,
}

impl KeyboardInteractivity {
  fn to_u32(&self) -> u32 {
    match self {
      KeyboardInteractivity::None => sl_keyboard_interactivity::NONE,
      KeyboardInteractivity::OnDemand => sl_keyboard_interactivity::ON_DEMAND,
      KeyboardInteractivity::Exclusive => sl_keyboard_interactivity::EXCLUSIVE,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
  Visible,
  Transient,
  Toggleable(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateWhen {
  OnDemand,
  EveryFrame,
  OnEvent,
}

#[derive(Clone, Debug)]
pub struct DesktopSettings {
  pub layer: Layer,
  pub anchor: Anchor,
  pub width: u32,
  pub height: u32,
  pub margin: [i32; 4],
  pub exclusive_zone: i32,
  pub events_transparent: bool,
  pub keyboard_interactivity: KeyboardInteractivity,
  pub per_monitor: bool,
  pub monitor: String,
  pub namespace: String,
  pub visibility: Visibility,
  pub update_when: UpdateWhen,
}

impl Default for DesktopSettings {
  fn default() -> Self {
    Self {
      layer: Layer::Top,
      anchor: Anchor::NONE,
      width: 220,
      height: 80,
      margin: [0; 4],
      exclusive_zone: 0,
      events_transparent: false,
      keyboard_interactivity: KeyboardInteractivity::None,
      per_monitor: false,
      monitor: String::new(),
      namespace: String::new(),
      visibility: Visibility::Visible,
      update_when: UpdateWhen::OnDemand,
    }
  }
}

impl DesktopSettings {
  fn to_sl(&self) -> SlDesktopSettings {
    SlDesktopSettings {
      layer: self.layer.to_u32(),
      anchor: self.anchor.0,
      width: self.width,
      height: self.height,
      margin: self.margin,
      exclusive_zone: self.exclusive_zone,
      events_transparent: self.events_transparent,
      keyboard_interactivity: self.keyboard_interactivity.to_u32(),
      per_monitor: self.per_monitor,
      monitor: SlStr::from_str(&self.monitor),
      ns: SlStr::from_str(&self.namespace),
      visibility: match self.visibility {
        Visibility::Visible => sl_visibility::VISIBLE,
        Visibility::Transient => sl_visibility::TRANSIENT,
        Visibility::Toggleable(_) => sl_visibility::TOGGLEABLE,
      },
      visible: match self.visibility {
        Visibility::Toggleable(visible) => visible,
        _ => true,
      },
      update_when: match self.update_when {
        UpdateWhen::OnDemand => sl_update_when::ON_DEMAND,
        UpdateWhen::EveryFrame => sl_update_when::EVERY_FRAME,
        UpdateWhen::OnEvent => sl_update_when::ON_EVENT,
      },
    }
  }
}

pub struct Args<'a> {
  entries: Vec<(&'a str, &'a str)>,
}

impl<'a> Args<'a> {
  fn from_sl(args: &'a SlPayloadArgs) -> Self {
    let mut entries = Vec::with_capacity(args.count);
    if !args.keys.is_null() && !args.values.is_null() {
      for index in 0..args.count {
        unsafe {
          let key = args.keys.add(index).as_ref().and_then(|k| k.as_str());
          let value = args.values.add(index).as_ref().and_then(|v| v.as_str());
          if let (Some(key), Some(value)) = (key, value) {
            entries.push((key, value));
          }
        }
      }
    }
    Self { entries }
  }

  pub fn get(&self, key: &str) -> Option<&'a str> {
    self
      .entries
      .iter()
      .find(|(k, _)| *k == key)
      .map(|(_, value)| *value)
  }

  pub fn get_f64(&self, key: &str) -> Option<f64> {
    self.get(key).and_then(|value| value.parse().ok())
  }

  pub fn get_i64(&self, key: &str) -> Option<i64> {
    self.get(key).and_then(|value| value.parse().ok())
  }

  pub fn get_bool(&self, key: &str) -> Option<bool> {
    self.get(key).and_then(|value| value.parse().ok())
  }

  pub fn contains(&self, key: &str) -> bool {
    self.entries.iter().any(|(k, _)| *k == key)
  }

  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }
}

pub trait Component: Send + 'static {
  fn new(ctx: &Context) -> Self;
  fn events(&self) -> EventMask {
    EventMask::NONE
  }

  fn watch(&mut self, _ctx: &Context) {}
  fn update(&mut self, _ctx: &Context, _event: &Event) -> ItemEffect {
    ItemEffect::None
  }

  fn check_view(&self, _ctx: &Context) -> bool {
    true
  }

  fn stop(&mut self, _ctx: &Context) {}
  fn view(&self, ctx: &Context) -> Node;
}

pub trait Renderable: Send + 'static {
  fn new(ctx: &Context) -> Self;
  fn settings(&self) -> RenderableSettings {
    RenderableSettings::default()
  }
  fn initialize(&mut self, _ctx: &Context) {}

  fn update(&mut self, _ctx: &Context) -> ItemEffect {
    ItemEffect::None
  }

  fn handle_message(&mut self, _ctx: &Context, _message: &Message) -> ItemEffect {
    ItemEffect::None
  }
  fn view(&self, ctx: &Context) -> Node;
}

pub trait Payload: Send + 'static {
  fn new(ctx: &Context) -> Self;
  fn invoke(&mut self, ctx: &Context, args: &Args);
}

pub trait DesktopItem: Send + 'static {
  fn new(ctx: &Context) -> Self;
  fn settings(&self) -> DesktopSettings;
  fn events(&self) -> EventMask {
    EventMask::NONE
  }

  fn initialize(&mut self, _ctx: &Context) {}
  fn update(&mut self, _ctx: &Context, _event: &Event) -> ItemEffect {
    ItemEffect::None
  }

  fn view(&self, ctx: &Context) -> Node;

  fn handle_message(&mut self, _ctx: &Context, _message: &Message) -> ItemEffect {
    ItemEffect::None
  }
}

pub trait Compositor: Send + 'static {
  fn new(ctx: &Context) -> Self;

  fn is_active(&self) -> bool {
    false
  }
  fn initialize(&mut self, _ctx: &Context) {}
  fn update_state(&mut self, _ctx: &Context) {}
  fn send_command(&mut self, _ctx: &Context, _command: u32, _arg: i32) {}
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DisplayStyle {
  List,
  Grid,
  ImageList,
}

impl DisplayStyle {
  pub const fn bit(self) -> u32 {
    match self {
      Self::List => sl_display_style::LIST,
      Self::Grid => sl_display_style::GRID,
      Self::ImageList => sl_display_style::IMAGE_LIST,
    }
  }
}

#[derive(Clone, Debug, Default)]
pub struct SpotlightResult {
  pub icon: Option<String>,
  pub title: String,
  pub subtitle: Option<String>,
  pub action_label: Option<String>,
  pub image: Option<String>,
  pub tags: Vec<String>,
}

pub trait Spotlight: Send + 'static {
  fn new(ctx: &Context) -> Self;
  fn generate(&mut self, ctx: &Context, query: &str) -> Vec<SpotlightResult> {
    let _ = (ctx, query);
    Vec::new()
  }
  fn activate(&mut self, _ctx: &Context, _index: usize) {}
  fn trigger_check(&self, _ctx: &Context, _query: &str) -> bool {
    false
  }
  fn allows_triggers() -> bool {
    false
  }
  fn display_styles() -> &'static [DisplayStyle] {
    &[DisplayStyle::List]
  }
}

pub struct Registrar {
  api: *const SlHostApi,
  host: *mut c_void,
}

impl Registrar {
  #[doc(hidden)]
  pub fn new(api: *const SlHostApi, host: *mut c_void) -> Self {
    Self { api, host }
  }

  pub fn component<T: Component>(&self, name: &str) {
    unsafe { register_component(self.api, self.host, name, component_vtable::<T>()) };
  }

  pub fn renderable<T: Renderable>(&self, name: &str) {
    unsafe { register_renderable(self.api, self.host, name, renderable_vtable::<T>()) };
  }

  pub fn payload<T: Payload>(&self, command: &str) {
    unsafe { register_payload(self.api, self.host, command, payload_vtable::<T>()) };
  }

  pub fn desktop_item<T: DesktopItem>(&self, name: &str) {
    unsafe { register_desktop_item(self.api, self.host, name, desktop_vtable::<T>()) };
  }

  pub fn compositor<T: Compositor>(&self, name: &str) {
    unsafe { register_compositor(self.api, self.host, name, compositor_vtable::<T>()) };
  }

  pub fn spotlight<T: Spotlight>(&self, name: &str) {
    unsafe { register_spotlight(self.api, self.host, name, spotlight_vtable::<T>()) };
  }

  pub fn style(&self, name: &str, sheet: &StyleSheet) {
    if self.api.is_null() {
      return;
    }
    let Some(register) = (unsafe { (*self.api).style_register }) else {
      return;
    };
    let (pool, entries) = sheet.to_sl_entries();
    let sl = SlStyleSheet {
      entries: if entries.is_empty() {
        core::ptr::null()
      } else {
        entries.as_ptr()
      },
      count: entries.len() as u32,
    };
    let _ = &pool;
    unsafe { register(self.host, SlStr::from_str(name), &sl) };
  }

  pub fn config_parser<T: Send + Sync + 'static>(
    &self,
    name: &str,
    parse: impl Fn(Option<&kdl::KdlNode>) -> T + Send + Sync + 'static,
  ) {
    let key = name.to_owned();
    self.register_config_parser(name, move |node| {
      let value = parse(node);
      store_config_value(&key, value);
      Ok(())
    });
  }

  pub fn config_parser_try<T: Send + Sync + 'static>(
    &self,
    name: &str,
    parse: impl Fn(Option<&kdl::KdlNode>) -> Result<T, String> + Send + Sync + 'static,
  ) {
    let key = name.to_owned();
    self.register_config_parser(name, move |node| {
      let value = parse(node)?;
      store_config_value(&key, value);
      Ok(())
    });
  }

  fn register_config_parser(
    &self,
    name: &str,
    parse: impl Fn(Option<&kdl::KdlNode>) -> Result<(), String> + Send + Sync + 'static,
  ) {
    config_parsers()
      .lock()
      .unwrap()
      .insert(name.to_owned(), Box::new(parse));

    if self.api.is_null() {
      return;
    }
    if let Some(register) = unsafe { (*self.api).config_parser_register } {
      unsafe { register(self.host, SlStr::from_str(name), sdk_config_parser_callback) };
    }
  }
}

struct ComponentAdapter<T> {
  inner: T,
  arena: std::cell::RefCell<NodeArena>,
}

fn context_for(ctx: *mut c_void) -> Context {
  Context::new(api(), ctx)
}

fn context_for_opts(ctx: *mut c_void, opts: *mut c_void) -> Context {
  Context::with_opts(api(), ctx, opts)
}

unsafe extern "C" fn component_create<T: Component>(ctx: *mut c_void) -> *mut c_void {
  let inner = T::new(&context_for(ctx));
  let adapter = ComponentAdapter {
    inner,
    arena: std::cell::RefCell::new(NodeArena::new()),
  };
  Box::into_raw(Box::new(adapter)) as *mut c_void
}

unsafe extern "C" fn component_destroy<T: Component>(_ctx: *mut c_void, state: *mut c_void) {
  if !state.is_null() {
    unsafe { drop(Box::from_raw(state as *mut ComponentAdapter<T>)) };
  }
}

unsafe extern "C" fn component_events<T: Component>(_ctx: *mut c_void, state: *mut c_void) -> u32 {
  if state.is_null() {
    return 0;
  }
  let adapter = unsafe { &*(state as *const ComponentAdapter<T>) };
  adapter.inner.events().0
}

unsafe extern "C" fn component_watch<T: Component>(
  ctx: *mut c_void,
  state: *mut c_void,
  opts: *mut c_void,
) {
  if state.is_null() {
    return;
  }
  let adapter = unsafe { &mut *(state as *mut ComponentAdapter<T>) };
  adapter.inner.watch(&context_for_opts(ctx, opts));
}

unsafe extern "C" fn component_update<T: Component>(
  ctx: *mut c_void,
  state: *mut c_void,
  event: *const SlEvent,
) -> SlEffect {
  if state.is_null() || event.is_null() {
    return SlEffect::default();
  }
  let adapter = unsafe { &mut *(state as *mut ComponentAdapter<T>) };
  let event = Event::from_sl(unsafe { &*event });
  adapter.inner.update(&context_for(ctx), &event).to_sl()
}

unsafe extern "C" fn component_view<T: Component>(
  ctx: *mut c_void,
  state: *mut c_void,
  opts: *mut c_void,
  out: *mut SlNodeList,
) {
  let Some(out) = (unsafe { out.as_mut() }) else {
    return;
  };
  let Some(adapter) = (unsafe { (state as *const ComponentAdapter<T>).as_ref() }) else {
    return;
  };
  let context = context_for_opts(ctx, opts);
  let ui = adapter.inner.view(&context);
  let mut arena = adapter.arena.borrow_mut();
  arena.clear();
  let root = arena.materialize(&ui, &context);
  out.nodes = root;
  out.len = if root.is_null() { 0 } else { 1 };
}

unsafe extern "C" fn component_check_view<T: Component>(
  ctx: *mut c_void,
  state: *mut c_void,
  opts: *mut c_void,
) -> bool {
  let Some(adapter) = (unsafe { (state as *const ComponentAdapter<T>).as_ref() }) else {
    return true;
  };
  adapter.inner.check_view(&context_for_opts(ctx, opts))
}

unsafe extern "C" fn component_stop<T: Component>(ctx: *mut c_void, state: *mut c_void) {
  if state.is_null() {
    return;
  }
  let adapter = unsafe { &mut *(state as *mut ComponentAdapter<T>) };
  adapter.inner.stop(&context_for(ctx));
}

fn component_vtable<T: Component>() -> &'static SlComponentVtable {
  Box::leak(Box::new(SlComponentVtable {
    size: std::mem::size_of::<SlComponentVtable>() as u32,
    create: Some(component_create::<T>),
    destroy: Some(component_destroy::<T>),
    events: Some(component_events::<T>),
    watch: Some(component_watch::<T>),
    update: Some(component_update::<T>),
    view: Some(component_view::<T>),
    check_view: Some(component_check_view::<T>),
    stop: Some(component_stop::<T>),
  }))
}

struct RenderableAdapter<T> {
  inner: T,
  arena: std::cell::RefCell<NodeArena>,
}

unsafe extern "C" fn renderable_create<T: Renderable>(ctx: *mut c_void) -> *mut c_void {
  let inner = T::new(&context_for(ctx));
  Box::into_raw(Box::new(RenderableAdapter {
    inner,
    arena: std::cell::RefCell::new(NodeArena::new()),
  })) as *mut c_void
}

unsafe extern "C" fn renderable_destroy<T: Renderable>(_ctx: *mut c_void, state: *mut c_void) {
  if !state.is_null() {
    unsafe { drop(Box::from_raw(state as *mut RenderableAdapter<T>)) };
  }
}

unsafe extern "C" fn renderable_settings<T: Renderable>(
  _ctx: *mut c_void,
  state: *mut c_void,
  out: *mut SlRenderableSettings,
) {
  let Some(out) = (unsafe { out.as_mut() }) else {
    return;
  };
  let Some(adapter) = (unsafe { (state as *const RenderableAdapter<T>).as_ref() }) else {
    return;
  };
  *out = adapter.inner.settings().to_sl();
}

unsafe extern "C" fn renderable_view<T: Renderable>(
  ctx: *mut c_void,
  state: *mut c_void,
  opts: *mut c_void,
  out: *mut SlNodeList,
) {
  let Some(out) = (unsafe { out.as_mut() }) else {
    return;
  };
  let Some(adapter) = (unsafe { (state as *const RenderableAdapter<T>).as_ref() }) else {
    return;
  };
  let context = context_for_opts(ctx, opts);
  let ui = adapter.inner.view(&context);
  let mut arena = adapter.arena.borrow_mut();
  arena.clear();
  let root = arena.materialize(&ui, &context);
  out.nodes = root;
  out.len = if root.is_null() { 0 } else { 1 };
}

unsafe extern "C" fn renderable_initialize<T: Renderable>(ctx: *mut c_void, state: *mut c_void) {
  if state.is_null() {
    return;
  }
  let adapter = unsafe { &mut *(state as *mut RenderableAdapter<T>) };
  adapter.inner.initialize(&context_for(ctx));
}

unsafe extern "C" fn renderable_update<T: Renderable>(
  ctx: *mut c_void,
  state: *mut c_void,
) -> SlEffect {
  if state.is_null() {
    return SlEffect::default();
  }
  let adapter = unsafe { &mut *(state as *mut RenderableAdapter<T>) };
  adapter.inner.update(&context_for(ctx)).to_sl()
}

unsafe extern "C" fn renderable_handle_message<T: Renderable>(
  ctx: *mut c_void,
  state: *mut c_void,
  message: *const SlItemMessage,
) -> SlEffect {
  let (Some(adapter), Some(message)) = (
    unsafe { (state as *mut RenderableAdapter<T>).as_mut() },
    unsafe { message.as_ref() },
  ) else {
    return SlEffect::default();
  };
  let message = Message::from_sl(message);
  adapter
    .inner
    .handle_message(&context_for(ctx), &message)
    .to_sl()
}

fn renderable_vtable<T: Renderable>() -> &'static SlRenderableVtable {
  Box::leak(Box::new(SlRenderableVtable {
    size: std::mem::size_of::<SlRenderableVtable>() as u32,
    create: Some(renderable_create::<T>),
    destroy: Some(renderable_destroy::<T>),
    view: Some(renderable_view::<T>),
    settings: Some(renderable_settings::<T>),
    initialize: Some(renderable_initialize::<T>),
    update: Some(renderable_update::<T>),
    handle_message: Some(renderable_handle_message::<T>),
  }))
}

struct PayloadAdapter<T> {
  inner: T,
}

unsafe extern "C" fn payload_create<T: Payload>(ctx: *mut c_void) -> *mut c_void {
  let inner = T::new(&context_for(ctx));
  Box::into_raw(Box::new(PayloadAdapter { inner })) as *mut c_void
}

unsafe extern "C" fn payload_destroy<T: Payload>(_ctx: *mut c_void, state: *mut c_void) {
  if !state.is_null() {
    unsafe { drop(Box::from_raw(state as *mut PayloadAdapter<T>)) };
  }
}

unsafe extern "C" fn payload_invoke<T: Payload>(
  ctx: *mut c_void,
  state: *mut c_void,
  args: *mut c_void,
) {
  let Some(adapter) = (unsafe { (state as *mut PayloadAdapter<T>).as_mut() }) else {
    return;
  };
  let Some(args) = (unsafe { (args as *const SlPayloadArgs).as_ref() }) else {
    let empty = SlPayloadArgs::default();
    adapter
      .inner
      .invoke(&context_for(ctx), &Args::from_sl(&empty));
    return;
  };
  adapter
    .inner
    .invoke(&context_for(ctx), &Args::from_sl(args));
}

fn payload_vtable<T: Payload>() -> &'static SlPayloadVtable {
  Box::leak(Box::new(SlPayloadVtable {
    size: std::mem::size_of::<SlPayloadVtable>() as u32,
    create: Some(payload_create::<T>),
    destroy: Some(payload_destroy::<T>),
    invoke: Some(payload_invoke::<T>),
  }))
}

struct DesktopAdapter<T> {
  inner: T,
  arena: std::cell::RefCell<NodeArena>,
}

unsafe extern "C" fn desktop_create<T: DesktopItem>(ctx: *mut c_void) -> *mut c_void {
  let inner = T::new(&context_for(ctx));
  Box::into_raw(Box::new(DesktopAdapter {
    inner,
    arena: std::cell::RefCell::new(NodeArena::new()),
  })) as *mut c_void
}

unsafe extern "C" fn desktop_destroy<T: DesktopItem>(_ctx: *mut c_void, state: *mut c_void) {
  if !state.is_null() {
    unsafe { drop(Box::from_raw(state as *mut DesktopAdapter<T>)) };
  }
}

unsafe extern "C" fn desktop_settings<T: DesktopItem>(
  _ctx: *mut c_void,
  state: *mut c_void,
  out: *mut SlDesktopSettings,
) {
  let Some(out) = (unsafe { out.as_mut() }) else {
    return;
  };
  let Some(adapter) = (unsafe { (state as *const DesktopAdapter<T>).as_ref() }) else {
    return;
  };
  *out = adapter.inner.settings().to_sl();
}

unsafe extern "C" fn desktop_initialize<T: DesktopItem>(
  ctx: *mut c_void,
  state: *mut c_void,
  opts: *mut c_void,
) {
  if state.is_null() {
    return;
  }
  let adapter = unsafe { &mut *(state as *mut DesktopAdapter<T>) };
  adapter.inner.initialize(&context_for_opts(ctx, opts));
}

unsafe extern "C" fn desktop_events<T: DesktopItem>(_ctx: *mut c_void, state: *mut c_void) -> u32 {
  if state.is_null() {
    return 0;
  }
  let adapter = unsafe { &*(state as *const DesktopAdapter<T>) };
  adapter.inner.events().0
}

unsafe extern "C" fn desktop_update<T: DesktopItem>(
  ctx: *mut c_void,
  state: *mut c_void,
  event: *const SlEvent,
) -> SlEffect {
  if state.is_null() || event.is_null() {
    return SlEffect::default();
  }
  let adapter = unsafe { &mut *(state as *mut DesktopAdapter<T>) };
  let event = Event::from_sl(unsafe { &*event });
  adapter.inner.update(&context_for(ctx), &event).to_sl()
}

unsafe extern "C" fn desktop_handle_message<T: DesktopItem>(
  ctx: *mut c_void,
  state: *mut c_void,
  message: *const SlItemMessage,
) -> SlEffect {
  let (Some(adapter), Some(message)) = (
    unsafe { (state as *mut DesktopAdapter<T>).as_mut() },
    unsafe { message.as_ref() },
  ) else {
    return SlEffect::default();
  };
  let message = Message::from_sl(message);
  adapter
    .inner
    .handle_message(&context_for(ctx), &message)
    .to_sl()
}

unsafe extern "C" fn desktop_view<T: DesktopItem>(
  ctx: *mut c_void,
  state: *mut c_void,
  opts: *mut c_void,
  out: *mut SlNodeList,
) {
  let Some(out) = (unsafe { out.as_mut() }) else {
    return;
  };
  let Some(adapter) = (unsafe { (state as *const DesktopAdapter<T>).as_ref() }) else {
    return;
  };
  let context = context_for_opts(ctx, opts);
  let ui = adapter.inner.view(&context);
  let mut arena = adapter.arena.borrow_mut();
  arena.clear();
  let root = arena.materialize(&ui, &context);
  out.nodes = root;
  out.len = if root.is_null() { 0 } else { 1 };
}

fn desktop_vtable<T: DesktopItem>() -> &'static SlDesktopItemVtable {
  Box::leak(Box::new(SlDesktopItemVtable {
    size: std::mem::size_of::<SlDesktopItemVtable>() as u32,
    create: Some(desktop_create::<T>),
    destroy: Some(desktop_destroy::<T>),
    settings: Some(desktop_settings::<T>),
    initialize: Some(desktop_initialize::<T>),
    events: Some(desktop_events::<T>),
    update: Some(desktop_update::<T>),
    view: Some(desktop_view::<T>),
    handle_message: Some(desktop_handle_message::<T>),
  }))
}

struct CompositorAdapter<T> {
  inner: T,
}

unsafe extern "C" fn compositor_create<T: Compositor>(ctx: *mut c_void) -> *mut c_void {
  let inner = T::new(&context_for(ctx));
  Box::into_raw(Box::new(CompositorAdapter { inner })) as *mut c_void
}

unsafe extern "C" fn compositor_destroy<T: Compositor>(_ctx: *mut c_void, state: *mut c_void) {
  if !state.is_null() {
    unsafe { drop(Box::from_raw(state as *mut CompositorAdapter<T>)) };
  }
}

unsafe extern "C" fn compositor_is_active<T: Compositor>(
  _ctx: *mut c_void,
  state: *mut c_void,
) -> bool {
  if state.is_null() {
    return false;
  }
  let adapter = unsafe { &*(state as *const CompositorAdapter<T>) };
  adapter.inner.is_active()
}

unsafe extern "C" fn compositor_initialize<T: Compositor>(ctx: *mut c_void, state: *mut c_void) {
  if state.is_null() {
    return;
  }
  let adapter = unsafe { &mut *(state as *mut CompositorAdapter<T>) };
  adapter.inner.initialize(&context_for(ctx));
}

unsafe extern "C" fn compositor_update_state<T: Compositor>(ctx: *mut c_void, state: *mut c_void) {
  if state.is_null() {
    return;
  }
  let adapter = unsafe { &mut *(state as *mut CompositorAdapter<T>) };
  adapter.inner.update_state(&context_for(ctx));
}

unsafe extern "C" fn compositor_send_command<T: Compositor>(
  ctx: *mut c_void,
  state: *mut c_void,
  command: u32,
  arg: i32,
) {
  if state.is_null() {
    return;
  }
  let adapter = unsafe { &mut *(state as *mut CompositorAdapter<T>) };
  adapter.inner.send_command(&context_for(ctx), command, arg);
}

fn compositor_vtable<T: Compositor>() -> &'static SlCompositorVtable {
  Box::leak(Box::new(SlCompositorVtable {
    size: std::mem::size_of::<SlCompositorVtable>() as u32,
    create: Some(compositor_create::<T>),
    destroy: Some(compositor_destroy::<T>),
    is_active: Some(compositor_is_active::<T>),
    initialize: Some(compositor_initialize::<T>),
    update_state: Some(compositor_update_state::<T>),
    send_command: Some(compositor_send_command::<T>),
  }))
}

struct SpotlightAdapter<T> {
  inner: T,
  arena: std::cell::RefCell<SpotlightArena>,
}

#[derive(Default)]
struct SpotlightArena {
  strings: Vec<Box<[u8]>>,
  tags: Vec<Vec<SlStr>>,
  items: Vec<SlSpotlightItem>,
}

impl SpotlightArena {
  fn intern(&mut self, value: Option<&str>) -> SlStr {
    match value {
      Some(value) => {
        let bytes = value.as_bytes().to_vec().into_boxed_slice();
        let string = SlStr {
          ptr: bytes.as_ptr(),
          len: bytes.len(),
        };
        self.strings.push(bytes);
        string
      }
      None => SlStr::EMPTY,
    }
  }

  fn materialize(&mut self, results: &[SpotlightResult]) {
    self.strings.clear();
    self.tags.clear();
    self.items.clear();

    let mut metas = Vec::with_capacity(results.len());
    for result in results {
      let icon = self.intern(result.icon.as_deref());
      let title = self.intern(Some(&result.title));
      let subtitle = self.intern(result.subtitle.as_deref());
      let action_label = self.intern(result.action_label.as_deref());
      let image = self.intern(result.image.as_deref());

      let mut tags = Vec::with_capacity(result.tags.len());
      for tag in &result.tags {
        tags.push(self.intern(Some(tag)));
      }
      self.tags.push(tags);
      metas.push((icon, title, subtitle, action_label, image));
    }

    for (index, (icon, title, subtitle, action_label, image)) in metas.into_iter().enumerate() {
      let tags = &self.tags[index];
      self.items.push(SlSpotlightItem {
        icon,
        title,
        subtitle,
        action_label,
        image,
        tags: if tags.is_empty() {
          core::ptr::null()
        } else {
          tags.as_ptr()
        },
        tag_count: tags.len(),
      });
    }
  }
}

unsafe extern "C" fn spotlight_create<T: Spotlight>(ctx: *mut c_void) -> *mut c_void {
  let inner = T::new(&context_for(ctx));
  Box::into_raw(Box::new(SpotlightAdapter {
    inner,
    arena: std::cell::RefCell::new(SpotlightArena::default()),
  })) as *mut c_void
}

unsafe extern "C" fn spotlight_destroy<T: Spotlight>(_ctx: *mut c_void, state: *mut c_void) {
  if !state.is_null() {
    unsafe { drop(Box::from_raw(state as *mut SpotlightAdapter<T>)) };
  }
}

unsafe extern "C" fn spotlight_generate<T: Spotlight>(
  ctx: *mut c_void,
  state: *mut c_void,
  query: SlStr,
  out: *mut SlSpotlightList,
) -> i32 {
  let Some(out) = (unsafe { out.as_mut() }) else {
    return -1;
  };
  let Some(adapter) = (unsafe { (state as *mut SpotlightAdapter<T>).as_mut() }) else {
    return -1;
  };
  let query = unsafe { query.as_str() }.unwrap_or_default();
  let results = adapter.inner.generate(&context_for(ctx), query);
  let mut arena = adapter.arena.borrow_mut();
  arena.materialize(&results);
  out.items = arena.items.as_ptr();
  out.count = arena.items.len();
  0
}

unsafe extern "C" fn spotlight_activate<T: Spotlight>(
  ctx: *mut c_void,
  state: *mut c_void,
  index: usize,
) {
  if state.is_null() {
    return;
  }
  let adapter = unsafe { &mut *(state as *mut SpotlightAdapter<T>) };
  adapter.inner.activate(&context_for(ctx), index);
}

unsafe extern "C" fn spotlight_trigger_check<T: Spotlight>(
  ctx: *mut c_void,
  state: *mut c_void,
  query: SlStr,
) -> bool {
  if state.is_null() {
    return false;
  }
  let adapter = unsafe { &mut *(state as *mut SpotlightAdapter<T>) };
  let query_str = unsafe { query.as_str() }.unwrap_or_default();
  adapter.inner.trigger_check(&context_for(ctx), query_str)
}

fn spotlight_vtable<T: Spotlight>() -> &'static SlSpotlightVtable {
  let display_styles = T::display_styles()
    .iter()
    .fold(0, |bits, style| bits | style.bit());

  Box::leak(Box::new(SlSpotlightVtable {
    size: std::mem::size_of::<SlSpotlightVtable>() as u32,
    create: Some(spotlight_create::<T>),
    destroy: Some(spotlight_destroy::<T>),
    generate: Some(spotlight_generate::<T>),
    activate: Some(spotlight_activate::<T>),
    trigger_check: Some(spotlight_trigger_check::<T>),
    allows_triggers: T::allows_triggers(),
    display_styles,
  }))
}

#[macro_export]
macro_rules! export_plugin {
  ($register:path) => {
    #[unsafe(no_mangle)]
    pub extern "C" fn slowshell_plugin_meta() -> $crate::SlPluginMeta {
      $crate::SlPluginMeta {
        abi_version: $crate::SL_PLUGIN_ABI_VERSION,
        id: $crate::SlStr::from_str(env!("CARGO_PKG_NAME")),
        version: $crate::SlStr::from_str(env!("CARGO_PKG_VERSION")),
      }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slowshell_plugin_init(
      api: *const $crate::SlHostApi,
      host: *mut core::ffi::c_void,
      _userdata: *mut *mut core::ffi::c_void,
    ) -> i32 {
      if api.is_null() {
        return -1;
      }
      $crate::set_api(api);
      let mut registrar = $crate::Registrar::new(api, host);
      $register(&mut registrar);
      0
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slowshell_plugin_shutdown(_userdata: *mut core::ffi::c_void) {}
  };
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn materializes_column_with_leaf_children() {
    let ctx = Context::new(core::ptr::null(), core::ptr::null_mut());
    let mut arena = NodeArena::new();

    let ui = Node::Column {
      children: vec![
        Node::Text {
          content: "hello".into(),
          size: 12.0,
          color: Some(Color::WHITE),
        },
        Node::icon("applications-system-symbolic").size(16.0),
      ],
      spacing: 6.0,
      align: Align::Start,
    };

    let root = arena.materialize(&ui, &ctx);
    unsafe {
      let column = &*root;
      assert_eq!(column.kind, sl_node_kind::COLUMN);
      assert_eq!(column.spacing, 6.0);
      assert_eq!(column.child_count, 2);
      assert!(!column.children.is_null());

      let text = &*column.children;
      assert_eq!(text.kind, sl_node_kind::TEXT);
      assert_eq!(text.text.as_str(), Some("hello"));
      assert_eq!(text.size, 12.0);
      assert!(text.style.has_text_color);

      let icon = &*column.children.add(1);
      assert_eq!(icon.kind, sl_node_kind::ICON);
      assert_eq!(icon.text.as_str(), Some("applications-system-symbolic"));
    }
  }

  #[test]
  fn materializes_action_wrapper() {
    let ctx = Context::new(core::ptr::null(), core::ptr::null_mut());
    let mut arena = NodeArena::new();

    let ui =
      Node::clickable(Node::text("Reset counter").color(Color::WHITE)).action("example.reset");

    let root = arena.materialize(&ui, &ctx);
    unsafe {
      let wrapper = &*root;
      assert_eq!(wrapper.child_count, 1);
      assert_eq!(wrapper.action.as_str(), Some("example.reset"));
      assert!(!wrapper.has_effect);
      assert!(!wrapper.children.is_null());
      let child = &*wrapper.children;
      assert_eq!(child.kind, sl_node_kind::TEXT);
    }
  }

  #[test]
  fn materializes_clickable_effect_and_scrollable() {
    let ctx = Context::new(core::ptr::null(), core::ptr::null_mut());
    let mut arena = NodeArena::new();

    let ui = Node::clickable(Node::text("Confirm")).effect([40, 1, 0, 0]);
    let root = arena.materialize(&ui, &ctx);
    unsafe {
      let wrapper = &*root;
      assert!(wrapper.has_effect);
      assert_eq!(wrapper.effect, [40, 1, 0, 0]);
      assert!(wrapper.action.is_empty());
    }

    let ui = Node::scrollable(Node::text("long list")).height(Length::Fill);
    let root = arena.materialize(&ui, &ctx);
    unsafe {
      let scrollable = &*root;
      assert_eq!(scrollable.kind, sl_node_kind::SCROLLABLE);
      assert_eq!(scrollable.child_count, 1);
      assert_eq!(scrollable.height.unit, sl_length_unit::FILL);
      let child = &*scrollable.children;
      assert_eq!(child.kind, sl_node_kind::TEXT);
    }
  }

  #[test]
  fn materializes_container_with_style_and_single_child() {
    let ctx = Context::new(core::ptr::null(), core::ptr::null_mut());
    let mut arena = NodeArena::new();

    let ui = Node::Container {
      child: Box::new(Node::text("inside")),
      padding: Padding::all(8.0),
      align_x: Align::Center,
      align_y: Align::Center,
      style: Some(Style {
        background: Some(Color::rgb(0.1, 0.1, 0.1)),
        border: None,
        text_color: None,
      }),
      width: Length::Fixed(120.0),
      height: Length::Fill,
    };

    let root = arena.materialize(&ui, &ctx);
    unsafe {
      let container = &*root;
      assert_eq!(container.kind, sl_node_kind::CONTAINER);
      assert_eq!(container.align_x, sl_align::CENTER);
      assert_eq!(container.padding, [8.0, 8.0, 8.0, 8.0]);
      assert!(container.style.has_background);
      assert_eq!(container.child_count, 1);
      assert!(!container.children.is_null());
      assert_eq!(container.width.unit, sl_length_unit::FIXED);
      assert_eq!(container.width.value, 120.0);
      assert_eq!(container.height.unit, sl_length_unit::FILL);

      let child = &*container.children;
      assert_eq!(child.kind, sl_node_kind::TEXT);
      assert_eq!(child.text.as_str(), Some("inside"));
    }
  }

  #[test]
  fn materializes_container_with_builder_lengths() {
    let ctx = Context::new(core::ptr::null(), core::ptr::null_mut());
    let mut arena = NodeArena::new();

    let default_ui = Node::container(Node::text("x"));
    let default_root = arena.materialize(&default_ui, &ctx);
    let default_node = unsafe { &*default_root };
    assert_eq!(default_node.width.unit, sl_length_unit::SHRINK);
    assert_eq!(default_node.height.unit, sl_length_unit::SHRINK);

    let scaled = Node::container(Node::text("x"))
      .width(Length::Shrink)
      .height(Length::Fixed(24.0));
    let scaled_root = arena.materialize(&scaled, &ctx);
    let scaled_node = unsafe { &*scaled_root };
    assert_eq!(scaled_node.width.unit, sl_length_unit::SHRINK);
    assert_eq!(scaled_node.height.unit, sl_length_unit::FIXED);
    assert_eq!(scaled_node.height.value, 24.0);

    let leaf = Node::text("x").width(Length::Fixed(10.0));
    let leaf_root = arena.materialize(&leaf, &ctx);
    let leaf_node = unsafe { &*leaf_root };
    assert_eq!(leaf_node.width.unit, sl_length_unit::SHRINK);
  }

  #[test]
  fn materializes_canvas_without_host() {
    let ctx = Context::new(core::ptr::null(), core::ptr::null_mut());
    let mut arena = NodeArena::new();

    let ui = Node::canvas_cached(7, 8, 8, |buffer| buffer.fill(0));

    let root = arena.materialize(&ui, &ctx);
    unsafe {
      let canvas = &*root;
      assert_eq!(canvas.kind, sl_node_kind::CANVAS);
      assert_eq!(canvas.canvas.width, 8);
      assert_eq!(canvas.canvas.height, 8);
      assert_eq!(canvas.canvas.cache_key, 7);
      assert!(canvas.canvas.data.is_null());
    }
  }

  #[test]
  fn example_menu_materializes_reset_and_close() {
    let ctx = Context::new(core::ptr::null(), core::ptr::null_mut());
    let mut arena = NodeArena::new();

    let row = |label: &str| Node::Row {
      children: vec![
        Node::icon("edit-undo-symbolic").size(14.0),
        Node::text(label).size(12.0),
      ],
      spacing: 8.0,
      align: Align::Center,
    };

    let ui = Node::Column {
      children: vec![
        Node::clickable(row("Reset counter")).action("example.reset"),
        Node::clickable(row("Close")).action("popup.close"),
      ],
      spacing: 6.0,
      align: Align::Start,
    };

    let root = arena.materialize(&ui, &ctx);
    unsafe {
      let column = &*root;
      assert_eq!(column.kind, sl_node_kind::COLUMN);
      assert_eq!(column.child_count, 2);

      let reset = &*column.children;
      assert_eq!(reset.kind, sl_node_kind::CONTAINER);
      assert_eq!(reset.action.as_str(), Some("example.reset"));
      assert_eq!(reset.child_count, 1);
      let reset_row = &*reset.children;
      assert_eq!(reset_row.kind, sl_node_kind::ROW);
      assert_eq!(reset_row.child_count, 2);
      assert_eq!(
        reset_row.children.add(1).as_ref().unwrap().text.as_str(),
        Some("Reset counter")
      );

      let close = &*column.children.add(1);
      assert_eq!(close.kind, sl_node_kind::CONTAINER);
      assert_eq!(close.action.as_str(), Some("popup.close"));
      assert_eq!(close.child_count, 1);
      let close_row = &*close.children;
      assert_eq!(close_row.kind, sl_node_kind::ROW);
      assert_eq!(close_row.child_count, 2);
      assert_eq!(
        close_row.children.add(1).as_ref().unwrap().text.as_str(),
        Some("Close")
      );

      assert_ne!(reset.children as usize, close.children as usize);
    }
  }

  #[test]
  fn parses_payload_args() {
    let key = String::from("name");
    let value = String::from("home");
    let sl_args = SlPayloadArgs {
      count: 1,
      keys: &SlStr {
        ptr: key.as_ptr(),
        len: key.len(),
      },
      values: &SlStr {
        ptr: value.as_ptr(),
        len: value.len(),
      },
    };

    let args = Args::from_sl(&sl_args);
    assert!(!args.is_empty());
    assert_eq!(args.get("name"), Some("home"));
    assert_eq!(args.get("missing"), None);
  }

  #[test]
  fn settings_map_to_sl() {
    let settings = RenderableSettings::popup(200.0, 0.0);
    let sl = settings.to_sl();
    assert!(sl.wrap_popup);
    assert_eq!(sl.width, 200.0);

    let desktop = DesktopSettings {
      layer: Layer::Top,
      anchor: Anchor::TOP | Anchor::RIGHT,
      namespace: "example-widget".into(),
      ..Default::default()
    };
    let sl = desktop.to_sl();
    assert_eq!(sl.layer, sl_layer::TOP);
    assert_eq!(sl.anchor, sl_anchor::TOP | sl_anchor::RIGHT);
    assert_eq!(unsafe { sl.ns.as_str() }, Some("example-widget"));
  }

  #[test]
  fn theme_resolves_named_colors_and_alpha() {
    let theme = Theme::default();
    assert!(theme.color("primary").is_some());
    assert_eq!(theme.color("primary").unwrap().a, 1.0);
    assert_eq!(theme.color("base/0.2").unwrap().a, 0.2);
    assert_eq!(theme.color("base").unwrap().a, 1.0);
    assert_eq!(theme.color("nope"), None);
  }

  #[test]
  fn style_value_from_string_detects_colors() {
    assert!(matches!(
      StyleValue::from("transparent"),
      StyleValue::Color(ColorValue::Transparent)
    ));
    assert!(matches!(
      StyleValue::from("#ff8800"),
      StyleValue::Color(ColorValue::Hex(hex)) if hex == "#ff8800"
    ));
    assert!(matches!(StyleValue::from("hello"), StyleValue::String(s) if s == "hello"));
    assert!(matches!(StyleValue::from(12), StyleValue::Integer(12)));
    assert!(matches!(StyleValue::from(1.5), StyleValue::Float(f) if f == 1.5));
    assert!(matches!(StyleValue::from(true), StyleValue::Boolean(true)));
  }

  #[test]
  fn stylesheet_queries_resolve_colors_numbers_and_booleans() {
    let theme = Theme::default();
    let mut sheet = StyleSheet::default();
    sheet.insert("background", "#10101e");
    sheet.insert("accent", ColorValue::Theme("primary".into()));
    sheet.insert("background.opacity", 0.5);
    sheet.insert("radius", 8);
    sheet.insert("spacing", 4.5);
    sheet.insert("shadow", true);
    sheet.insert("title", "hello");

    let background = sheet.get_color(&theme, "background").unwrap();
    assert!((background.r - 16.0 / 255.0).abs() < 1e-4);
    assert!((background.a - 0.5).abs() < 1e-4);
    assert_eq!(
      sheet.color(&theme, "accent", Color::WHITE),
      theme.color("primary").unwrap()
    );
    assert_eq!(sheet.color(&theme, "accent", Color::WHITE).a, 1.0);
    assert_eq!(sheet.color(&theme, "missing", Color::WHITE), Color::WHITE);
    assert_eq!(sheet.number("radius"), Some(8.0));
    assert_eq!(sheet.number("spacing"), Some(4.5));
    assert!(sheet.bool("shadow"));
    assert_eq!(
      sheet.get("title"),
      Some(&StyleValue::String("hello".into()))
    );
  }

  #[test]
  fn stylesheet_roundtrips_through_abi_entries() {
    let mut sheet = StyleSheet::default();
    sheet.insert("background", "#112233");
    sheet.insert("text", ColorValue::Theme("text".into()));
    sheet.insert("radius", 6);
    sheet.insert("shadow", false);
    sheet.insert("label", "hi");

    let (pool, entries) = sheet.to_sl_entries();
    let sl = SlStyleSheet {
      entries: if entries.is_empty() {
        core::ptr::null()
      } else {
        entries.as_ptr()
      },
      count: entries.len() as u32,
    };
    let _ = &pool;

    let back = StyleSheet::from_sl(&sl);
    assert_eq!(
      back.get("background"),
      Some(&StyleValue::Color(ColorValue::Hex("#112233".into())))
    );
    assert_eq!(
      back.get("text"),
      Some(&StyleValue::Color(ColorValue::Theme("text".into())))
    );
    assert_eq!(back.get("radius"), Some(&StyleValue::Integer(6)));
    assert_eq!(back.get("shadow"), Some(&StyleValue::Boolean(false)));
    assert_eq!(back.get("label"), Some(&StyleValue::String("hi".into())));
  }

  #[test]
  fn style_and_theme_unavailable_without_api() {
    let ctx = Context::new(core::ptr::null(), core::ptr::null_mut());
    assert!(ctx.style("anything").is_empty());
    assert_eq!(ctx.theme().base, Theme::default().base);
  }

  #[test]
  fn store_shares_typed_and_raw_values_across_surfaces() {
    let store = Context::new(core::ptr::null(), core::ptr::null_mut()).store();
    let key = 0x534c_4f57_0001;
    let other = 0x534c_4f57_0002;

    assert!(!store.contains(key));
    assert!(store.insert(key, 41u32).is_none());
    assert!(store.contains(key));
    assert_eq!(
      store.with::<u32, _>(key, |value| {
        *value += 1;
        *value
      }),
      Some(42)
    );
    assert_eq!(store.remove::<u32>(key), Some(42));
    assert!(!store.contains(key));

    let boxed = Box::into_raw(Box::new(7u8)) as *mut c_void;
    store.set_raw(key, boxed);
    assert_eq!(store.raw(key), Some(boxed));
    let raw = store.remove_raw(key).expect("raw value");
    assert_eq!(unsafe { *(raw as *mut u8) }, 7);
    unsafe { drop(Box::from_raw(raw as *mut u8)) };
    assert!(store.raw(key).is_none());

    assert!(store.insert(other, 1u8).is_none());
    assert_eq!(store.remove::<u64>(other), None);
    assert_eq!(store.remove::<u8>(other), Some(1));
  }

  #[test]
  fn options_are_empty_without_a_host() {
    let ctx = Context::new(core::ptr::null(), core::ptr::null_mut());
    let options = ctx.options();
    assert!(options.str("label").is_none());
    assert!(options.f64("n").is_none());
    assert!(options.i64("n").is_none());
    assert!(options.bool("flag").is_none());
  }

  #[test]
  fn error_without_a_host_is_a_noop() {
    let ctx = Context::new(core::ptr::null(), core::ptr::null_mut());
    ctx.error(1, "ignored");
  }

  #[test]
  fn notify_without_a_host_returns_none() {
    let ctx = Context::new(core::ptr::null(), core::ptr::null_mut());
    let notification = Notification::new("hi", "there").action("Reset", "example.reset");
    assert_eq!(ctx.notify(&notification), None);
  }

  #[test]
  fn config_parser_parses_and_stores_named_entry() {
    let registrar = Registrar::new(core::ptr::null(), core::ptr::null_mut());
    registrar.config_parser::<String>("sdk.configtest", |node| {
      node
        .and_then(|node| node.get(0))
        .and_then(kdl::KdlValue::as_string)
        .map(str::to_owned)
        .unwrap_or_default()
    });

    let ctx = Context::new(core::ptr::null(), core::ptr::null_mut());
    assert_eq!(ctx.config::<String>("sdk.configtest"), None);

    unsafe {
      assert_eq!(
        sdk_config_parser_callback(
          core::ptr::null_mut(),
          SlStr::from_str("sdk.configtest"),
          SlStr::from_str("sdk.configtest \"hello\""),
        ),
        0
      );
    }
    assert_eq!(
      ctx.config::<String>("sdk.configtest"),
      Some("hello".to_owned())
    );

    registrar.config_parser::<bool>("sdk.configtest", |node| node.is_some());
    unsafe {
      assert_eq!(
        sdk_config_parser_callback(
          core::ptr::null_mut(),
          SlStr::from_str("sdk.configtest"),
          SlStr::EMPTY,
        ),
        0
      )
    };
    assert_eq!(ctx.config::<bool>("sdk.configtest"), Some(false));
  }

  #[test]
  fn config_parser_try_reports_failure() {
    let registrar = Registrar::new(core::ptr::null(), core::ptr::null_mut());
    registrar.config_parser_try::<u32>("sdk.trytest", |_node| Err("bad config".to_owned()));

    let rc = unsafe {
      sdk_config_parser_callback(
        core::ptr::null_mut(),
        SlStr::from_str("sdk.trytest"),
        SlStr::from_str("sdk.trytest 1"),
      )
    };
    assert_eq!(rc, 1);

    let ctx = Context::new(core::ptr::null(), core::ptr::null_mut());
    assert_eq!(ctx.config::<u32>("sdk.trytest"), None);
  }

  #[test]
  fn compositor_state_copies_abi_buffers() {
    let monitor = SlMonitor {
      name: SlStr::from_str("DP-1"),
      width: 1920,
      height: 1080,
      scale: 1.0,
    };
    let workspace = SlWorkspace {
      id: 7,
      idx: 3,
      output: SlStr::from_str("DP-1"),
      name: SlStr::from_str("three"),
      is_active: true,
      is_focused: true,
      is_urgent: false,
    };
    let window = SlWindow {
      id: 0,
      title: SlStr::from_str("terminal"),
      wclass: SlStr::from_str("kitty"),
    };
    let state = SlCompositorState {
      monitors: &monitor,
      monitor_count: 1,
      workspaces: &workspace,
      workspace_count: 1,
      active_window: &window,
      overview_active: true,
    };

    let copied = unsafe { CompositorState::from_sl(&state) };
    assert_eq!(
      copied.monitors,
      vec![Monitor {
        name: "DP-1".into(),
        width: 1920,
        height: 1080,
        scale: 1.0,
      }]
    );
    assert_eq!(copied.workspaces.len(), 1);
    assert_eq!(copied.workspaces[0].name.as_deref(), Some("three"));
    assert_eq!(copied.workspaces[0].output.as_deref(), Some("DP-1"));
    assert!(copied.workspaces[0].is_focused);
    assert_eq!(
      copied.active_window,
      Some(Window {
        id: 0,
        title: "terminal".into(),
        class: "kitty".into(),
      })
    );
    assert!(copied.overview_active);
  }

  #[test]
  fn check_view_defaults_to_true() {
    struct Plain;
    impl Component for Plain {
      fn new(_ctx: &Context) -> Self {
        Plain
      }
      fn view(&self, _ctx: &Context) -> Node {
        Node::text("x")
      }
    }

    let ctx = Context::new(core::ptr::null(), core::ptr::null_mut());
    assert!(Plain.check_view(&ctx));
  }
}
