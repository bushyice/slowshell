use core::marker::PhantomData;

use abi_stable::{
  StableAbi, sabi_trait,
  std_types::{RBox, ROption, RStr, RString, RVec},
};

use crate::{
  payload::{PayloadBox, PayloadParserBox},
  store::{ErasedRef, ErasedValue, Store, StoreCallbackBox, TypeKey},
  surfaces::{
    ComponentFactoryBox, CompositorFactoryBox, ConfigParserBox, DeployableDesktopItemFactoryBox,
    DesktopItemFactoryBox, PayloadFactoryBox, QuickSettingsFactoryBox, RenderableFactoryBox,
    SpotlightFactoryBox,
  },
  types::{
    AudioState, BluetoothState, CanvasRef, Color, CompositorState, Length, NetworkState, Node,
    Notification, PowerState, SystemState, Theme, TrayState,
  },
};

#[sabi_trait]
pub trait Host: Send + Sync + 'static {
  fn log(&self, level: i32, message: RString);
  fn plugin_error(&self, code: u32, message: RString);

  fn option_str(&self, key: RStr<'_>) -> ROption<RString>;
  fn option_f64(&self, key: RStr<'_>) -> ROption<f64>;
  fn option_i64(&self, key: RStr<'_>) -> ROption<i64>;
  fn option_bool(&self, key: RStr<'_>) -> ROption<bool>;

  fn config_str(&self, key: RStr<'_>) -> ROption<RString>;
  fn config_f64(&self, key: RStr<'_>) -> ROption<f64>;
  fn config_i64(&self, key: RStr<'_>) -> ROption<i64>;
  fn config_bool(&self, key: RStr<'_>) -> ROption<bool>;

  fn set_interval(&self, millis: u32, repeating: bool, tag: u32) -> i32;
  fn register_fd(&self, fd: i32, action: u32) -> i32;
  fn request_redraw(&self, window: u64);

  fn compositor_state(&self) -> ROption<CompositorState>;
  fn set_compositor_state(&self, state: CompositorState);
  fn compositor_register_fd(&self, fd: i32, flags: u32) -> i32;

  fn audio_state(&self) -> ROption<AudioState>;
  fn bluetooth_state(&self) -> ROption<BluetoothState>;
  fn network_state(&self) -> ROption<NetworkState>;
  fn system_state(&self) -> ROption<SystemState>;
  fn tray_state(&self) -> ROption<TrayState>;
  fn power_state(&self) -> ROption<PowerState>;

  fn style_number(&self, classes: RStr<'_>, property: RStr<'_>) -> ROption<f32>;
  fn style_bool(&self, classes: RStr<'_>, property: RStr<'_>) -> ROption<bool>;
  fn style_color(&self, classes: RStr<'_>, property: RStr<'_>) -> ROption<Color>;
  fn theme(&self) -> Theme;
  fn register_style(&self, css: RStr<'_>) -> i32;

  fn dispatch(&self, command: RStr<'_>) -> bool;
  fn dispatch_with_string(&self, name: RStr<'_>, payload: RStr<'_>) -> bool;

  fn persistence_get(&self, key: RStr<'_>) -> ROption<RString>;
  fn persistence_set(&self, key: RStr<'_>, value: RStr<'_>) -> bool;
  fn persistence_remove(&self, key: RStr<'_>) -> bool;

  fn notify(&self, notification: Notification) -> ROption<u32>;

  fn store_insert(&self, key: TypeKey, value: ErasedValue) -> bool;
  fn store_get<'a>(&self, ctx: &Context<'a>, key: TypeKey) -> ROption<ErasedRef<'a>>;
  fn store_remove(&self, key: TypeKey) -> ROption<ErasedValue>;
  fn store_contains(&self, key: TypeKey) -> bool;
  fn store_subscribe(&self, key: TypeKey, callback: StoreCallbackBox);

  fn register_component(&self, name: RString, factory: ComponentFactoryBox);
  fn register_renderable(&self, name: RString, factory: RenderableFactoryBox);
  fn register_payload(&self, command: RString, factory: PayloadFactoryBox);
  fn register_desktop_item(&self, name: RString, factory: DesktopItemFactoryBox);
  fn register_desktop_item_deployer(&self, name: RString, factory: DeployableDesktopItemFactoryBox);
  fn register_compositor(&self, name: RString, factory: CompositorFactoryBox);
  fn register_spotlight(&self, name: RString, factory: SpotlightFactoryBox);
  fn register_quick_settings(&self, name: RString, factory: QuickSettingsFactoryBox);
  fn register_config_parser(&self, name: RString, parser: ConfigParserBox);
  fn register_command(&self, name: RString, title: RString, description: RString);

  fn register_payload_parser(&self, commands: RVec<RString>, parser: PayloadParserBox);

  fn dispatch_payload(&self, name: RStr<'_>, payload: PayloadBox) -> bool;

  fn canvas(&self, width: u32, height: u32) -> ROption<CanvasRef>;

  fn store_touch(&self, key: TypeKey) -> bool;
}

pub type HostRef<'a> = &'a Host_TO<RBox<()>>;

#[repr(C)]
#[derive(StableAbi, Clone, Copy)]
pub struct Context<'a> {
  host: HostRef<'a>,
}

impl<'a> Context<'a> {
  pub fn new(host: HostRef<'a>) -> Self {
    Self { host }
  }

  pub fn host(&self) -> HostRef<'a> {
    self.host
  }

  pub fn log(&self, level: i32, message: impl Into<RString>) {
    self.host.log(level, message.into());
  }

  pub fn error(&self, code: u32, message: impl Into<RString>) {
    self.host.plugin_error(code, message.into());
  }

  pub fn set_interval(&self, millis: u32, repeating: bool, tag: u32) -> i32 {
    self.host.set_interval(millis, repeating, tag)
  }

  pub fn register_fd(&self, fd: i32, action: u32) -> i32 {
    self.host.register_fd(fd, action)
  }

  pub fn request_redraw(&self, window: u64) {
    self.host.request_redraw(window);
  }

  pub fn options(&self) -> Options<'a> {
    Options { ctx: *self }
  }

  pub fn config_str(&self, key: &str) -> Option<RString> {
    match self.host.config_str(RStr::from_str(key)) {
      ROption::RSome(value) => Some(value),
      ROption::RNone => None,
    }
  }

  pub fn config_f64(&self, key: &str) -> Option<f64> {
    match self.host.config_f64(RStr::from_str(key)) {
      ROption::RSome(value) => Some(value),
      ROption::RNone => None,
    }
  }

  pub fn config_i64(&self, key: &str) -> Option<i64> {
    match self.host.config_i64(RStr::from_str(key)) {
      ROption::RSome(value) => Some(value),
      ROption::RNone => None,
    }
  }

  pub fn config_bool(&self, key: &str) -> Option<bool> {
    match self.host.config_bool(RStr::from_str(key)) {
      ROption::RSome(value) => Some(value),
      ROption::RNone => None,
    }
  }

  pub fn style(&self, classes: &str) -> StyleRef<'a> {
    StyleRef {
      ctx: *self,
      classes: RString::from(classes),
    }
  }

  pub fn resolve(&self, classes: &[&str]) -> StyleRef<'a> {
    self.style(&classes.join(" "))
  }

  pub fn theme(&self) -> Theme {
    self.host.theme()
  }

  pub fn compositor_state(&self) -> Option<CompositorState> {
    match self.host.compositor_state() {
      ROption::RSome(state) => Some(state),
      ROption::RNone => None,
    }
  }

  pub fn set_compositor_state(&self, state: CompositorState) {
    self.host.set_compositor_state(state);
  }

  pub fn dispatch(&self, command: &str) -> bool {
    self.host.dispatch(RStr::from_str(command))
  }

  pub fn dispatch_with_string(&self, name: &str, payload: &str) -> bool {
    self
      .host
      .dispatch_with_string(RStr::from_str(name), RStr::from_str(payload))
  }

  pub fn dispatch_payload(&self, name: &str, payload: PayloadBox) -> bool {
    self.host.dispatch_payload(RStr::from_str(name), payload)
  }

  pub fn persistence_get_raw(&self, key: &str) -> Option<RString> {
    match self.host.persistence_get(RStr::from_str(key)) {
      ROption::RSome(value) => Some(value),
      ROption::RNone => None,
    }
  }

  pub fn persistence_set_raw(&self, key: &str, value: &str) -> bool {
    self
      .host
      .persistence_set(RStr::from_str(key), RStr::from_str(value))
  }

  pub fn persistence_remove(&self, key: &str) -> bool {
    self.host.persistence_remove(RStr::from_str(key))
  }

  pub fn persistence_get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
    let raw = self.persistence_get_raw(key)?;
    match ron::from_str::<T>(&raw) {
      Ok(value) => Some(value),
      Err(e) => {
        self.log(1, format!("[persistence] failed to decode {key:?}: {e}"));
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
    match ron::ser::to_string(value) {
      Ok(raw) => self.persistence_set_raw(key, &raw),
      Err(_) => false,
    }
  }

  pub fn persistence_has(&self, key: &str) -> bool {
    self.persistence_get_raw(key).is_some()
  }

  pub fn notify(&self, notification: &Notification) -> Option<u32> {
    match self.host.notify(notification.clone()) {
      ROption::RSome(id) => Some(id),
      ROption::RNone => None,
    }
  }

  pub fn audio_state(&self) -> Option<AudioState> {
    match self.host.audio_state() {
      ROption::RSome(state) => Some(state),
      ROption::RNone => None,
    }
  }

  pub fn bluetooth_state(&self) -> Option<BluetoothState> {
    match self.host.bluetooth_state() {
      ROption::RSome(state) => Some(state),
      ROption::RNone => None,
    }
  }

  pub fn network_state(&self) -> Option<NetworkState> {
    match self.host.network_state() {
      ROption::RSome(state) => Some(state),
      ROption::RNone => None,
    }
  }

  pub fn system_state(&self) -> Option<SystemState> {
    match self.host.system_state() {
      ROption::RSome(state) => Some(state),
      ROption::RNone => None,
    }
  }

  pub fn tray_state(&self) -> Option<TrayState> {
    match self.host.tray_state() {
      ROption::RSome(state) => Some(state),
      ROption::RNone => None,
    }
  }

  pub fn power_state(&self) -> Option<PowerState> {
    match self.host.power_state() {
      ROption::RSome(state) => Some(state),
      ROption::RNone => None,
    }
  }

  pub fn store(&self) -> Store<'a> {
    Store::new(*self)
  }

  pub fn canvas(&self, width: u32, height: u32) -> Option<CanvasBuffer<'_>> {
    match self.host.canvas(width, height) {
      ROption::RSome(reference) if !reference.ptr.is_null() && reference.len > 0 => {
        Some(CanvasBuffer {
          ptr: reference.ptr,
          len: reference.len,
          id: reference.id,
          marker: PhantomData,
        })
      }
      _ => None,
    }
  }

  pub fn canvas_node(&self, width: u32, height: u32, draw: impl FnOnce(&mut [u8])) -> Node {
    self.canvas_node_inner(0, width, height, draw)
  }

  pub fn canvas_cached_node(
    &self,
    cache_key: u64,
    width: u32,
    height: u32,
    draw: impl FnOnce(&mut [u8]),
  ) -> Node {
    self.canvas_node_inner(cache_key, width, height, draw)
  }

  fn canvas_node_inner(
    &self,
    cache_key: u64,
    width: u32,
    height: u32,
    draw: impl FnOnce(&mut [u8]),
  ) -> Node {
    match self.canvas(width, height) {
      Some(mut buffer) => {
        draw(&mut buffer);
        Node::Canvas {
          cache_key,
          pixel_width: width,
          pixel_height: height,
          width: Length::Shrink,
          height: Length::Shrink,
          pixels: RVec::new(),
          pool_id: buffer.id(),
          classes: RVec::new(),
        }
      }
      None => {
        if cache_key == 0 {
          Node::canvas(width, height, draw)
        } else {
          Node::canvas_cached(cache_key, width, height, draw)
        }
      }
    }
  }

  pub fn config<T: Send + Sync + Clone + 'static>(&self, name: &str) -> Option<T> {
    self.with_config(name, |value: &T| value.clone())
  }

  pub fn with_config<T: Send + Sync + 'static, R>(
    &self,
    name: &str,
    f: impl FnOnce(&T) -> R,
  ) -> Option<R> {
    crate::sdk::config_values()
      .lock()
      .unwrap()
      .get(name)
      .and_then(|value| value.downcast_ref::<T>())
      .map(f)
  }
}

pub struct CanvasBuffer<'a> {
  ptr: *mut u8,
  len: usize,
  id: u64,
  marker: PhantomData<&'a mut [u8]>,
}

impl CanvasBuffer<'_> {
  pub fn id(&self) -> u64 {
    self.id
  }
}

impl core::ops::Deref for CanvasBuffer<'_> {
  type Target = [u8];

  fn deref(&self) -> &[u8] {
    unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
  }
}

