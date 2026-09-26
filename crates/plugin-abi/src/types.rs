use abi_stable::{
  StableAbi,
  std_types::{RBox, ROption, RString, RVec},
};

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, Default, PartialEq)]
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

  Some(Color {
    r: out[0] as f32 / 255.0,
    g: out[1] as f32 / 255.0,
    b: out[2] as f32 / 255.0,
    a: out[3] as f32 / 255.0,
  })
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, Default, PartialEq)]
pub struct Border {
  pub width: f32,
  pub radius: f32,
  pub color: Color,
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, Default, PartialEq)]
pub struct Style {
  pub background: ROption<Color>,
  pub border: ROption<Border>,
  pub text_color: ROption<Color>,
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, Default, PartialEq)]
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

  pub fn to_array(&self) -> [f32; 4] {
    [self.top, self.right, self.bottom, self.left]
  }
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
  #[default]
  Start,
  Center,
  End,
  Fill,
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, Default, PartialEq)]
pub enum Length {
  #[default]
  Shrink,
  Fill,
  Fixed(f32),
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq)]
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
      _ if let Some((name, value)) = name.split_once('/') => {
        let mut color = self.color(name)?;
        color.a = value.parse::<f32>().ok()?;
        return Some(color);
      }
      _ => return None,
    };
    Some(color)
  }
}

// TODO: Add custom
#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq)]
pub enum AnimationKind {
  None,
  Slide,
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Easing {
  Linear,
  #[default]
  Ease,
  EaseIn,
  EaseOut,
  EaseInOut,
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq)]
pub struct Animation {
  pub kind: AnimationKind,
  pub duration_ms: u32,
  pub easing: Easing,
  pub offset_x: f32,
  pub offset_y: f32,
  pub tween: bool,
}

impl Animation {
  pub const fn slide(offset: (f32, f32)) -> Self {
    Self {
      kind: AnimationKind::Slide,
      duration_ms: 130,
      easing: Easing::EaseOut,
      offset_x: offset.0,
      offset_y: offset.1,
      tween: false,
    }
  }

  pub const fn tween() -> Self {
    Self {
      kind: AnimationKind::None,
      duration_ms: 130,
      easing: Easing::Ease,
      offset_x: 0.0,
      offset_y: 0.0,
      tween: true,
    }
  }

  pub const fn duration(mut self, millis: u32) -> Self {
    self.duration_ms = millis;
    self
  }

  pub const fn easing(mut self, easing: Easing) -> Self {
    self.easing = easing;
    self
  }
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug)]
pub struct CanvasRef {
  pub ptr: *mut u8,
  pub len: usize,
  pub id: u64,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, PartialEq)]
pub enum Image {
  Rgba {
    cache_key: u64,
    width: u32,
    height: u32,
    pixels: RVec<u8>,
  },
  Encoded {
    cache_key: u64,
    bytes: RVec<u8>,
  },
  Path(RString),
}

impl Image {
  pub fn rgba(width: u32, height: u32, pixels: impl Into<Vec<u8>>) -> Image {
    Image::Rgba {
      cache_key: 0,
      width,
      height,
      pixels: pixels.into().into(),
    }
  }

  pub fn rgba_cached(cache_key: u64, width: u32, height: u32, pixels: impl Into<Vec<u8>>) -> Image {
    Image::Rgba {
      cache_key,
      width,
      height,
      pixels: pixels.into().into(),
    }
  }

  pub fn encoded(bytes: impl Into<Vec<u8>>) -> Image {
    Image::Encoded {
      cache_key: 0,
      bytes: bytes.into().into(),
    }
  }

  pub fn encoded_cached(cache_key: u64, bytes: impl Into<Vec<u8>>) -> Image {
    Image::Encoded {
      cache_key,
      bytes: bytes.into().into(),
    }
  }

