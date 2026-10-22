use std::sync::atomic::Ordering;

use slowshell_commons::{
  audio::{AUDIO_CHANGED, SharedAudioState, volume_icon},
  bluetooth::{BT_CHANGED, SharedBluetoothState, bluetooth_icon},
  network::{NET_CHANGED, SharedNetworkState},
  notifications::{NOTIF_CHANGED, SharedNotificationState},
};

use std::sync::{Arc, Mutex};

use iced::{
  Alignment, Element, Length,
  widget::{Space, column, container, row},
};
use iced_exwlshell::reexport::IcedId;
use slowshell_commons::quicksettings::*;
use slowshell_commons::quicksettings::{
  QuickSettingsProvider, QuickSettingsRegistry, QuickSettingsTile,
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{EventFilter, ItemEffect, ItemMessage, Message},
  types::ToUstr,
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{
  Backdrop, Icon, Renderable, SizedPopup, clickable, render_tile, sl_slider,
};

pub fn register_builtin_providers(store: &mut Store) {
  let registry = ensure_registry(store);

  let providers: Vec<Arc<dyn QuickSettingsProvider>> = vec![
    Arc::new(NetworkProvider),
    Arc::new(BluetoothProvider),
    Arc::new(SoundProvider),
    Arc::new(DndProvider),
    Arc::new(AirplaneProvider),
    Arc::new(PowerProvider),
  ];

  for provider in providers {
    registry.register(provider);
  }
}

pub struct QuickSettingsRenderable {
  expanded: Arc<Mutex<Option<String>>>,
  slots: Arc<Mutex<std::collections::HashMap<usize, String>>>,
}

impl Default for QuickSettingsRenderable {
  fn default() -> Self {
    Self {
      expanded: Arc::new(Mutex::new(None)),
      slots: Arc::new(Mutex::new(std::collections::HashMap::new())),
    }
  }
}

impl QuickSettingsRenderable {
  pub const EXPAND: usize = 0;
  pub const COLLAPSE: usize = 1;
}

pub fn handle_tile_expand(
  expanded: &Arc<Mutex<Option<String>>>,
  slots: &Arc<Mutex<std::collections::HashMap<usize, String>>>,
  message: &ItemMessage,
) -> Option<ItemEffect> {
  let ItemMessage::Effect(_, ItemEffect::Custom([42, kind, slot, _])) = message else {
    return None;
  };

  let id = slots
    .lock()
    .ok()
    .and_then(|slots| slots.get(slot).cloned())?;

  if let Ok(mut expanded) = expanded.lock() {
    match *kind {
      QuickSettingsRenderable::EXPAND => *expanded = Some(id),
      QuickSettingsRenderable::COLLAPSE if expanded.as_deref() == Some(id.as_str()) => {
        *expanded = None
      }
      _ => {}
    }
  }

  Some(ItemEffect::Redraw)
}

pub fn render_quicksettings_content<'a>(
  config: &'a Config,
  store: &'a Store,
  id: IcedId,
  expanded: Option<&str>,
  slots: &Arc<Mutex<std::collections::HashMap<usize, String>>>,
  settings: Option<&PopupSettings>,
) -> Vec<Element<'a, ItemMessage>> {
  let registry = store.borrow::<QuickSettingsRegistry>();
  let tiles = registry
    .as_ref()
    .map(|registry| registry.tiles(config, store))
    .unwrap_or_default();

  let mut allocator = SlotAllocator::default();
  if let Ok(mut s) = slots.lock() {
    s.clear();
  }

  let mut children: Vec<Element<'a, ItemMessage>> = Vec::new();
  children.push(render_user_header(config));

  if let Some(registry) = registry.as_ref() {
    children.extend(registry.sections(config, store));
  }

  let grid = render_tile_grid(
    config,
    &tiles,
    expanded,
    id,
    &mut allocator,
    slots,
    settings,
  );
  children.push(grid);

  if let (Some(expanded), Some(registry)) = (expanded, registry.as_ref()) {
    let sub_tiles = registry.children(config, store, expanded);
    if !sub_tiles.is_empty() {
      children.push(render_expanded(
        config,
        &sub_tiles,
        id,
        &mut allocator,
        slots,
        settings,
      ));
    }
  }

  children
}

impl Renderable for QuickSettingsRenderable {
  fn events(&self, store: &Store) -> Vec<EventFilter> {
    store
      .borrow::<QuickSettingsRegistry>()
      .map(|registry| registry.events())
      .unwrap_or_default()
  }