impl core::ops::DerefMut for CanvasBuffer<'_> {
  fn deref_mut(&mut self) -> &mut [u8] {
    unsafe { core::slice::from_raw_parts_mut(self.ptr, self.len) }
  }
}

#[derive(Clone, Copy)]
pub struct Options<'a> {
  ctx: Context<'a>,
}

impl Options<'_> {
  pub fn str(&self, key: &str) -> Option<RString> {
    match self.ctx.host.option_str(RStr::from_str(key)) {
      ROption::RSome(value) => Some(value),
      ROption::RNone => None,
    }
  }

  pub fn f64(&self, key: &str) -> Option<f64> {
    match self.ctx.host.option_f64(RStr::from_str(key)) {
      ROption::RSome(value) => Some(value),
      ROption::RNone => None,
    }
  }

  pub fn i64(&self, key: &str) -> Option<i64> {
    match self.ctx.host.option_i64(RStr::from_str(key)) {
      ROption::RSome(value) => Some(value),
      ROption::RNone => None,
    }
  }

  pub fn bool(&self, key: &str) -> Option<bool> {
    match self.ctx.host.option_bool(RStr::from_str(key)) {
      ROption::RSome(value) => Some(value),
      ROption::RNone => None,
    }
  }
}

#[derive(Clone)]
pub struct StyleRef<'a> {
  ctx: Context<'a>,
  classes: RString,
}