  pub fn path(path: impl Into<String>) -> Image {
    Image::Path(path.into().into())
  }
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub enum Node {
  Row {
    children: RVec<Node>,
    spacing: f32,
    align: Align,
    classes: RVec<RString>,
  },
  Column {
    children: RVec<Node>,
    spacing: f32,
    align: Align,
    classes: RVec<RString>,
  },
  Container {
    child: RBox<Node>,
    padding: Padding,
    align_x: Align,
    align_y: Align,
    style: ROption<Style>,
    width: Length,
    height: Length,
    classes: RVec<RString>,
  },
  Clickable {
    child: RBox<Node>,
    action: ROption<RString>,
    effect: ROption<[usize; 4]>,
    classes: RVec<RString>,
  },
  Scrollable {
    child: RBox<Node>,
    width: Length,
    height: Length,
    report_scroll: bool,
    classes: RVec<RString>,
  },
  Icon {
    name: RString,
    size: f32,
    color: ROption<Color>,
    classes: RVec<RString>,
  },
  Text {
    content: RString,
    size: f32,
    color: ROption<Color>,
    classes: RVec<RString>,
  },
  Progress {
    value: f32,
    color: ROption<Color>,
    classes: RVec<RString>,
  },
  Image {
    image: Image,
    width: Length,
    height: Length,
    classes: RVec<RString>,
  },
  Canvas {
    cache_key: u64,
    pixel_width: u32,
    pixel_height: u32,
    width: Length,
    height: Length,
    pixels: RVec<u8>,
    pool_id: u64,
    classes: RVec<RString>,
  },
  TextInput {
    id: RString,
    value: RString,
    placeholder: RString,
    secure: bool,
    size: f32,
    color: ROption<Color>,
    width: Length,
    classes: RVec<RString>,
  },
  EventWrapper {
    child: RBox<Node>,
    id: RString,
    classes: RVec<RString>,
  },
  Animated {
    child: RBox<Node>,
    animation: Animation,
  },
}

impl Node {
  pub fn row(children: impl IntoIterator<Item = Node>) -> Node {
    Node::Row {
      children: children.into_iter().collect(),
      spacing: 4.0,
      align: Align::Start,
      classes: RVec::new(),
    }
  }

  pub fn column(children: impl IntoIterator<Item = Node>) -> Node {
    Node::Column {
      children: children.into_iter().collect(),
      spacing: 4.0,
      align: Align::Start,
      classes: RVec::new(),
    }
  }

  pub fn container(child: Node) -> Node {
    Node::Container {
      child: RBox::new(child),
      padding: Padding::ZERO,
      align_x: Align::Start,
      align_y: Align::Start,
      style: ROption::RNone,
      width: Length::Shrink,
      height: Length::Shrink,
      classes: RVec::new(),
    }
  }

  pub fn scrollable(child: Node) -> Node {
    Node::Scrollable {
      child: RBox::new(child),
      width: Length::Shrink,
      height: Length::Shrink,
      report_scroll: false,
      classes: RVec::new(),
    }
  }

  pub fn clickable(child: Node) -> Node {
    Node::Clickable {
      child: RBox::new(child),
      action: ROption::RNone,
      effect: ROption::RNone,
      classes: RVec::new(),
    }
  }

  pub fn icon(name: impl Into<RString>) -> Node {
    Node::Icon {
      name: name.into(),
      size: 16.0,
      color: ROption::RNone,
      classes: RVec::new(),
    }
  }

  pub fn text(content: impl Into<RString>) -> Node {
    Node::Text {
      content: content.into(),
      size: 12.0,
      color: ROption::RNone,
      classes: RVec::new(),
    }
  }

  pub fn text_input(id: impl Into<RString>, value: impl Into<RString>) -> Node {
    Node::TextInput {
      id: id.into(),
      value: value.into(),
      placeholder: RString::new(),
      secure: false,
      size: 12.0,
      color: ROption::RNone,
      width: Length::Fill,
      classes: RVec::new(),
    }
  }

  pub fn event_wrapper(child: Node, id: impl Into<RString>) -> Node {
    Node::EventWrapper {
      child: RBox::new(child),
      id: id.into(),
      classes: RVec::new(),
    }
  }

  pub fn progress(value: f32) -> Node {
    Node::Progress {
      value,
      color: ROption::RNone,
      classes: RVec::new(),
    }
  }

  pub fn image(image: Image) -> Node {
    Node::Image {
      image,
      width: Length::Shrink,
      height: Length::Shrink,
      classes: RVec::new(),
    }
  }

  pub fn canvas(width: u32, height: u32, draw: impl FnOnce(&mut [u8])) -> Node {
    Self::canvas_inner(0, width, height, draw)
  }

  pub fn canvas_cached(
    cache_key: u64,
    width: u32,
    height: u32,
    draw: impl FnOnce(&mut [u8]),
  ) -> Node {
    Self::canvas_inner(cache_key, width, height, draw)
  }

  fn canvas_inner(cache_key: u64, width: u32, height: u32, draw: impl FnOnce(&mut [u8])) -> Node {
    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    draw(&mut pixels);
    Node::Canvas {
      cache_key,
      pixel_width: width,
      pixel_height: height,
      width: Length::Shrink,
      height: Length::Shrink,
      pixels: pixels.into(),
      pool_id: 0,
      classes: RVec::new(),
    }
  }