  fn handle_message(&mut self, message: &ItemMessage) -> Option<ItemEffect> {
    handle_tile_expand(&self.expanded, &self.slots, message)
  }

  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    data: &'a dyn std::any::Any,
  ) -> Element<'a, ItemMessage> {
    let data = data.downcast_ref::<PopupSettings>().unwrap();

    let expanded = self.expanded.lock().ok().and_then(|slot| slot.clone());
    let children = render_quicksettings_content(
      config,
      store,
      id,
      expanded.as_deref(),
      &self.slots,
      Some(data),
    );

    let list_spacing = config.sizing(&["menu", "list"]).gap;
    let list_width = config
      .computed(&["menu", "qs"])
      .length("width")
      .unwrap_or(340.0);
    let list = column(children)
      .spacing(list_spacing)
      .width(Length::Fixed(list_width));


    let inner_padding = config.sizing(&["menu", "inner"]).padding();
    let popup_style = config.container_cached(&["menu"]);

    SizedPopup::new(container(
      container(list).padding(inner_padding).style(popup_style),
    ))
    .with_width(140.)
    .with_backdrop(Backdrop::transparent().with_close_on_click(true))
    .with_position(data.position.0.get(store), data.position.1.get(store))
    .with_panel_edge(data.panel_edge)
    .with_panel_insets(store, data.monitor.as_deref())
    .with_on_close(ItemMessage::Effect(id, ItemEffect::Hide))
    .into()
  }
}

pub fn render_user_header<'a>(config: &'a Config) -> Element<'a, ItemMessage> {

  let faded = config.palette().subtext;
  let avatar = config.computed(&["menu", "qs", "avatar"]);
  let avatar_size = avatar.length("size").unwrap_or(42.0);
  let avatar_radius = avatar.length("border-radius").unwrap_or(42.0);

  let avatar: Element<'a, ItemMessage> = if let Some(handle) = user_picture() {
    iced::widget::image(handle)
      .width(Length::Fixed(avatar_size))
      .height(Length::Fixed(avatar_size))
      .content_fit(iced::ContentFit::Cover)
      .border_radius(avatar_radius)
      .into()
  } else {
    Icon::new("avatar-default-symbolic")
      .size(avatar_size as u16)
      .color(faded)
      .into()
  };

  let avatar = container(avatar)
    .width(Length::Fixed(avatar_size))
    .height(Length::Fixed(avatar_size))
    .clip(true)
    .style(move |_| container::Style {
      border: iced::Border {
        radius: (avatar_size / 2.0).into(),
        ..Default::default()
      },
      ..Default::default()
    });

  let names = column![
    config.text(user_name(), &["menu", "text-header"]),
    config.text(host_name(), &["menu", "text-status", "text-faded"]),
  ]
  .spacing(1);

  row![
    avatar,
    Space::new().width(10),
    names,
    Space::new().width(Length::Fill),
    config.text(uptime_label(), &["menu", "text-status", "text-faded"]),
  ]
  .align_y(Alignment::Center)
  .into()
}

#[derive(Default)]
pub struct SlotAllocator {
  pub next: usize,
}

impl SlotAllocator {
  pub fn allocate(
    &mut self,
    tile_id: &str,
    slots: &Arc<Mutex<std::collections::HashMap<usize, String>>>,
  ) -> usize {
    let slot = self.next;
    self.next += 1;
    if let Ok(mut slots) = slots.lock() {
      slots.insert(slot, tile_id.to_string());
    }
    slot
  }
}

pub fn render_tile_grid<'a>(
  config: &'a Config,
  tiles: &[QuickSettingsTile],
  expanded: Option<&str>,
  id: IcedId,
  allocator: &mut SlotAllocator,
  slots: &Arc<Mutex<std::collections::HashMap<usize, String>>>,
  settings: Option<&PopupSettings>,
) -> Element<'a, ItemMessage> {

  let mut rows: Vec<Element<'a, ItemMessage>> = Vec::new();
  let mut current: Vec<Element<'a, ItemMessage>> = Vec::new();

  for tile in tiles {
    let is_expanded = expanded == Some(tile.id.as_str());
    let rendered = tile_element(config, tile, is_expanded, id, allocator, slots, settings);
    current.push(rendered);

    if current.len() == 2 {
      rows.push(row(std::mem::take(&mut current)).spacing(8).into());
    }
  }

  if !current.is_empty() {
    current.push(Space::new().width(Length::FillPortion(1)).into());
    rows.push(row(current).spacing(8).into());
  }

  column(rows).spacing(8).into()
}