impl StyleRef<'_> {
  pub fn classes(&self) -> &str {
    &self.classes
  }

  pub fn number(&self, property: &str) -> Option<f32> {
    match self
      .ctx
      .host
      .style_number(RStr::from_str(&self.classes), RStr::from_str(property))
    {
      ROption::RSome(value) => Some(value),
      ROption::RNone => None,
    }
  }

  pub fn length(&self, property: &str) -> Option<f32> {
    self.number(property)
  }

  pub fn bool(&self, property: &str) -> Option<bool> {
    match self
      .ctx
      .host
      .style_bool(RStr::from_str(&self.classes), RStr::from_str(property))
    {
      ROption::RSome(value) => Some(value),
      ROption::RNone => None,
    }
  }

  pub fn color(&self, property: &str) -> Option<Color> {
    match self
      .ctx
      .host
      .style_color(RStr::from_str(&self.classes), RStr::from_str(property))
    {
      ROption::RSome(value) => Some(value),
      ROption::RNone => None,
    }
  }

  pub fn color_or(&self, property: &str, default: Color) -> Color {
    self.color(property).unwrap_or(default)
  }

  pub fn padding(&self, default: impl Into<[f32; 2]>) -> [f32; 2] {
    if let Some(x) = self.length("padding-x") {
      let y = self.length("padding-y").unwrap_or(x);
      return [y, x];
    }
    if let Some(p) = self.length("padding") {
      return [p, p];
    }
    default.into()
  }
}