  pub fn scroll_events(mut self) -> Node {
    if let Node::Scrollable { report_scroll, .. } = &mut self {
      *report_scroll = true;
    }
    self
  }

  pub fn placeholder(mut self, placeholder: impl Into<RString>) -> Node {
    let placeholder = placeholder.into();
    if let Node::TextInput {
      placeholder: target,
      ..
    } = &mut self
    {
      *target = placeholder;
    }
    self
  }

  pub fn secure(mut self, secure: bool) -> Node {
    if let Node::TextInput { secure: target, .. } = &mut self {
      *target = secure;
    }
    self
  }

  pub fn value(mut self, value: impl Into<RString>) -> Node {
    let value = value.into();
    if let Node::TextInput { value: target, .. } = &mut self {
      *target = value;
    }
    self
  }

  pub fn classes(mut self, classes: &[&str]) -> Node {
    let owned: RVec<RString> = classes.iter().map(|c| RString::from(*c)).collect();
    match &mut self {
      Node::Row { classes: c, .. }
      | Node::Column { classes: c, .. }
      | Node::Container { classes: c, .. }
      | Node::Clickable { classes: c, .. }
      | Node::Scrollable { classes: c, .. }
      | Node::Icon { classes: c, .. }
      | Node::Text { classes: c, .. }
      | Node::Progress { classes: c, .. }
      | Node::Image { classes: c, .. }
      | Node::Canvas { classes: c, .. }
      | Node::TextInput { classes: c, .. }
      | Node::EventWrapper { classes: c, .. } => *c = owned,
      Node::Animated { child, .. } => {
        let child = std::mem::replace(child, RBox::new(Node::text(RString::new())));
        return RBox::into_inner(child).classes(classes);
      }
    }
    self
  }

  pub fn own_classes(&self) -> RVec<RString> {
    match self {
      Node::Row { classes, .. }
      | Node::Column { classes, .. }
      | Node::Container { classes, .. }
      | Node::Clickable { classes, .. }
      | Node::Scrollable { classes, .. }
      | Node::Icon { classes, .. }
      | Node::Text { classes, .. }
      | Node::Progress { classes, .. }
      | Node::Image { classes, .. }
      | Node::Canvas { classes, .. }
      | Node::TextInput { classes, .. }
      | Node::EventWrapper { classes, .. } => classes.clone(),
      Node::Animated { child, .. } => child.own_classes(),
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
      Node::Icon { size: target, .. }
      | Node::Text { size: target, .. }
      | Node::TextInput { size: target, .. } => *target = size,
      _ => {}
    }
    self
  }

  pub fn color(mut self, color: Color) -> Node {
    match &mut self {
      Node::Icon { color: target, .. }
      | Node::Text { color: target, .. }
      | Node::TextInput { color: target, .. }
      | Node::Progress { color: target, .. } => *target = ROption::RSome(color),
      _ => {}
    }
    self
  }

  pub fn width(mut self, width: Length) -> Node {
    match &mut self {
      Node::Container { width: target, .. }
      | Node::Scrollable { width: target, .. }
      | Node::Image { width: target, .. }
      | Node::TextInput { width: target, .. }
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

  pub fn action(self, command: impl Into<RString>) -> Node {
    match self {
      Node::Clickable {
        child,
        effect,
        action: _,
        classes,
      } => Node::Clickable {
        child,
        action: ROption::RSome(command.into()),
        effect,
        classes,
      },
      other => {
        let classes = other.own_classes();
        Node::Clickable {
          child: RBox::new(other),
          action: ROption::RSome(command.into()),
          effect: ROption::RNone,
          classes,
        }
      }
    }
  }

  pub fn effect(self, value: [usize; 4]) -> Node {
    match self {
      Node::Clickable {
        child,
        action,
        effect: _,
        classes,
      } => Node::Clickable {
        child,
        action,
        effect: ROption::RSome(value),
        classes,
      },
      other => {
        let classes = other.own_classes();
        Node::Clickable {
          child: RBox::new(other),
          action: ROption::RNone,
          effect: ROption::RSome(value),
          classes,
        }
      }
    }
  }

  pub fn animated(self, animation: Animation) -> Node {
    Node::Animated {
      child: RBox::new(self),
      animation,
    }
  }

  pub fn slide_in(self, offset: (f32, f32)) -> Node {
    self.animated(Animation::slide(offset))
  }

  pub fn tween(self) -> Node {
    self.animated(Animation::tween())
  }
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, Default, PartialEq)]
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
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layer {
  Background,
  Bottom,
  #[default]
  Top,
  Overlay,
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Anchor(pub u32);

impl Anchor {
  pub const TOP: Self = Self(1);
  pub const BOTTOM: Self = Self(2);
  pub const LEFT: Self = Self(4);
  pub const RIGHT: Self = Self(8);
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

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyboardInteractivity {
  #[default]
  None,
  OnDemand,
  Exclusive,
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Visibility {
  #[default]
  Visible,
  Transient,
  Toggleable(bool),
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UpdateWhen {
  #[default]
  OnDemand,
  EveryFrame,
  OnEvent,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, PartialEq)]
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
  pub monitor: RString,
  pub namespace: RString,
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
      monitor: RString::new(),
      namespace: RString::new(),
      visibility: Visibility::Visible,
      update_when: UpdateWhen::OnDemand,
    }
  }
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq)]
pub struct Monitor {
  pub name: RString,
  pub width: u32,
  pub height: u32,
  pub scale: f32,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq, Eq)]
