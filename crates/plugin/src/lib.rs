#![allow(non_camel_case_types)]

use core::ffi::c_void;

pub const SL_PLUGIN_ABI_VERSION: u32 = 1;

pub const SL_PLUGIN_INIT_SYMBOL: &str = "slowshell_plugin_init";
pub const SL_PLUGIN_META_SYMBOL: &str = "slowshell_plugin_meta";
pub const SL_PLUGIN_SHUTDOWN_SYMBOL: &str = "slowshell_plugin_shutdown";

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SlStr {
  pub ptr: *const u8,
  pub len: usize,
}

impl Default for SlStr {
  fn default() -> Self {
    Self::EMPTY
  }
}

impl SlStr {
  pub const EMPTY: SlStr = SlStr {
    ptr: core::ptr::null(),
    len: 0,
  };

  pub const fn from_str(s: &str) -> Self {
    Self {
      ptr: s.as_ptr(),
      len: s.len(),
    }
  }

  pub fn is_empty(&self) -> bool {
    self.ptr.is_null() || self.len == 0
  }

  pub unsafe fn as_str(&self) -> Option<&str> {
    if self.ptr.is_null() {
      return None;
    }

    unsafe { core::str::from_utf8(core::slice::from_raw_parts(self.ptr, self.len)).ok() }
  }