fn tile_element<'a>(
  config: &'a Config,
  tile: &QuickSettingsTile,
  expanded: bool,
  id: IcedId,
  allocator: &mut SlotAllocator,
  slots: &Arc<Mutex<std::collections::HashMap<usize, String>>>,
  settings: Option<&PopupSettings>,
) -> Element<'a, ItemMessage> {
  let base = render_tile(tile, expanded, config);

  match tile.on_click.clone() {
    TileAction::None => base,
    TileAction::Command(command) => render_action_tile(base, action_message(command)),
    TileAction::Commands(commands) => render_action_tile(base, commands_message(commands)),
    TileAction::Menu(menu) => render_action_tile(base, menu_open_message(id, menu, settings)),
    TileAction::Expand => {
      let slot = allocator.allocate(&tile.id, slots);
      clickable(base, move |_, _, _| {
        let kind = if expanded {
          QuickSettingsRenderable::COLLAPSE
        } else {
          QuickSettingsRenderable::EXPAND
        };
        Some(expand_message(id, kind, slot))
      })
    }
  }
}

fn expand_message(id: IcedId, kind: usize, slot: usize) -> ItemMessage {
  ItemMessage::Effect(id, ItemEffect::Custom([42, kind, slot, 0]))
}

fn action_message(command: String) -> ItemMessage {
  ItemMessage::Action(ListenerAction::Payload {
    name: command.to_ustr(),
    payload: None,
  })
}

fn commands_message(commands: Vec<String>) -> ItemMessage {
  ItemMessage::Task(Arc::new(move || {
    iced::Task::batch(commands.iter().map(|command| {
      iced::Task::done(Message::Item(ItemMessage::Action(
        ListenerAction::Payload {
          name: command.to_ustr(),
          payload: None,
        },
      )))
    }))
  }))
}

fn menu_open_message(id: IcedId, content: String, settings: Option<&PopupSettings>) -> ItemMessage {
  ItemMessage::EffectAction(
    id,
    ItemEffect::Hide,
    ListenerAction::Payload {
      name: "popup.open".to_ustr(),
      payload: Some(slowshell_core::types::PayloadBox::new(
        settings
          .map(|p| PopupSettings {
            content: content.to_ustr(),
            ..p.clone()
          })
          .unwrap_or(PopupSettings {
            content: content.to_ustr(),
            panel_edge: None,
            position: (
              slowshell_commons::popups::PopupPosition::Fixed(0.0),
              slowshell_commons::popups::PopupPosition::Fixed(0.0),
            ),
            insets: false,
            monitor: None,
          }),
      )),
    },
  )
}

fn render_action_tile<'a>(
  base: Element<'a, ItemMessage>,
  message: ItemMessage,
) -> Element<'a, ItemMessage> {
  clickable(base, move |_, _, _| Some(message.clone()))
}

pub fn render_expanded<'a>(
  config: &'a Config,
  tiles: &[QuickSettingsTile],
  id: IcedId,
  allocator: &mut SlotAllocator,
  slots: &Arc<Mutex<std::collections::HashMap<usize, String>>>,
  settings: Option<&PopupSettings>,
) -> Element<'a, ItemMessage> {
  let mut items: Vec<Element<'a, ItemMessage>> = tiles
    .iter()
    .map(|tile| tile_element(config, tile, false, id, allocator, slots, settings))
    .collect();

  if items.is_empty() {
    items.push(
      config
        .text("Nothing here yet", &["menu", "text-status", "text-faded"])
        .into(),
    );
  }

  container(column(items).spacing(6).width(Length::Fill))
    .padding([4.0, 4.0])
    .width(Length::Fill)
    .into()
}

// fn render_footer<'a>(config: &'a Config) -> Element<'a, ItemMessage> {
//   let engine = config.theme();
//   let style = config.computed(&["menu"]);
//   let faded = style.color("color-faded").map(|c| c.to_iced()).unwrap_or(config.palette().subtext);

//   row![
//     Icon::new("preferences-system-symbolic")
//       .size(14)
//       .color(faded),
//     Space::new().width(6),
//     config.text("Settings", &["menu", "text-status", "text-faded"]),
//   ]
//   .align_y(Alignment::Center)
//   .into()
// }

pub fn user_name() -> &'static str {
  static NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
  NAME
    .get_or_init(|| {
      std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "user".to_string())
    })
    .as_str()
}