pub struct Workspace {
  pub id: u64,
  pub idx: u8,
  pub name: ROption<RString>,
  pub output: ROption<RString>,
  pub is_active: bool,
  pub is_focused: bool,
  pub is_urgent: bool,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq, Eq)]
pub struct Window {
  pub id: u64,
  pub title: RString,
  pub class: RString,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq)]
pub struct CompositorState {
  pub monitors: RVec<Monitor>,
  pub workspaces: RVec<Workspace>,
  pub active_window: ROption<Window>,
  pub overview_active: bool,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq)]
pub struct AudioSink {
  pub id: u32,
  pub name: RString,
  pub description: RString,
  pub volume: f32,
  pub muted: bool,
  pub is_default: bool,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq)]
pub struct MprisPlayer {
  pub identity: RString,
  pub title: RString,
  pub artist: RString,
  pub album: RString,
  pub art_url: ROption<RString>,
  pub playback_status: RString,
  pub can_play_pause: bool,
  pub can_go_next: bool,
  pub can_go_previous: bool,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq)]
pub struct AudioState {
  pub volume: f32,
  pub muted: bool,
  pub default_sink: ROption<RString>,
  pub sinks: RVec<AudioSink>,
  pub player: ROption<MprisPlayer>,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq)]
pub struct BluetoothDevice {
  pub address: RString,
  pub name: RString,
  pub icon: ROption<RString>,
  pub paired: bool,
  pub connected: bool,
  pub trusted: bool,
  pub battery: ROption<u8>,
  pub rssi: ROption<i16>,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq)]
pub struct BluetoothState {
  pub powered: bool,
  pub discovering: bool,
  pub adapter_name: ROption<RString>,
  pub devices: RVec<BluetoothDevice>,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq, Eq)]
pub struct AccessPoint {
  pub ssid: RString,
  pub signal: u8,
  pub secured: bool,
  pub in_use: bool,
  pub saved: bool,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq, Eq)]
pub struct Ethernet {
  pub iface: RString,
  pub speed: u32,
  pub carrier: bool,
  pub connected: bool,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq, Eq)]
pub struct ConnectionInfo {
  pub label: RString,
  pub mac: RString,
  pub is_wifi: bool,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq)]
pub struct NetworkState {
  pub wifi_enabled: bool,
  pub wifi_hardware_enabled: bool,
  pub scanning: bool,
  pub busy: bool,
  pub wifi_connected: ROption<AccessPoint>,
  pub ethernet: ROption<Ethernet>,
  pub networks: RVec<AccessPoint>,
  pub connected: ROption<ConnectionInfo>,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq)]
pub struct ProcessInfo {
  pub pid: u32,
  pub name: RString,
  pub cpu_usage: f32,
  pub memory: u64,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq)]