  pub unsafe fn as_bytes(&self) -> Option<&[u8]> {
    if self.ptr.is_null() {
      return None;
    }

    Some(unsafe { core::slice::from_raw_parts(self.ptr, self.len) })
  }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlColor {
  pub r: f32,
  pub g: f32,
  pub b: f32,
  pub a: f32,
}

impl SlColor {
  pub const TRANSPARENT: SlColor = SlColor {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
  };
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlBorder {
  pub width: f32,
  pub radius: f32,
  pub color: SlColor,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlStyle {
  pub background: SlColor,
  pub has_background: bool,
  pub border: SlBorder,
  pub has_border: bool,
  pub text_color: SlColor,
  pub has_text_color: bool,
}

pub mod sl_node_kind {
  pub const ROW: u32 = 0;
  pub const COLUMN: u32 = 1;
  pub const ICON: u32 = 2;
  pub const TEXT: u32 = 3;
  pub const PROGRESS: u32 = 4;
  pub const CANVAS: u32 = 5;
  pub const CONTAINER: u32 = 6;
  pub const IMAGE: u32 = 7;
  pub const SCROLLABLE: u32 = 8;
}

pub mod sl_align {
  pub const START: u32 = 0;
  pub const CENTER: u32 = 1;
  pub const END: u32 = 2;
  pub const FILL: u32 = 3;
}

pub mod sl_length_unit {
  pub const SHRINK: u32 = 0;
  pub const FILL: u32 = 1;
  pub const FIXED: u32 = 2;
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlLength {
  pub unit: u32,
  pub value: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SlCanvas {
  pub width: u32,
  pub height: u32,
  pub stride: u32,
  pub data: *mut u8,
  pub cache_key: u64,
}

impl Default for SlCanvas {
  fn default() -> Self {
    Self {
      width: 0,
      height: 0,
      stride: 0,
      data: core::ptr::null_mut(),
      cache_key: 0,
    }
  }
}

pub mod sl_image_source {
  pub const RAW_RGBA: u32 = 0;
  pub const ENCODED: u32 = 1;
  pub const PATH: u32 = 2;
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlImage {
  pub source: u32,
  pub data: SlStr,
  pub width: u32,
  pub height: u32,
  pub cache_key: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SlNode {
  pub kind: u32,
  pub style: SlStyle,
  pub padding: [f32; 4],
  pub spacing: f32,
  pub align_x: u32,
  pub align_y: u32,
  pub children: *const SlNode,
  pub child_count: usize,
  pub text: SlStr,
  pub size: f32,
  pub value: f32,
  pub canvas: SlCanvas,
  pub image: SlImage,
  pub action: SlStr,
  pub width: SlLength,
  pub height: SlLength,
  pub has_effect: bool,
  pub effect: [usize; 4],
}

impl Default for SlNode {
  fn default() -> Self {
    Self {
      kind: sl_node_kind::ROW,
      style: SlStyle::default(),
      padding: [0.0; 4],
      spacing: 0.0,
      align_x: sl_align::START,
      align_y: sl_align::START,
      children: core::ptr::null(),
      child_count: 0,
      text: SlStr::EMPTY,
      size: 0.0,
      value: 0.0,
      canvas: SlCanvas::default(),
      image: SlImage::default(),
      action: SlStr::EMPTY,
      width: SlLength::default(),
      height: SlLength::default(),
      has_effect: false,
      effect: [0; 4],
    }
  }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SlNodeList {
  pub nodes: *const SlNode,
  pub len: usize,
}

impl SlNodeList {
  pub const EMPTY: SlNodeList = SlNodeList {
    nodes: core::ptr::null(),
    len: 0,
  };
}

pub mod sl_event_kind {
  pub const NONE: u32 = 0;
  pub const TICK: u32 = 1;
  pub const CONFIG_RELOAD: u32 = 2;
  pub const COMPOSITOR_UPDATE: u32 = 3;
  pub const FD: u32 = 4;
  pub const FRAME: u32 = 5;
}

pub mod sl_event_mask {
  pub const TICK: u32 = 1 << 0;
  pub const CONFIG_RELOAD: u32 = 1 << 1;
  pub const COMPOSITOR_UPDATE: u32 = 1 << 2;
  pub const FD: u32 = 1 << 3;
  pub const FRAME: u32 = 1 << 4;
}

pub mod sl_effect {
  pub const NONE: u32 = 0;
  pub const REDRAW: u32 = 1;
  pub const SUBSCRIBE: u32 = 2;
  pub const HIDE: u32 = 3;
  pub const SHOW: u32 = 4;
  pub const REALLY_HIDE: u32 = 5;
  pub const DESTROY: u32 = 6;
  pub const REALLY_DESTROY: u32 = 7;
  pub const CUSTOM: u32 = 8;
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlEffect {
  pub code: u32,
  pub custom: [usize; 4],
}

pub mod sl_message_kind {
  pub const NOOP: u32 = 0;
  pub const EFFECT: u32 = 1;
  pub const ACTION: u32 = 2;
  pub const EFFECT_ACTION: u32 = 3;
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlItemMessage {
  pub kind: u32,
  pub effect: u32,
  pub custom: [usize; 4],
  pub name: SlStr,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SlEvent {
  pub kind: u32,
  pub name: SlStr,
  pub bag: *mut c_void,
  pub action: u32,
  pub fd: i32,
}

impl Default for SlEvent {
  fn default() -> Self {
    Self {
      kind: sl_event_kind::NONE,
      name: SlStr::EMPTY,
      bag: core::ptr::null_mut(),
      action: 0,
      fd: -1,
    }
  }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlMonitor {
  pub name: SlStr,
  pub width: u32,
  pub height: u32,
  pub scale: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SlWorkspace {
  pub id: u64,
  pub idx: u8,
  pub output: SlStr,
  pub name: SlStr,
  pub is_active: bool,
  pub is_focused: bool,
  pub is_urgent: bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SlWindow {
  pub title: SlStr,
  pub wclass: SlStr,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlCompositorState {
  pub monitors: *const SlMonitor,
  pub monitor_count: usize,
  pub workspaces: *const SlWorkspace,
  pub workspace_count: usize,
  pub active_window: *const SlWindow,
  pub overview_active: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlNotificationAction {
  pub command: SlStr,
  pub label: SlStr,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlNotification {
  pub summary: SlStr,
  pub body: SlStr,
  pub app_name: SlStr,
  pub app_icon: SlStr,
  pub urgency: u8,
  pub timeout_ms: i32,
  pub replaces_id: u32,
  pub actions: *const SlNotificationAction,
  pub action_count: usize,
}

pub type SlComponentCreateFn = unsafe extern "C" fn(ctx: *mut c_void) -> *mut c_void;
pub type SlComponentDestroyFn = unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void);
pub type SlComponentEventsFn = unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void) -> u32;
pub type SlComponentWatchFn =
  unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void, opts: *mut c_void);
pub type SlComponentUpdateFn =
  unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void, event: *const SlEvent) -> SlEffect;
pub type SlComponentViewFn = unsafe extern "C" fn(
  ctx: *mut c_void,
  state: *mut c_void,
  opts: *mut c_void,
  out: *mut SlNodeList,
);

pub type SlComponentCheckViewFn =
  unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void, opts: *mut c_void) -> bool;
pub type SlComponentStopFn = unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SlComponentVtable {
  pub size: u32,
  pub create: Option<SlComponentCreateFn>,
  pub destroy: Option<SlComponentDestroyFn>,
  pub events: Option<SlComponentEventsFn>,
  pub watch: Option<SlComponentWatchFn>,
  pub update: Option<SlComponentUpdateFn>,
  pub view: Option<SlComponentViewFn>,
  pub check_view: Option<SlComponentCheckViewFn>,
  pub stop: Option<SlComponentStopFn>,
}

pub type SlCompositorCreateFn = unsafe extern "C" fn(ctx: *mut c_void) -> *mut c_void;
pub type SlCompositorDestroyFn = unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void);
pub type SlCompositorIsActiveFn =
  unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void) -> bool;
pub type SlCompositorInitializeFn = unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void);
pub type SlCompositorUpdateFn = unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void);
pub type SlCompositorCommandFn =
  unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void, command: u32, arg: i32);

pub type SlCompositorRegisterFdFn =
  unsafe extern "C" fn(ctx: *mut c_void, fd: i32, flags: u32) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SlCompositorVtable {
  pub size: u32,
  pub create: Option<SlCompositorCreateFn>,
  pub destroy: Option<SlCompositorDestroyFn>,
  pub is_active: Option<SlCompositorIsActiveFn>,
  pub initialize: Option<SlCompositorInitializeFn>,
  pub update_state: Option<SlCompositorUpdateFn>,
  pub send_command: Option<SlCompositorCommandFn>,
}

pub mod sl_compositor_command {
  pub const FOCUS_WORKSPACE: u32 = 0;
}

pub mod sl_epoll {
  pub const IN: u32 = 1 << 0;
  pub const ET: u32 = 1 << 1;
}

pub type SlPayloadCreateFn = unsafe extern "C" fn(ctx: *mut c_void) -> *mut c_void;
pub type SlPayloadDestroyFn = unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void);
pub type SlPayloadInvokeFn =
  unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void, args: *mut c_void);

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlPayloadArgs {
  pub count: usize,
  pub keys: *const SlStr,
  pub values: *const SlStr,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SlPayloadVtable {
  pub size: u32,
  pub create: Option<SlPayloadCreateFn>,
  pub destroy: Option<SlPayloadDestroyFn>,
  pub invoke: Option<SlPayloadInvokeFn>,
}

pub type SlRenderableCreateFn = unsafe extern "C" fn(ctx: *mut c_void) -> *mut c_void;
pub type SlRenderableDestroyFn = unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void);
pub type SlRenderableViewFn = unsafe extern "C" fn(
  ctx: *mut c_void,
  state: *mut c_void,
  opts: *mut c_void,
  out: *mut SlNodeList,
);

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlRenderableSettings {
  pub wrap_popup: bool,
  pub width: f32,
  pub height: f32,
}

pub type SlRenderableInitializeFn = unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void);
pub type SlRenderableUpdateFn =
  unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void) -> SlEffect;
pub type SlRenderableHandleMessageFn = unsafe extern "C" fn(
  ctx: *mut c_void,
  state: *mut c_void,
  message: *const SlItemMessage,
) -> SlEffect;
pub type SlRenderableSettingsFn =
  unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void, out: *mut SlRenderableSettings);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SlRenderableVtable {
  pub size: u32,
  pub create: Option<SlRenderableCreateFn>,
  pub destroy: Option<SlRenderableDestroyFn>,
  pub view: Option<SlRenderableViewFn>,
  pub settings: Option<SlRenderableSettingsFn>,
  pub initialize: Option<SlRenderableInitializeFn>,
  pub update: Option<SlRenderableUpdateFn>,
  pub handle_message: Option<SlRenderableHandleMessageFn>,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlSpotlightItem {
  pub icon: SlStr,
  pub title: SlStr,
  pub subtitle: SlStr,
  pub action_label: SlStr,
  pub image: SlStr,
  pub tags: *const SlStr,
  pub tag_count: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlSpotlightList {
  pub items: *const SlSpotlightItem,
  pub count: usize,
}

pub type SlSpotlightCreateFn = unsafe extern "C" fn(ctx: *mut c_void) -> *mut c_void;
pub type SlSpotlightDestroyFn = unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void);

pub type SlSpotlightGenerateFn = unsafe extern "C" fn(
  ctx: *mut c_void,
  state: *mut c_void,
  query: SlStr,
  out: *mut SlSpotlightList,
) -> i32;
pub type SlSpotlightActivateFn =
  unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void, index: usize);
pub type SlSpotlightTriggerCheckFn =
  unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void, query: SlStr) -> bool;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SlSpotlightVtable {
  pub size: u32,
  pub create: Option<SlSpotlightCreateFn>,
  pub destroy: Option<SlSpotlightDestroyFn>,
  pub generate: Option<SlSpotlightGenerateFn>,
  pub activate: Option<SlSpotlightActivateFn>,
  pub trigger_check: Option<SlSpotlightTriggerCheckFn>,
  pub allows_triggers: bool,
  pub display_styles: u32,
}

pub mod sl_display_style {
  pub const LIST: u32 = 1;
  pub const GRID: u32 = 2;
  pub const IMAGE_LIST: u32 = 4;
}

pub type SlRegistryNotifyFn =
  unsafe extern "C" fn(ctx: *mut c_void, key: usize, value: *mut c_void);
pub type SlRegistryRegisterFn =
  unsafe extern "C" fn(ctx: *mut c_void, key: usize, value: *mut c_void) -> i32;
pub type SlRegistryGetFn = unsafe extern "C" fn(ctx: *mut c_void, key: usize) -> *mut c_void;
pub type SlRegistryRemoveFn = unsafe extern "C" fn(ctx: *mut c_void, key: usize) -> *mut c_void;
pub type SlRegistrySubscribeFn =
  unsafe extern "C" fn(ctx: *mut c_void, key: usize, callback: SlRegistryNotifyFn) -> i32;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlAudioSink {
  pub id: u32,
  pub name: SlStr,
  pub description: SlStr,
  pub volume: f32,
  pub muted: bool,
  pub is_default: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlMprisPlayer {
  pub identity: SlStr,
  pub title: SlStr,
  pub artist: SlStr,
  pub album: SlStr,
  pub art_url: SlStr,
  pub playback_status: SlStr,
  pub can_play_pause: bool,
  pub can_go_next: bool,
  pub can_go_previous: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlAudioState {
  pub volume: f32,
  pub muted: bool,
  pub default_sink: SlStr,
  pub sinks: *const SlAudioSink,
  pub sink_count: usize,
  pub has_player: bool,
  pub player: SlMprisPlayer,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlBluetoothDevice {
  pub address: SlStr,
  pub name: SlStr,
  pub icon: SlStr,
  pub paired: bool,
  pub connected: bool,
  pub trusted: bool,
  pub battery: i32,
  pub rssi: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlBluetoothState {
  pub powered: bool,
  pub discovering: bool,
  pub adapter_name: SlStr,
  pub devices: *const SlBluetoothDevice,
  pub device_count: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlAccessPoint {
  pub ssid: SlStr,
  pub signal: u8,
  pub secured: bool,
  pub in_use: bool,
  pub saved: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlEthernet {
  pub iface: SlStr,
  pub speed: u32,
  pub carrier: bool,
  pub connected: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlConnectionInfo {
  pub label: SlStr,
  pub mac: SlStr,
  pub is_wifi: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlNetworkState {
  pub wifi_enabled: bool,
  pub wifi_hardware_enabled: bool,
  pub scanning: bool,
  pub busy: bool,
  pub has_wifi_connected: bool,
  pub wifi_connected: SlAccessPoint,
  pub has_ethernet: bool,
  pub ethernet: SlEthernet,
  pub networks: *const SlAccessPoint,
  pub network_count: usize,
  pub has_connected: bool,
  pub connected: SlConnectionInfo,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlProcessInfo {
  pub pid: u32,
  pub name: SlStr,
  pub cpu_usage: f32,
  pub memory: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlSystemState {
  pub cpu_usage: f32,
  pub mem_usage: f32,
  pub mem_total: u64,
  pub mem_used: u64,
  pub has_temperature: bool,
  pub temperature: f32,
  pub load: [f32; 3],
  pub network_state: u16,
  pub network_rx: u64,
  pub network_tx: u64,
  pub top_processes: *const SlProcessInfo,
  pub process_count: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlTrayItem {
  pub address: SlStr,
  pub title: SlStr,
  pub icon_name: SlStr,
  pub menu_path: SlStr,
  pub has_pixmap: bool,
  pub pixmap_width: u32,
  pub pixmap_height: u32,
  pub pixmap: SlStr,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlTrayState {
  pub items: *const SlTrayItem,
  pub item_count: usize,
}

pub type SlAudioStateGetFn = unsafe extern "C" fn(ctx: *mut c_void, out: *mut SlAudioState) -> i32;
pub type SlBluetoothStateGetFn =
  unsafe extern "C" fn(ctx: *mut c_void, out: *mut SlBluetoothState) -> i32;
pub type SlNetworkStateGetFn =
  unsafe extern "C" fn(ctx: *mut c_void, out: *mut SlNetworkState) -> i32;
pub type SlSystemStateGetFn =
  unsafe extern "C" fn(ctx: *mut c_void, out: *mut SlSystemState) -> i32;
pub type SlTrayStateGetFn = unsafe extern "C" fn(ctx: *mut c_void, out: *mut SlTrayState) -> i32;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlPowerState {
  pub has_percent: bool,
  pub percent: u8,
  pub charging: bool,
  pub status: SlStr,
  pub has_health: bool,
  pub health: u8,
  pub has_energy_now: bool,
  pub energy_now_wh: f32,
  pub has_energy_full: bool,
  pub energy_full_wh: f32,
  pub has_power: bool,
  pub power_w: f32,
  pub time_remaining: SlStr,
  pub has_active_profile: bool,
  pub active_profile: SlStr,
  pub available_profiles: *const SlStr,
  pub available_profile_count: usize,
  pub brightness_percent: u8,
  pub brightness_max: u32,
  pub brightness_current: u32,
  pub device_name: SlStr,
}

pub type SlPowerStateGetFn = unsafe extern "C" fn(ctx: *mut c_void, out: *mut SlPowerState) -> i32;

pub type SlDispatchFn = unsafe extern "C" fn(ctx: *mut c_void, command: SlStr) -> i32;

pub mod sl_anchor {
  pub const TOP: u32 = 1;
  pub const BOTTOM: u32 = 2;
  pub const LEFT: u32 = 4;
  pub const RIGHT: u32 = 8;
}

pub mod sl_layer {
  pub const BACKGROUND: u32 = 0;
  pub const BOTTOM: u32 = 1;
  pub const TOP: u32 = 2;
  pub const OVERLAY: u32 = 3;
}

pub mod sl_keyboard_interactivity {
  pub const NONE: u32 = 0;
  pub const ON_DEMAND: u32 = 1;
  pub const EXCLUSIVE: u32 = 2;
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlDesktopSettings {
  pub layer: u32,
  pub anchor: u32,
  pub width: u32,
  pub height: u32,
  pub margin: [i32; 4],
  pub exclusive_zone: i32,
  pub events_transparent: bool,
  pub keyboard_interactivity: u32,
  pub per_monitor: bool,
  pub monitor: SlStr,
  pub ns: SlStr,
  pub visibility: u32,
  pub visible: bool,
  // useless
  pub update_when: u32,
}

pub mod sl_visibility {
  pub const VISIBLE: u32 = 0;
  pub const TRANSIENT: u32 = 1;
  pub const TOGGLEABLE: u32 = 2;
}

pub mod sl_update_when {
  pub const ON_DEMAND: u32 = 0;
  pub const EVERY_FRAME: u32 = 1;
  pub const ON_EVENT: u32 = 2;
}

pub type SlDesktopCreateFn = unsafe extern "C" fn(ctx: *mut c_void) -> *mut c_void;
pub type SlDesktopDestroyFn = unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void);
pub type SlDesktopSettingsFn =
  unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void, out: *mut SlDesktopSettings);
pub type SlDesktopInitializeFn =
  unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void, opts: *mut c_void);
pub type SlDesktopEventsFn = unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void) -> u32;
pub type SlDesktopUpdateFn =
  unsafe extern "C" fn(ctx: *mut c_void, state: *mut c_void, event: *const SlEvent) -> SlEffect;
pub type SlDesktopHandleMessageFn = unsafe extern "C" fn(
  ctx: *mut c_void,
  state: *mut c_void,
  message: *const SlItemMessage,
) -> SlEffect;
pub type SlDesktopViewFn = unsafe extern "C" fn(
  ctx: *mut c_void,
  state: *mut c_void,
  opts: *mut c_void,
  out: *mut SlNodeList,
);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SlDesktopItemVtable {
  pub size: u32,
  pub create: Option<SlDesktopCreateFn>,
  pub destroy: Option<SlDesktopDestroyFn>,
  pub settings: Option<SlDesktopSettingsFn>,
  pub initialize: Option<SlDesktopInitializeFn>,
  pub events: Option<SlDesktopEventsFn>,
  pub update: Option<SlDesktopUpdateFn>,
  pub view: Option<SlDesktopViewFn>,
  pub handle_message: Option<SlDesktopHandleMessageFn>,
}

pub type SlRegisterComponentFn = unsafe extern "C" fn(
  host: *mut c_void,
  name: SlStr,
  vtable: *const SlComponentVtable,
  userdata: *mut c_void,
);
pub type SlRegisterCompositorFn = unsafe extern "C" fn(
  host: *mut c_void,
  name: SlStr,
  vtable: *const SlCompositorVtable,
  userdata: *mut c_void,
);
pub type SlRegisterPayloadFn = unsafe extern "C" fn(
  host: *mut c_void,
  command: SlStr,
  vtable: *const SlPayloadVtable,
  userdata: *mut c_void,
);
pub type SlRegisterRenderableFn = unsafe extern "C" fn(
  host: *mut c_void,
  name: SlStr,
  vtable: *const SlRenderableVtable,
  userdata: *mut c_void,
);
pub type SlRegisterSpotlightFn = unsafe extern "C" fn(
  host: *mut c_void,
  name: SlStr,
  vtable: *const SlSpotlightVtable,
  userdata: *mut c_void,
);
pub type SlRegisterDesktopItemFn = unsafe extern "C" fn(
  host: *mut c_void,
  name: SlStr,
  vtable: *const SlDesktopItemVtable,
  userdata: *mut c_void,
);
pub type SlRequestRedrawFn = unsafe extern "C" fn(ctx: *mut c_void, window: u64);
pub type SlRegisterFdFn = unsafe extern "C" fn(ctx: *mut c_void, fd: i32, action: u32) -> i32;
pub type SlSetIntervalFn =
  unsafe extern "C" fn(ctx: *mut c_void, millis: u32, repeating: bool, tag: u32) -> i32;
pub type SlCanvasAllocFn =
  unsafe extern "C" fn(ctx: *mut c_void, width: u32, height: u32) -> *mut u8;
pub type SlLogFn = unsafe extern "C" fn(ctx: *mut c_void, level: i32, message: SlStr);
pub type SlGetStrFn = unsafe extern "C" fn(bag: *mut c_void, key: SlStr, out: *mut SlStr) -> i32;
pub type SlGetF64Fn = unsafe extern "C" fn(bag: *mut c_void, key: SlStr, out: *mut f64) -> i32;
pub type SlGetI64Fn = unsafe extern "C" fn(bag: *mut c_void, key: SlStr, out: *mut i64) -> i32;
pub type SlGetBoolFn = unsafe extern "C" fn(bag: *mut c_void, key: SlStr, out: *mut bool) -> i32;
pub type SlSetCompositorStateFn =
  unsafe extern "C" fn(ctx: *mut c_void, state: *const SlCompositorState);

pub mod sl_log_level {
  pub const ERROR: i32 = 0;
  pub const WARN: i32 = 1;
  pub const INFO: i32 = 2;
  pub const DEBUG: i32 = 3;
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlTheme {
  pub base: SlColor,
  pub crust: SlColor,
  pub mantle: SlColor,
  pub primary: SlColor,
  pub secondary: SlColor,
  pub green: SlColor,
  pub red: SlColor,
  pub blue: SlColor,
  pub yellow: SlColor,
  pub orange: SlColor,
  pub text: SlColor,
  pub subtext: SlColor,
  pub overlay: SlColor,
}

pub mod sl_style_value_kind {
  pub const COLOR: u32 = 0;
  pub const INTEGER: u32 = 1;
  pub const FLOAT: u32 = 2;
  pub const STRING: u32 = 3;
  pub const BOOLEAN: u32 = 4;
}

pub mod sl_style_color_kind {
  pub const TRANSPARENT: u32 = 0;
  pub const THEME: u32 = 1;
  pub const HEX: u32 = 2;
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlStyleSheetEntry {
  pub key: SlStr,
  pub kind: u32,
  pub color_kind: u32,
  pub text: SlStr,
  pub integer: i32,
  pub float_value: f32,
  pub boolean: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct SlStyleSheet {
  pub entries: *const SlStyleSheetEntry,
  pub count: u32,
}

pub type SlStyleGetFn =
  unsafe extern "C" fn(ctx: *mut c_void, name: SlStr, out: *mut SlStyleSheet) -> i32;
pub type SlThemeGetFn = unsafe extern "C" fn(ctx: *mut c_void, out: *mut SlTheme) -> i32;
pub type SlStyleRegisterFn =
  unsafe extern "C" fn(ctx: *mut c_void, name: SlStr, sheet: *const SlStyleSheet) -> i32;

pub type SlCompositorStateGetFn =
  unsafe extern "C" fn(ctx: *mut c_void, out: *mut SlCompositorState) -> i32;

pub type SlConfigParserFn =
  unsafe extern "C" fn(ctx: *mut c_void, name: SlStr, block: SlStr) -> i32;

pub type SlConfigParserRegisterFn =
  unsafe extern "C" fn(host: *mut c_void, name: SlStr, callback: SlConfigParserFn) -> i32;

pub type SlPluginErrorFn = unsafe extern "C" fn(ctx: *mut c_void, code: u32, message: SlStr);

pub type SlNotifyFn =
  unsafe extern "C" fn(ctx: *mut c_void, notification: *const SlNotification) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SlHostApi {
  pub size: u32,
  pub register_component: Option<SlRegisterComponentFn>,
  pub register_compositor: Option<SlRegisterCompositorFn>,
  pub request_redraw: Option<SlRequestRedrawFn>,
  pub register_fd: Option<SlRegisterFdFn>,
  pub log: Option<SlLogFn>,
  pub get_str: Option<SlGetStrFn>,
  pub get_f64: Option<SlGetF64Fn>,
  pub get_i64: Option<SlGetI64Fn>,
  pub get_bool: Option<SlGetBoolFn>,
  pub set_compositor_state: Option<SlSetCompositorStateFn>,
  pub set_interval: Option<SlSetIntervalFn>,
  pub register_payload: Option<SlRegisterPayloadFn>,
  pub register_renderable: Option<SlRegisterRenderableFn>,
  pub register_desktop_item: Option<SlRegisterDesktopItemFn>,
  pub canvas: Option<SlCanvasAllocFn>,
  pub config_get_str: Option<SlGetStrFn>,
  pub config_get_f64: Option<SlGetF64Fn>,
  pub config_get_i64: Option<SlGetI64Fn>,
  pub config_get_bool: Option<SlGetBoolFn>,
  pub style_get: Option<SlStyleGetFn>,
  pub theme_get: Option<SlThemeGetFn>,
  pub style_register: Option<SlStyleRegisterFn>,
  pub compositor_state_get: Option<SlCompositorStateGetFn>,
  pub config_parser_register: Option<SlConfigParserRegisterFn>,
  pub plugin_error: Option<SlPluginErrorFn>,
  pub notify: Option<SlNotifyFn>,
  pub compositor_register_fd: Option<SlCompositorRegisterFdFn>,
  pub register_spotlight: Option<SlRegisterSpotlightFn>,
  pub registry_register: Option<SlRegistryRegisterFn>,
  pub registry_get: Option<SlRegistryGetFn>,
  pub registry_remove: Option<SlRegistryRemoveFn>,
  pub registry_subscribe: Option<SlRegistrySubscribeFn>,
  pub dispatch: Option<SlDispatchFn>,
  pub audio_state_get: Option<SlAudioStateGetFn>,
  pub bluetooth_state_get: Option<SlBluetoothStateGetFn>,
  pub network_state_get: Option<SlNetworkStateGetFn>,
  pub system_state_get: Option<SlSystemStateGetFn>,
  pub tray_state_get: Option<SlTrayStateGetFn>,
  pub power_state_get: Option<SlPowerStateGetFn>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SlPluginMeta {
  pub abi_version: u32,
  pub id: SlStr,
  pub version: SlStr,
}

pub type SlPluginGetMetaFn = unsafe extern "C" fn() -> SlPluginMeta;
pub type SlPluginInitFn = unsafe extern "C" fn(
  api: *const SlHostApi,
  host: *mut c_void,
  out_userdata: *mut *mut c_void,
) -> i32;
pub type SlPluginShutdownFn = unsafe extern "C" fn(userdata: *mut c_void);

pub const fn plugin_meta(id: &'static str, version: &'static str) -> SlPluginMeta {
  SlPluginMeta {
    abi_version: SL_PLUGIN_ABI_VERSION,
    id: SlStr::from_str(id),
    version: SlStr::from_str(version),
  }
}

#[macro_export]
macro_rules! export_plugin {
  ($register:path) => {
    #[unsafe(no_mangle)]
    pub extern "C" fn slowshell_plugin_meta() -> $crate::SlPluginMeta {
      $crate::plugin_meta(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slowshell_plugin_init(
      api: *const $crate::SlHostApi,
      host: *mut core::ffi::c_void,
      _userdata: *mut *mut core::ffi::c_void,
    ) -> i32 {
      unsafe { $register(api, host) };
      0
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slowshell_plugin_shutdown(_userdata: *mut core::ffi::c_void) {}
  };
}

pub unsafe fn register_component(
  api: *const SlHostApi,
  host: *mut c_void,
  name: &str,
  vtable: &'static SlComponentVtable,
) {
  let Some(api) = (unsafe { api.as_ref() }) else {
    return;
  };
  if let Some(register) = api.register_component {
    unsafe {
      register(
        host,
        SlStr::from_str(name),
        vtable as *const _,
        core::ptr::null_mut(),
      );
    }
  }
}

pub unsafe fn register_compositor(
  api: *const SlHostApi,
  host: *mut c_void,
  name: &str,
  vtable: &'static SlCompositorVtable,
) {
  let Some(api) = (unsafe { api.as_ref() }) else {
    return;
  };
  if let Some(register) = api.register_compositor {
    unsafe {
      register(
        host,
        SlStr::from_str(name),
        vtable as *const _,
        core::ptr::null_mut(),
      );
    }
  }
}

pub unsafe fn register_payload(
  api: *const SlHostApi,
  host: *mut c_void,
  command: &str,
  vtable: &'static SlPayloadVtable,
) {
  let Some(api) = (unsafe { api.as_ref() }) else {
    return;
  };
  if let Some(register) = api.register_payload {
    unsafe {
      register(
        host,
        SlStr::from_str(command),
        vtable as *const _,
        core::ptr::null_mut(),
      );
    }
  }
}

pub unsafe fn register_renderable(
  api: *const SlHostApi,
  host: *mut c_void,
  name: &str,
  vtable: &'static SlRenderableVtable,
) {
  let Some(api) = (unsafe { api.as_ref() }) else {
    return;
  };
  if let Some(register) = api.register_renderable {
    unsafe {
      register(
        host,
        SlStr::from_str(name),
        vtable as *const _,
        core::ptr::null_mut(),
      );
    }
  }
}

pub unsafe fn register_spotlight(
  api: *const SlHostApi,
  host: *mut c_void,
  name: &str,
  vtable: &'static SlSpotlightVtable,
) {
  let Some(api) = (unsafe { api.as_ref() }) else {
    return;
  };
  if let Some(register) = api.register_spotlight {
    unsafe {
      register(
        host,
        SlStr::from_str(name),
        vtable as *const _,
        core::ptr::null_mut(),
      );
    }
  }
}

pub unsafe fn register_desktop_item(
  api: *const SlHostApi,
  host: *mut c_void,
  name: &str,
  vtable: &'static SlDesktopItemVtable,
) {
  let Some(api) = (unsafe { api.as_ref() }) else {
    return;
  };
  if let Some(register) = api.register_desktop_item {
    unsafe {
      register(
        host,
        SlStr::from_str(name),
        vtable as *const _,
        core::ptr::null_mut(),
      );
    }
  }
}