pub fn host_name() -> &'static str {
  static HOST: std::sync::OnceLock<String> = std::sync::OnceLock::new();
  HOST
    .get_or_init(|| {
      std::fs::read_to_string("/etc/hostname")
        .map(|name| name.trim().to_string())
        .ok()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "localhost".to_string())
    })
    .as_str()
}

pub fn user_picture() -> Option<&'static iced::widget::image::Handle> {

  static PICTURE: std::sync::OnceLock<Option<iced::widget::image::Handle>> =
    std::sync::OnceLock::new();

  PICTURE
    .get_or_init(|| {
      let home = std::env::var("HOME").ok()?;
      let home = std::path::PathBuf::from(home);

      let mut candidates = Vec::new();

      for candidate in [".face", ".face.icon", ".config/avatar"] {
        candidates.push(home.join(candidate));
      }

      if let Ok(user) = std::env::var("USER") {
        candidates.push(std::path::PathBuf::from(format!(
          "/var/lib/AccountsService/icons/{user}"
        )));
      }

      let path = candidates.into_iter().find(|path| path.is_file())?;

      let bytes = std::fs::read(path).ok()?;

      Some(iced::widget::image::Handle::from_bytes(bytes))
    })
    .as_ref()
}

pub fn uptime_label() -> String {
  static CACHE: Mutex<Option<(std::time::Instant, String)>> = Mutex::new(None);

  let Ok(mut cache) = CACHE.lock() else {
    return String::new();
  };

  if let Some((computed_at, label)) = cache.as_ref()
    && computed_at.elapsed() < std::time::Duration::from_secs(60)
  {
    return label.clone();
  }

  let label = compute_uptime_label();
  *cache = Some((std::time::Instant::now(), label.clone()));
  label
}

fn compute_uptime_label() -> String {
  let Ok(raw) = std::fs::read_to_string("/proc/uptime") else {
    return String::new();
  };

  let seconds = raw
    .split_whitespace()
    .next()
    .and_then(|value| value.parse::<f64>().ok())
    .unwrap_or(0.0) as u64;

  let days = seconds / 86_400;
  let hours = (seconds % 86_400) / 3_600;
  let minutes = (seconds % 3_600) / 60;

  if days > 0 {
    format!("Up {days}d {hours}h")
  } else if hours > 0 {
    format!("Up {hours}h {minutes}m")
  } else {
    format!("Up {minutes}m")
  }
}

pub struct NetworkProvider;

impl QuickSettingsProvider for NetworkProvider {
  fn id(&self) -> &str {
    "network"
  }

  fn events(&self) -> Vec<EventFilter> {
    vec![EventFilter::Named(NET_CHANGED.into())]
  }

  fn tiles(&self, _config: &Config, store: &Store) -> Vec<QuickSettingsTile> {
    let Some(shared) = store.borrow::<SharedNetworkState>() else {
      return Vec::new();
    };

    let enabled = shared.wifi_enabled.load(Ordering::Relaxed);
    let inner = shared.state.lock().unwrap().clone();

    let status = if !enabled {
      "Off".to_string()
    } else if let Some(conn) = inner.networks.iter().find(|net| net.in_use) {
      format!("{} · {}%", conn.ssid, conn.signal)
    } else if inner.ethernet.as_ref().is_some_and(|eth| eth.connected) {
      "Ethernet".to_string()
    } else {
      "Not connected".to_string()
    };

    vec![
      QuickSettingsTile::new("network", "network-wireless-symbolic", "Wi-Fi")
        .status(status)
        .active(enabled)
        .menu("menus/network")
        .order(10),
    ]
  }
}

pub struct BluetoothProvider;

impl QuickSettingsProvider for BluetoothProvider {
  fn id(&self) -> &str {
    "bluetooth"
  }

  fn events(&self) -> Vec<EventFilter> {
    vec![EventFilter::Named(BT_CHANGED.into())]
  }

  fn tiles(&self, _config: &Config, store: &Store) -> Vec<QuickSettingsTile> {
    let Some(shared) = store.borrow::<SharedBluetoothState>() else {
      return Vec::new();
    };

    let powered = shared.powered.load(Ordering::Relaxed);
    let inner = shared.state.lock().unwrap().clone();
    let connected = inner.devices.iter().filter(|dev| dev.connected).count();

    let status = if !powered {
      "Off".to_string()
    } else if connected == 1 {
      inner
        .devices
        .iter()
        .find(|dev| dev.connected)
        .map(|dev| dev.name.clone())
        .unwrap_or_else(|| "1 device".to_string())
    } else if connected > 1 {
      format!("{connected} devices")
    } else {
      "Not connected".to_string()
    };

    vec![
      QuickSettingsTile::new("bluetooth", bluetooth_icon(powered, connected), "Bluetooth")
        .status(status)
        .active(powered)
        .menu("menus/bluetooth")
        .order(20),
    ]
  }
}