pub struct SystemState {
  pub cpu_usage: f32,
  pub mem_usage: f32,
  pub mem_total: u64,
  pub mem_used: u64,
  pub temperature: ROption<f32>,
  pub load: [f32; 3],
  pub network_state: u16,
  pub network_rx: u64,
  pub network_tx: u64,
  pub processes: RVec<ProcessInfo>,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayPixmap {
  pub width: u32,
  pub height: u32,
  pub data: RVec<u8>,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayItem {
  pub address: RString,
  pub title: RString,
  pub icon_name: ROption<RString>,
  pub menu_path: ROption<RString>,
  pub pixmap: ROption<TrayPixmap>,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayState {
  pub items: RVec<TrayItem>,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq)]
pub struct PowerState {
  pub percent: ROption<u8>,
  pub charging: bool,
  pub status: RString,
  pub health: ROption<u8>,
  pub energy_now_wh: ROption<f32>,
  pub energy_full_wh: ROption<f32>,
  pub power_w: ROption<f32>,
  pub time_remaining: ROption<RString>,
  pub active_profile: ROption<RString>,
  pub available_profiles: RVec<RString>,
  pub brightness_percent: u8,
  pub brightness_max: u32,
  pub brightness_current: u32,
  pub device_name: RString,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq, Eq)]
pub struct NotificationAction {
  pub label: RString,
  pub command: RString,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, PartialEq, Eq)]
pub struct Notification {
  pub summary: RString,
  pub body: RString,
  pub app_name: ROption<RString>,
  pub app_icon: ROption<RString>,
  pub urgency: u8,
  pub timeout_ms: i32,
  pub actions: RVec<NotificationAction>,
}

impl Default for Notification {
  fn default() -> Self {
    Self {
      summary: RString::new(),
      body: RString::new(),
      app_name: ROption::RNone,
      app_icon: ROption::RNone,
      urgency: 1,
      timeout_ms: -1,
      actions: RVec::new(),
    }
  }
}

impl Notification {
  pub fn new(summary: impl Into<RString>, body: impl Into<RString>) -> Self {
    Self {
      summary: summary.into(),
      body: body.into(),
      ..Default::default()
    }
  }

  pub fn app_name(mut self, app_name: impl Into<RString>) -> Self {
    self.app_name = ROption::RSome(app_name.into());
    self
  }

  pub fn icon(mut self, icon: impl Into<RString>) -> Self {
    self.app_icon = ROption::RSome(icon.into());
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

  pub fn action(mut self, label: impl Into<RString>, command: impl Into<RString>) -> Self {
    self.actions.push(NotificationAction {
      label: label.into(),
      command: command.into(),
    });
    self
  }
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq, Eq)]
pub struct SpotlightResult {
  pub icon: ROption<RString>,
  pub title: RString,
  pub subtitle: ROption<RString>,
  pub action_label: ROption<RString>,
  pub image: ROption<RString>,
  pub tags: RVec<RString>,
}

pub mod display_style {
  pub const LIST: u32 = 1;
  pub const GRID: u32 = 2;
  pub const IMAGE_LIST: u32 = 4;
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayStyle {
  List,
  Grid,
  ImageList,
}

impl DisplayStyle {
  pub const fn bit(self) -> u32 {
    match self {
      Self::List => display_style::LIST,
      Self::Grid => display_style::GRID,
      Self::ImageList => display_style::IMAGE_LIST,
    }
  }
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq, Eq)]
pub enum TileAction {
  #[default]
  None,
  Command(RString),
  Commands(RVec<RString>),
  Menu(RString),
  Expand,
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug, Default, PartialEq, Eq)]
pub struct QuickSettingsTile {
  pub id: RString,
  pub icon: RString,
  pub title: RString,
  pub status: RString,
  pub detail: RString,
  pub active: bool,
  pub accent: RString,
  pub on_click: TileAction,
  pub order: i32,
}

impl QuickSettingsTile {
  pub fn new(id: impl Into<RString>, icon: impl Into<RString>, title: impl Into<RString>) -> Self {
    Self {
      id: id.into(),
      icon: icon.into(),
      title: title.into(),
      order: 100,
      ..Default::default()
    }
  }

  pub fn status(mut self, status: impl Into<RString>) -> Self {
    self.status = status.into();
    self
  }

  pub fn active(mut self, active: bool) -> Self {
    self.active = active;
    self
  }

  pub fn action(mut self, command: impl Into<RString>) -> Self {
    self.on_click = TileAction::Command(command.into());
    self
  }

  pub fn commands<I, S>(mut self, commands: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: Into<RString>,
  {
    self.on_click = TileAction::Commands(commands.into_iter().map(Into::into).collect());
    self
  }

  pub fn menu(mut self, menu: impl Into<RString>) -> Self {
    self.on_click = TileAction::Menu(menu.into());
    self
  }

  pub fn expandable(mut self) -> Self {
    self.on_click = TileAction::Expand;
    self
  }

  pub fn accent(mut self, key: impl Into<RString>) -> Self {
    self.accent = key.into();
    self
  }

  pub fn order(mut self, order: i32) -> Self {
    self.order = order;
    self
  }
}