pub struct SoundProvider;

impl QuickSettingsProvider for SoundProvider {
  fn id(&self) -> &str {
    "sound"
  }

  fn events(&self) -> Vec<EventFilter> {
    vec![EventFilter::Named(AUDIO_CHANGED.into())]
  }

  fn tiles(&self, _config: &Config, store: &Store) -> Vec<QuickSettingsTile> {
    let Some(shared) = store.borrow::<SharedAudioState>() else {
      return Vec::new();
    };

    let (volume, muted) = {
      let state = shared.state.lock().unwrap();
      (state.volume, state.muted)
    };

    let status = if muted {
      "Muted".to_string()
    } else {
      format!("{}%", (volume * 100.0).round() as u32)
    };

    vec![
      QuickSettingsTile::new("sound", volume_icon(volume, muted), "Sound")
        .status(status)
        .active(!muted && volume > 0.0)
        .action("audio.toggle_mute")
        .order(30),
    ]
  }

  fn section<'a>(
    &self,
    config: &Config,
    store: &Store,
  ) -> Option<iced::Element<'a, slowshell_core::message::ItemMessage>> {
    let shared = store.borrow::<SharedAudioState>()?;
    let (volume, muted) = {
      let state = shared.state.lock().unwrap();
      (state.volume, state.muted)
    };

    Some(sl_slider(
      "volume",
      "audio.set_volume",
      volume,
      0.0..=1.5,
      muted,
      &config.computed(&["slider"]),
      config.palette(),
    ))
  }
}

pub struct DndProvider;

impl QuickSettingsProvider for DndProvider {
  fn id(&self) -> &str {
    "dnd"
  }

  fn events(&self) -> Vec<EventFilter> {
    vec![EventFilter::Named(NOTIF_CHANGED.into())]
  }

  fn tiles(&self, _config: &Config, store: &Store) -> Vec<QuickSettingsTile> {
    let Some(shared) = store.borrow::<SharedNotificationState>() else {
      return Vec::new();
    };

    let dnd = shared.dnd.load(Ordering::Relaxed);

    vec![
      QuickSettingsTile::new(
        "dnd",
        if dnd {
          "notifications-disabled-symbolic"
        } else {
          "notifications-symbolic"
        },
        "Do Not Disturb",
      )
      .status(if dnd { "On" } else { "Off" })
      .active(dnd)
      .action("notifications.toggle_dnd")
      .order(50),
    ]
  }
}

pub struct AirplaneProvider;

impl QuickSettingsProvider for AirplaneProvider {
  fn id(&self) -> &str {
    "airplane"
  }

  fn events(&self) -> Vec<EventFilter> {
    vec![
      EventFilter::Named(NET_CHANGED.into()),
      EventFilter::Named(BT_CHANGED.into()),
    ]
  }

  fn tiles(&self, _config: &Config, store: &Store) -> Vec<QuickSettingsTile> {
    let wifi = store
      .borrow::<SharedNetworkState>()
      .map(|state| state.wifi_enabled.load(Ordering::Relaxed));
    let bluetooth = store
      .borrow::<SharedBluetoothState>()
      .map(|state| state.powered.load(Ordering::Relaxed));

    if wifi.is_none() && bluetooth.is_none() {
      return Vec::new();
    }

    let on = wifi == Some(false) && bluetooth == Some(false);

    vec![
      QuickSettingsTile::new("airplane", "airplane-mode-symbolic", "Airplane Mode")
        .status(if on { "On" } else { "Off" })
        .active(on)
        .commands(["network.toggle", "bluetooth.toggle"])
        .order(60),
    ]
  }
}

pub struct PowerProvider;

impl QuickSettingsProvider for PowerProvider {
  fn id(&self) -> &str {
    "power"
  }

  fn tiles(&self, _config: &Config, _store: &Store) -> Vec<QuickSettingsTile> {
    vec![
      QuickSettingsTile::new("power", "system-shutdown-symbolic", "Power")
        .status("Session and power")
        .menu("menus/actions")
        .order(70),
    ]
  }
}
