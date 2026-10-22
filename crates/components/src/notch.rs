use std::sync::{
  Arc, Mutex,
  atomic::{AtomicBool, AtomicU64, Ordering},
};

use iced::{
  Alignment, Color, Element, Length,
  widget::{Space, column, container, row, svg},
};
use slowshell_commons::power::POWER_CHANGED;
#[cfg(feature = "power")]
use slowshell_commons::power::{SharedPowerState, battery_icon};
#[cfg(feature = "tray")]
use slowshell_commons::tray::{SharedTrayState, TrayCmd, TrayItem, TrayPayload};
use slowshell_commons::{
  audio::{AUDIO_CHANGED, SharedAudioState},
  bluetooth::{BT_CHANGED, SharedBluetoothState},
  desktop::DesktopEntries,
  network::{NET_CHANGED, SharedNetworkState, signal_icon},
  notch::{NOTCH_STATUS_CHANGED, NotchStatus},
  notifications::{NOTIF_CHANGED, SharedNotificationState},
  panels::{PanelEdge, PanelOrientation},
  popups::{PopupPosition, PopupSettings},
  system::shared as system_shared,
};
use slowshell_compositor::CompositorStore;
use slowshell_config::Config;
#[cfg(feature = "tray")]
use slowshell_core::types::OptionalPayloadBox;
use slowshell_core::{
  Store,
  listeners::{CompositorEvent, ListenerAction},
  message::{EventFilter, ItemEffect, ItemMessage},
  types::{PayloadBox, ToUstr},
};
#[cfg(feature = "notifications")]
use slowshell_services::notifications::ensure as ensure_notifications;
#[cfg(feature = "tray")]
use slowshell_services::tray::TRAY_TICK;
use slowshell_services::util::drain_signal_fd;
use slowshell_widgets::{ComponentOptions, EventWrapper, Icon, clickable, separator};

use crate::{Component, ComponentContext};
#[cfg(feature = "tray")]
use crate::{MenuConfig, popup_open_action};

pub struct Notch {
  hovered: Arc<AtomicBool>,
  tray_bounds: Arc<Mutex<Option<iced::Rectangle>>>,
  #[cfg(feature = "tray")]
  last_tray_revision: u64,
  last_notif_revision: AtomicU64,
}

impl Default for Notch {
  fn default() -> Self {
    Self::new()
  }
}

impl Notch {
  pub fn new() -> Self {
    Self {
      hovered: Arc::new(AtomicBool::new(false)),
      tray_bounds: Arc::new(Mutex::new(None)),
      #[cfg(feature = "tray")]
      last_tray_revision: 0,
      last_notif_revision: AtomicU64::new(0),
    }
  }

  #[cfg(feature = "tray")]
  fn redraw_tray_if_changed(&mut self) -> ItemEffect {
    let revision = slowshell_services::tray::shared()
      .revision
      .load(Ordering::Relaxed);
    if revision != self.last_tray_revision {
      self.last_tray_revision = revision;
      ItemEffect::Redraw
    } else {
      ItemEffect::None
    }
  }

  fn redraw_notif_if_changed(&self, store: &Store) -> ItemEffect {
    if let Some(shared) = store.borrow::<SharedNotificationState>() {
      let rev = shared.revision.load(Ordering::SeqCst);
      let old = self.last_notif_revision.swap(rev, Ordering::SeqCst);
      if rev != old {
        return ItemEffect::Redraw;
      }
    }
    ItemEffect::None
  }
}

impl Component for Notch {
  fn events(&self) -> Vec<EventFilter> {
    #[allow(unused_mut)]
    let mut events = vec![
      EventFilter::Compositor(
        CompositorEvent::WORKSPACES
          | CompositorEvent::WINDOWS
          | CompositorEvent::FOCUS
          | CompositorEvent::MONITORS,
      ),
      EventFilter::Named("compositor.focus".into()),
      EventFilter::Named(AUDIO_CHANGED.into()),
      EventFilter::Named(NET_CHANGED.into()),
      EventFilter::Named(BT_CHANGED.into()),
      EventFilter::Named(NOTCH_STATUS_CHANGED.into()),
      EventFilter::Named(NOTIF_CHANGED.into()),
      EventFilter::Named("config.reload".into()),
      EventFilter::Tick,
    ];
    #[cfg(feature = "power")]
    {
      events.push(EventFilter::Named(
        slowshell_commons::power::POWER_CHANGED.into(),
      ));
    }
    #[cfg(feature = "tray")]
    {
      events.push(EventFilter::Named(TRAY_TICK.into()));
      events.push(EventFilter::Payload("tray.activate".into()));
    }
    events
  }

  fn watch(&mut self, store: &mut Store, _options: Option<&ComponentOptions>) {
    #[cfg(feature = "audio")]
    {
      slowshell_services::audio::ensure(store);
    }
    #[cfg(feature = "network")]
    {
      slowshell_services::network::ensure(store);
    }
    #[cfg(feature = "bluetooth")]
    {
      slowshell_services::bluetooth::ensure(store);
    }
    #[cfg(feature = "tray")]
    {
      slowshell_services::tray::ensure(store);
    }
    #[cfg(feature = "notifications")]
    {
      ensure_notifications(store);
    }
    #[cfg(feature = "power")]
    {
      slowshell_services::power::ensure(store);
    }
    let _ = store;
  }

  fn update(
    &mut self,
    _config: &Config,
    store: &mut Store,
    event: &ListenerAction,
    _options: Option<&ComponentOptions>,
  ) -> miette::Result<ItemEffect> {
    match event {
      ListenerAction::Signal { name, fd } if &**name == NOTIF_CHANGED => {
        drain_signal_fd(*fd);
        return Ok(self.redraw_notif_if_changed(store));
      }
      ListenerAction::Named(n) if &**n == NOTIF_CHANGED => {
        return Ok(self.redraw_notif_if_changed(store));
      }
      ListenerAction::Signal { name, fd }
        if &**name == AUDIO_CHANGED
          || &**name == NET_CHANGED
          || &**name == BT_CHANGED
          || &**name == NOTCH_STATUS_CHANGED
          || &**name == POWER_CHANGED =>
      {
        drain_signal_fd(*fd);
        return Ok(ItemEffect::Redraw);
      }
      ListenerAction::Named(n)
        if &**n == AUDIO_CHANGED
          || &**n == NET_CHANGED
          || &**n == BT_CHANGED
          || &**n == NOTCH_STATUS_CHANGED
          || &**n == POWER_CHANGED
          || &**n == "compositor.focus"
          || &**n == "config.reload" =>
      {
        return Ok(ItemEffect::Redraw);
      }
      ListenerAction::UpdateCompositor(_) => return Ok(ItemEffect::Redraw),
      _ => {}
    }

    #[cfg(feature = "tray")]
    {
      if store.borrow::<SharedTrayState>().is_none() {
        store.insert(slowshell_services::tray::shared().clone());
      }
      match event {
        ListenerAction::Named(n) if &**n == TRAY_TICK => return Ok(self.redraw_tray_if_changed()),
        ListenerAction::Signal { name, fd } if &**name == TRAY_TICK => {
          drain_signal_fd(*fd);
          return Ok(self.redraw_tray_if_changed());
        }
        ListenerAction::Payload { name, payload } if name.as_ref() == "tray.activate" => {
          if let Some(payload) = payload.transform::<TrayPayload>() {
            if let Some(menu_path) = payload.menu_path.clone() {
              let _ = slowshell_services::tray::shared()
                .cmd_tx
                .send(TrayCmd::Activate {
                  address: payload.address.to_string(),
                  menu_path: menu_path.to_string(),
                  submenu_id: payload.submenu_id,
                });
            }
          }
          return Ok(ItemEffect::None);
        }
        _ => {}
      }
    }
    let _ = (store, event);
    Ok(ItemEffect::None)
  }

  fn view<'a>(
    &'a self,
    config: &'a Config,
    store: &'a Store,
    ctx: &ComponentContext,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let palette = config.palette();

    let element_names = parse_notch_elements(options);
    let clock_format = parse_clock_format(options);
    let window_stat_key = options.and_then(|o| o.str("window-stat"));
    let system_displays = parse_system_displays(options);
    let priority_list = parse_priority(options);
    let shade_on_hover = options
      .and_then(|o| o.bool("shade-on-hover"))
      .unwrap_or(false);
    let battery_status = parse_battery_status(options);

    let sep_height = config
      .computed(&["notch", "separator"])
      .length("height")
      .unwrap_or(7.0);

    let (nowplaying_info, is_nowplaying_active) = query_nowplaying(store);

    let condense_window = if let (Some(np_idx), Some(w_idx)) = (
      priority_list.iter().position(|p| p == "nowplaying"),
      priority_list.iter().position(|p| p == "window"),
    ) {
      np_idx < w_idx && is_nowplaying_active
    } else {
      is_nowplaying_active
    };

    let mut rendered_elements: Vec<Element<'a, ItemMessage>> = Vec::new();
    let mut tray_idx: Option<usize> = None;

    let incoming_notification = store
      .borrow::<SharedNotificationState>()
      .and_then(|shared| {
        let guard = shared.state.lock().ok()?;
        guard.popups.last().cloned()
      });

    if let Some(notif) = &incoming_notification {
      let notif_icon = notif
        .app_icon
        .as_deref()
        .unwrap_or("preferences-system-notifications-symbolic");
      let icon_elem = Icon::any([
        notif_icon,
        "preferences-system-notifications-symbolic",
        "dialog-information-symbolic",
      ])
      .size(13)
      .color(palette.primary);

      let summary = if notif.summary.chars().count() > 32 {
        format!("{}…", notif.summary.chars().take(31).collect::<String>())
      } else if notif.summary.is_empty() {
        "Notification".to_string()
      } else {
        notif.summary.clone()
      };

      let label_elem = config.text(summary, &["notch", "notification-title"]);

      rendered_elements.push(
        row![icon_elem, Space::new().width(6), label_elem]
          .align_y(Alignment::Center)
          .into(),
      );
    } else {
      for elem_name in &element_names {
        match elem_name.as_str() {
          "workspaces" => {
            if let Some(workspaces_elem) = render_workspaces_dots(config, store, ctx) {
              rendered_elements.push(workspaces_elem);
            }
          }
          "sep" => {
            rendered_elements.push(
              container(Space::new().height(sep_height).width(2.0))
                .padding([5, 0])
                .style(move |_t: &iced::Theme| container::Style {
                  background: Some(
                    Color {
                      a: 0.1,
                      ..config
                        .color(&["component"], "color-faded")
                        .unwrap_or(Color::from_rgb(0., 0., 0.))
                    }
                    .into(),
                  ),
                  border: iced::Border {
                    radius: 4.0.into(),
                    ..Default::default()
                  },
                  ..container::Style::default()
                })
                .into(),
            );
          }
          "sep-circle" => {
            rendered_elements.push(
              separator()
                .circle(true)
                .padding([0, 0])
                .size(5.0)
                .opacity(0.5)
                .color(
                  config
                    .color(&["component"], "color-faded")
                    .unwrap_or(Color::from_rgb(0., 0., 0.)),
                )
                .into(),
            );
          }
          "window" => {
            if let Some(window_elem) =
              render_window(config, store, window_stat_key, condense_window)
            {
              rendered_elements.push(window_elem);
            }
          }
          "nowplaying" => {
            if let Some((artist, title)) = &nowplaying_info {
              rendered_elements.push(render_nowplaying(config, artist, title));
            }
          }
          "system" => {
            if let Some(sys_elem) = render_system(config, &system_displays) {
              rendered_elements.push(sys_elem);
            }
          }
          "network" => {
            rendered_elements.push(render_network(config, store));
          }
          "battery" => {
            if let Some(battery_elem) = render_battery(config, store, battery_status) {
              rendered_elements.push(battery_elem);
            }
          }
          "clock" => {
            rendered_elements.push(render_clock(config, clock_format));
          }
          "tray" => {
            if let Some(tray_elem) = render_tray(config, ctx, self.tray_bounds.clone()) {
              tray_idx = Some(rendered_elements.len());
              rendered_elements.push(tray_elem);
            }
          }
          custom_name => {
            if let Some(status) = slowshell_commons::notch::shared().get(custom_name) {
              rendered_elements.push(render_notch_status(config, &status));
            }
          }
        }
      }
    }

    let notch_computed = config.computed(&["notch"]);
    let pad_v = notch_computed.length("padding-v").unwrap_or(3.0);
    let pad_h = notch_computed.length("padding-h").unwrap_or(10.0);
    let gap = notch_computed.length("gap").unwrap_or(8.0);

    let is_vertical = ctx.orientation == PanelOrientation::Vertical;
    let content: Element<'a, ItemMessage> = if is_vertical {
      column(rendered_elements)
        .spacing(gap)
        .align_x(Alignment::Center)
        .into()
    } else {
      row(rendered_elements)
        .spacing(gap)
        .align_y(Alignment::Center)
        .into()
    };

    let pill_style = config.container_style(&["notch"]);

    let pill = container(content)
      .padding([pad_v, pad_h])
      .style(move |_| pill_style);

    let hovered = self.hovered.clone();
    let tray_bounds_fallback = self.tray_bounds.clone();
    let panel_edge = ctx.position;
    let panel_name = ctx.panel_name.to_string();
    let monitor = ctx.monitor.to_ustr();
    let has_incoming_notif = incoming_notification.is_some();

    EventWrapper::new(pill.into(), move |event, layout, cursor| {
      let bounds = layout.bounds();
      let is_over_notch = cursor.is_over(bounds);

      let is_over_tray = {
        let mut over = false;
        if let Some(idx) = tray_idx {
          if let Some(row_layout) = layout.children().next() {
            if let Some(tray_child) = row_layout.children().nth(idx) {
              over = cursor.is_over(tray_child.bounds());
            }
          }
        }
        if !over {
          if let Ok(lock) = tray_bounds_fallback.lock() {
            over = lock.as_ref().map_or(false, |tb| cursor.is_over(*tb));
          }
        }
        over
      };

      if !is_over_notch || is_over_tray {
        hovered.store(false, Ordering::Relaxed);
        return None;
      }

      let popup_pos = match panel_edge {
        PanelEdge::Top { .. } | PanelEdge::Bottom { .. } => (
          PopupPosition::Fixed(bounds.center_x() + 220.0),
          if !panel_name.is_empty() {
            PopupPosition::Panel {
              name: panel_name.clone().into(),
            }
          } else {
            PopupPosition::Fixed(0.0)
          },
        ),
        PanelEdge::Left { .. } | PanelEdge::Right { .. } => (
          if !panel_name.is_empty() {
            PopupPosition::Panel {
              name: panel_name.clone().into(),
            }
          } else {
            PopupPosition::Fixed(0.0)
          },
          PopupPosition::Fixed(bounds.center_y() + 310.0),
        ),
      };

      let click_target = if has_incoming_notif {
        "menus/notifications"
      } else {
        "menus/shade"
      };

      let click_settings = PopupSettings {
        content: click_target.into(),
        panel_edge: Some(panel_edge),
        position: popup_pos.clone(),
        insets: true,
        monitor: Some(monitor.clone()),
      };

      let hover_settings = PopupSettings {
        content: "menus/shade".into(),
        panel_edge: Some(panel_edge),
        position: popup_pos,
        insets: true,
        monitor: Some(monitor.clone()),
      };

      match event {
        iced::Event::Mouse(iced::mouse::Event::CursorMoved { .. }) => {
          if shade_on_hover
            && hovered
              .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
              .is_ok()
          {
            let open_action = ListenerAction::Payload {
              name: "popup.open".to_ustr(),
              payload: Some(PayloadBox::new(hover_settings)),
            };
            return Some(ItemMessage::Action(open_action));
          }
        }
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
          let toggle_action = ListenerAction::Payload {
            name: "popup.toggle".to_ustr(),
            payload: Some(PayloadBox::new(click_settings)),
          };
          return Some(ItemMessage::Action(toggle_action));
        }
        _ => {}
      }

      None
    })
    .into()
  }
}

pub fn parse_notch_elements(options: Option<&ComponentOptions>) -> Vec<String> {
  if let Some(opts) = options {
    if let Some(strings) = opts.strings("notch") {
      if !strings.is_empty() {
        return strings.into_iter().map(String::from).collect();
      }
    }
    if let Some(s) = opts.str("notch") {
      if !s.is_empty() {
        return vec![s.to_string()];
      }
    }
  }
  vec![
    "clock".to_string(),
    "nowplaying".to_string(),
    "tray".to_string(),
    "network".to_string(),
    "battery".to_string(),
    "workspaces".to_string(),
    "window".to_string(),
    "system".to_string(),
  ]
}

pub fn parse_clock_format(options: Option<&ComponentOptions>) -> &str {
  options
    .and_then(|o| o.str("clock-format"))
    .and_then(|fmt| if fmt.is_empty() { None } else { Some(fmt) })
    .unwrap_or("%H:%M")
}

pub fn parse_system_displays(options: Option<&ComponentOptions>) -> Vec<String> {
  let list = if let Some(opts) = options {
    if let Some(strings) = opts.strings("system-display") {
      strings.into_iter().map(String::from).collect()
    } else if let Some(s) = opts.str("system-display") {
      vec![s.to_string()]
    } else {
      vec!["cpu".to_string(), "mem".to_string()]
    }
  } else {
    vec!["cpu".to_string(), "mem".to_string()]
  };

  list
    .into_iter()
    .filter(|item| item == "cpu" || item == "mem")
    .collect()
}

pub fn parse_priority(options: Option<&ComponentOptions>) -> Vec<String> {
  if let Some(opts) = options {
    if let Some(strings) = opts.strings("priority") {
      if !strings.is_empty() {
        return strings.into_iter().map(String::from).collect();
      }
    }
    if let Some(s) = opts.str("priority") {
      if !s.is_empty() {
        return vec![s.to_string()];
      }
    }
  }
  vec!["nowplaying".to_string(), "window".to_string()]
}

pub fn parse_battery_status(options: Option<&ComponentOptions>) -> bool {
  options
    .and_then(|o| {
      o.bool("battery-status").or_else(|| {
        o.str("battery-status")
          .map(|s| s.eq_ignore_ascii_case("true") || s == "1")
      })
    })
    .unwrap_or(false)
}

fn query_nowplaying(store: &Store) -> (Option<(String, String)>, bool) {
  let Some(shared) = store.borrow::<SharedAudioState>() else {
    return (None, false);
  };
  let Ok(player_lock) = shared.player.lock() else {
    return (None, false);
  };
  let Some(player) = player_lock.as_ref() else {
    return (None, false);
  };
  if player.title.is_empty() {
    return (None, false);
  }
  (Some((player.artist.clone(), player.title.clone())), true)
}

fn render_workspaces_dots<'a>(
  config: &'a Config,
  store: &'a Store,
  ctx: &ComponentContext,
) -> Option<Element<'a, ItemMessage>> {
  let state = store.borrow::<CompositorStore>()?.state().ok()?;

  let dot_computed = config.computed(&["notch", "workspaces", "dot"]);
  let active_width = dot_computed.length("width-active").unwrap_or(12.0);
  let inactive_width = dot_computed.length("width-inactive").unwrap_or(6.0);
  let dot_height = dot_computed.length("height").unwrap_or(6.0);
  let dot_gap = dot_computed.length("gap").unwrap_or(4.0);

  let active_style = config.container_style(&["notch", "workspaces", "dot", "active"]);
  let inactive_style = config.container_style(&["notch", "workspaces", "dot"]);

  let dots: Vec<Element<'a, ItemMessage>> = state
    .workspaces
    .iter()
    .filter_map(|ws| {
      if ws.output.as_ref().map_or(false, |out| out != ctx.monitor) {
        return None;
      }
      let is_active = ws.is_active;
      let dot_width = if is_active {
        active_width
      } else {
        inactive_width
      };
      let style = if is_active {
        active_style
      } else {
        inactive_style
      };
      let dot = container(Space::new())
        .width(Length::Fixed(dot_width))
        .height(Length::Fixed(dot_height))
        .style(move |_| style);
      let id = ws.id as u8;
      Some(clickable(dot.into(), move |_, _, _| {
        Some(ItemMessage::Action(ListenerAction::FocusWorkspace(id)))
      }))
    })
    .collect();

  if dots.is_empty() {
    return None;
  }

  Some(row(dots).spacing(dot_gap).align_y(Alignment::Center).into())
}

fn render_window<'a>(
  config: &'a Config,
  store: &'a Store,
  window_stat_key: Option<&str>,
  condense_to_icon: bool,
) -> Option<Element<'a, ItemMessage>> {
  let state = store.borrow::<CompositorStore>()?.state().ok()?;
  let window = state.active_window.as_ref()?;
  slowshell_commons::window::last_active_window().set(window.clone());

  let window_computed = config.computed(&["notch", "window"]);
  let icon_size = window_computed.length("icon-size").unwrap_or(13.0) as u16;

  let icon_elem = window_icon(&window.class, icon_size);

  if condense_to_icon {
    return Some(icon_elem);
  }

  let text_val = if let Some(key) = window_stat_key {
    window
      .metadata
      .get(key)
      .cloned()
      .unwrap_or_else(|| resolve_window_title(window, store))
  } else {
    resolve_window_title(window, store)
  };

  let max_len = 15;
  let truncated = if text_val.chars().count() > max_len {
    format!(
      "{}…",
      text_val.chars().take(max_len - 1).collect::<String>()
    )
  } else {
    text_val
  };

  let label_elem = config.text(truncated, &["notch", "window"]);

  Some(
    row![icon_elem, Space::new().width(4), label_elem]
      .align_y(Alignment::Center)
      .into(),
  )
}

fn resolve_window_title(window: &slowshell_core::Window, store: &Store) -> String {
  if let Some(name) = store
    .borrow::<DesktopEntries>()
    .map(|d| d.resolve_name(&window.class))
  {
    name.to_string()
  } else if !window.class.is_empty() {
    window.class.clone()
  } else if !window.title.is_empty() {
    window.title.clone()
  } else {
    "Desktop".to_string()
  }
}

fn window_icon<'a>(class: &str, size: u16) -> Element<'a, ItemMessage> {
  Icon::any([class, "window", "application-x-executable"])
    .size(size)
    .into()
}

fn render_nowplaying<'a>(
  config: &'a Config,
  artist: &str,
  title: &str,
) -> Element<'a, ItemMessage> {
  let palette = config.palette();
  let np_computed = config.computed(&["notch", "nowplaying"]);

  let name_truncated = if title.chars().count() > 10 {
    format!("{}…", title.chars().take(9).collect::<String>())
  } else {
    title.to_string()
  };

  let text_display = if artist.is_empty() {
    name_truncated
  } else {
    format!("{artist} - {name_truncated}")
  };

  let icon_color = np_computed
    .color("color-primary")
    .or_else(|| np_computed.color("color"))
    .map(|c| c.to_iced())
    .unwrap_or(palette.primary);
  let icon_size = np_computed.length("icon-size").unwrap_or(12.0) as u16;

  let icon = Icon::new("audio-x-generic-symbolic")
    .size(icon_size)
    .color(icon_color);

  let label = config.text(text_display, &["notch", "nowplaying"]);

  row![icon, Space::new().width(4), label]
    .align_y(Alignment::Center)
    .into()
}

fn render_system<'a>(config: &'a Config, displays: &[String]) -> Option<Element<'a, ItemMessage>> {
  if displays.is_empty() {
    return None;
  }

  let palette = config.palette();
  let sys_computed = config.computed(&["notch", "system"]);
  let gauge_size = sys_computed.length("gauge-size").unwrap_or(20.0);
  let cpu_color = sys_computed
    .color("color-cpu")
    .or_else(|| sys_computed.color("color"))
    .map(|c| c.to_iced())
    .unwrap_or(palette.green);
  let mem_color = sys_computed
    .color("color-mem")
    .or_else(|| sys_computed.color("color-secondary"))
    .map(|c| c.to_iced())
    .unwrap_or(palette.blue);
  let track_color = sys_computed
    .color("color-track")
    .or_else(|| sys_computed.color("color-faded"))
    .map(|c| c.to_iced())
    .unwrap_or(Color {
      a: 0.25,
      ..palette.overlay
    });
  let gap = sys_computed.length("gap").unwrap_or(6.0);

  let sys = system_shared();
  let snapshot = sys.snapshot.lock().unwrap();

  let mut gauges: Vec<Element<'a, ItemMessage>> = Vec::new();

  for disp in displays {
    match disp.as_str() {
      "cpu" => {
        let cpu_pct = snapshot.cpu_usage.clamp(0.0, 100.0);
        gauges.push(render_circular_gauge(
          cpu_pct,
          if cpu_pct > 70. {
            palette.red
          } else if cpu_pct > 50. {
            palette.yellow
          } else {
            cpu_color
          },
          track_color,
          "cpu-symbolic",
          gauge_size,
        ));
      }
      "mem" => {
        let mem_pct = snapshot.mem_usage.clamp(0.0, 100.0);
        gauges.push(render_circular_gauge(
          mem_pct,
          if mem_pct > 70. {
            palette.red
          } else if mem_pct > 50. {
            palette.yellow
          } else {
            mem_color
          },
          track_color,
          "memory-symbolic",
          gauge_size,
        ));
      }
      _ => {}
    }
  }

  if gauges.is_empty() {
    return None;
  }

  Some(row(gauges).spacing(gap).align_y(Alignment::Center).into())
}

pub fn render_circular_gauge<'a>(
  pct: f32,
  fg: Color,
  bg: Color,
  icon_name: &'static str,
  size: f32,
) -> Element<'a, ItemMessage> {
  let pct = pct.clamp(0.0, 100.0);
  let fg_hex = format!(
    "#{:02x}{:02x}{:02x}",
    (fg.r * 255.0) as u8,
    (fg.g * 255.0) as u8,
    (fg.b * 255.0) as u8
  );
  let bg_hex = format!(
    "#{:02x}{:02x}{:02x}",
    (bg.r * 255.0) as u8,
    (bg.g * 255.0) as u8,
    (bg.b * 255.0) as u8
  );
  let bg_opacity = 0.25;

  let svg_str = format!(
    r##"<svg viewBox="0 0 36 36" width="{size}" height="{size}" xmlns="http://www.w3.org/2000/svg">
      <path d="M18 2.0845 a 15.9155 15.9155 0 0 1 0 31.831 a 15.9155 15.9155 0 0 1 0 -31.831"
        fill="none" stroke="{bg_hex}" stroke-width="3.5" stroke-opacity="{bg_opacity}" />
      <path d="M18 2.0845 a 15.9155 15.9155 0 0 1 0 31.831 a 15.9155 15.9155 0 0 1 0 -31.831"
        fill="none" stroke="{fg_hex}" stroke-width="3.5" stroke-dasharray="{pct:.1}, 100" stroke-linecap="round" />
    </svg>"##
  );

  let gauge_svg = svg(svg::Handle::from_memory(svg_str.into_bytes()))
    .width(Length::Fixed(size))
    .height(Length::Fixed(size));

  let icon_center = container(
    Icon::any([icon_name, "applications-system-symbolic"])
      .size(9)
      .color(fg),
  )
  .width(Length::Fixed(size))
  .height(Length::Fixed(size))
  .align_x(Alignment::Center)
  .align_y(Alignment::Center);

  iced::widget::stack([gauge_svg.into(), icon_center.into()]).into()
}

fn render_network<'a>(config: &'a Config, store: &'a Store) -> Element<'a, ItemMessage> {
  let palette = config.palette();
  let net_computed = config.computed(&["notch", "network"]);
  let icon_color = net_computed
    .color("color")
    .map(|c| c.to_iced())
    .unwrap_or(palette.subtext);
  let icon_size = net_computed.length("icon-size").unwrap_or(13.0) as u16;

  let net_icon = if let Some(shared_net) = store.borrow::<SharedNetworkState>()
    && let Ok(state) = shared_net.state.lock()
  {
    if let Some(wifi) = &state.wifi_connected {
      signal_icon(wifi.signal)
    } else if let Some(eth) = &state.ethernet
      && eth.connected
    {
      "network-wired-symbolic"
    } else {
      "network-wireless-offline-symbolic"
    }
  } else {
    "network-wireless-signal-excellent-symbolic"
  };

  let net_elem: Element<'a, ItemMessage> =
    Icon::new(net_icon).size(icon_size).color(icon_color).into();

  let has_connected_bt = store
    .borrow::<SharedBluetoothState>()
    .map(|shared| {
      shared.powered.load(Ordering::Relaxed)
        && shared
          .state
          .lock()
          .map(|s| s.devices.iter().any(|d| d.connected))
          .unwrap_or(false)
    })
    .unwrap_or(false);

  if has_connected_bt {
    let bt_elem: Element<'a, ItemMessage> = Icon::new("bluetooth-active-symbolic")
      .size(icon_size)
      .color(icon_color)
      .into();
    row![net_elem, Space::new().width(4), bt_elem]
      .align_y(Alignment::Center)
      .into()
  } else {
    net_elem
  }
}

fn render_clock<'a>(config: &'a Config, fmt: &str) -> Element<'a, ItemMessage> {
  let time_str = chrono::Local::now().format(fmt).to_string();
  config.text(time_str, &["notch", "clock"]).into()
}

#[cfg(feature = "power")]
fn render_battery<'a>(
  config: &'a Config,
  store: &'a Store,
  battery_status: bool,
) -> Option<Element<'a, ItemMessage>> {
  let shared = store
    .borrow::<SharedPowerState>()
    .cloned()
    .unwrap_or_else(slowshell_services::power::shared);
  let Ok(data) = shared.data.lock() else {
    return None;
  };

  let percent = data.percent;
  let charging = data.charging;

  if percent.is_none() && !charging {
    return None;
  }

  let palette = config.palette();
  let bat_computed = config.computed(&["notch", "battery"]);
  let icon_size = bat_computed.length("icon-size").unwrap_or(13.0) as u16;
  let text_color = bat_computed
    .color("color")
    .map(|c| c.to_iced())
    .unwrap_or(palette.subtext);
  let charging_color = bat_computed
    .color("color-charging")
    .or_else(|| bat_computed.color("color-primary"))
    .map(|c| c.to_iced())
    .unwrap_or(palette.primary);
  let danger_color = bat_computed
    .color("color-danger")
    .map(|c| c.to_iced())
    .unwrap_or(palette.red);
  let warning_color = bat_computed
    .color("color-warning")
    .map(|c| c.to_iced())
    .unwrap_or(palette.yellow);

  let icon_name = battery_icon(charging, percent);
  let icon_color = if charging {
    charging_color
  } else if let Some(p) = percent {
    if p <= 20 {
      danger_color
    } else if p <= 40 {
      warning_color
    } else {
      text_color
    }
  } else {
    text_color
  };

  let icon_elem = Icon::new(icon_name).size(icon_size).color(icon_color);

  let elem: Element<'a, ItemMessage> = if battery_status {
    let status_str = match (percent, charging) {
      (Some(p), _) => Some(format!("{p}%")),
      (None, true) => Some("AC".to_string()),
      (None, false) => None,
    };
    if let Some(txt) = status_str {
      let label = config.text(txt, &["notch", "battery"]);
      row![icon_elem, Space::new().width(4), label]
        .align_y(Alignment::Center)
        .into()
    } else {
      icon_elem.into()
    }
  } else {
    icon_elem.into()
  };

  Some(elem)
}

#[cfg(not(feature = "power"))]
fn render_battery<'a>(
  _config: &'a Config,
  _store: &'a Store,
  _battery_status: bool,
) -> Option<Element<'a, ItemMessage>> {
  None
}

#[cfg(feature = "tray")]
fn render_tray<'a>(
  config: &'a Config,
  ctx: &ComponentContext,
  tray_bounds: Arc<Mutex<Option<iced::Rectangle>>>,
) -> Option<Element<'a, ItemMessage>> {
  let items = slowshell_services::tray::shared()
    .state
    .lock()
    .unwrap()
    .items
    .clone();

  if items.is_empty() {
    return None;
  }

  let tray_computed = config.computed(&["notch", "tray"]);
  let color = tray_computed
    .color("color")
    .map(|c| c.to_iced())
    .unwrap_or(config.palette().text);
  let icon_size = tray_computed.length("icon-size").unwrap_or(13.0) as u16;
  let gap = tray_computed.length("gap").unwrap_or(6.0);

  let shared = slowshell_services::tray::shared().clone();
  let position = ctx.position;
  let menu_suppressed = ctx.menu_suppressed;
  let monitor = ctx.monitor.to_ustr();

  let mut tray_elements: Vec<Element<'a, ItemMessage>> = Vec::new();

  for item in items.iter() {
    if let Some(el) = render_tray_item(item, icon_size, color) {
      let shared = shared.clone();
      let address = item.address.clone();
      let menu_path = item.menu_path.clone();
      let monitor = monitor.clone();
      let has_menu = menu_path.is_some();

      let el = clickable(el, move |_event, _layout, cursor| {
        if menu_suppressed {
          return None;
        }
        if has_menu {
          let Some(point) = cursor.position() else {
            return None;
          };
          *shared.selected.lock().unwrap() = Some(address.clone());
          shared.reset_nav();
          if let Some(path) = &menu_path {
            let _ = shared.cmd_tx.send(TrayCmd::AboutToShow {
              address: address.clone(),
              menu_path: path.clone(),
              submenu_id: 0,
            });
          }
          let action = popup_open_action(
            &MenuConfig {
              content: "menus/tray".into(),
              panel_name: None,
              position,
            },
            point,
            &monitor,
          );
          Some(ItemMessage::Action(action))
        } else {
          let _ = shared.cmd_tx.send(TrayCmd::Activate {
            address: address.clone(),
            menu_path: String::new(),
            submenu_id: 0,
          });
          None
        }
      });
      tray_elements.push(el);
    }
  }

  if tray_elements.is_empty() {
    return None;
  }

  let tray_row = row(tray_elements).spacing(gap).align_y(Alignment::Center);

  let tb = tray_bounds.clone();
  Some(
    EventWrapper::new(tray_row.into(), move |_event, layout, _cursor| {
      if let Ok(mut lock) = tb.lock() {
        *lock = Some(layout.bounds());
      }
      None
    })
    .into(),
  )
}

#[cfg(not(feature = "tray"))]
fn render_tray<'a>(
  _config: &'a Config,
  _ctx: &ComponentContext,
  _tray_bounds: Arc<Mutex<Option<iced::Rectangle>>>,
) -> Option<Element<'a, ItemMessage>> {
  None
}

#[cfg(feature = "tray")]
fn render_tray_item<'a>(
  item: &TrayItem,
  size: u16,
  color: Color,
) -> Option<Element<'a, ItemMessage>> {
  if let Some(name) = &item.icon_name {
    let mut icon = Icon::new(name.clone()).size(size);
    if name.contains("symbolic") {
      icon = icon.color(color);
    }
    return Some(icon.into());
  }

  if let Some((w, h, rgba)) = &item.pixmap {
    if *w > 0 && *h > 0 && !rgba.is_empty() {
      let handle = Icon::<ItemMessage>::from_pixmap(&item.address, *w, *h, rgba);
      return Some(
        iced::widget::image(handle)
          .width(Length::Fixed(size as f32))
          .height(Length::Fixed(size as f32))
          .into(),
      );
    }
  }

  None
}

fn render_notch_status<'a>(config: &'a Config, status: &NotchStatus) -> Element<'a, ItemMessage> {
  let computed = config.computed(&["notch", "status"]);
  let icon_size = computed.length("icon-size").unwrap_or(13.0) as u16;
  let color = computed
    .color("color")
    .map(|c| c.to_iced())
    .unwrap_or(config.palette().text);

  match status {
    NotchStatus::Icon(icon) => Icon::new(icon.clone()).size(icon_size).color(color).into(),
    NotchStatus::Text(text) => config.text(text.clone(), &["notch", "status"]).into(),
    NotchStatus::IconText(icon, text) => row![
      Icon::new(icon.clone()).size(icon_size).color(color),
      Space::new().width(4),
      config.text(text.clone(), &["notch", "status"])
    ]
    .align_y(Alignment::Center)
    .into(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use slowshell_config::Value;
  use std::collections::HashMap;

  #[test]
  fn test_parse_notch_elements() {
    let mut map = HashMap::new();
    map.insert(
      "notch".into(),
      Value::Array(vec![
        Value::Str("workspaces".into()),
        Value::Str("window".into()),
        Value::Str("clock".into()),
      ]),
    );
    let options = ComponentOptions::from(map);
    let elements = parse_notch_elements(Some(&options));
    assert_eq!(elements, vec!["workspaces", "window", "clock"]);
  }

  #[test]
  fn test_parse_system_displays_only_cpu_mem() {
    let mut map = HashMap::new();
    map.insert(
      "system-display".into(),
      Value::Array(vec![
        Value::Str("cpu".into()),
        Value::Str("disk".into()),
        Value::Str("mem".into()),
        Value::Str("gpu".into()),
      ]),
    );
    let options = ComponentOptions::from(map);
    let displays = parse_system_displays(Some(&options));
    assert_eq!(displays, vec!["cpu", "mem"]);
  }

  #[test]
  fn test_parse_clock_format() {
    let mut map = HashMap::new();
    map.insert("clock-format".into(), Value::Str("%I:%M %p".into()));
    let options = ComponentOptions::from(map);
    assert_eq!(parse_clock_format(Some(&options)), "%I:%M %p");

    let empty_map = HashMap::new();
    let empty_options = ComponentOptions::from(empty_map);
    assert_eq!(parse_clock_format(Some(&empty_options)), "%H:%M");
  }

  #[test]
  fn test_parse_priority() {
    let mut map = HashMap::new();
    map.insert(
      "priority".into(),
      Value::Array(vec![
        Value::Str("window".into()),
        Value::Str("nowplaying".into()),
      ]),
    );
    let options = ComponentOptions::from(map);
    let priority = parse_priority(Some(&options));
    assert_eq!(priority, vec!["window", "nowplaying"]);
  }

  #[test]
  fn test_notch_status_store_update_and_remove() {
    let store = slowshell_commons::notch::shared();
    store.update_status(
      "vpn",
      NotchStatus::IconText("vpn".into(), "Connected".into()),
    );
    assert_eq!(
      store.get("vpn"),
      Some(NotchStatus::IconText("vpn".into(), "Connected".into()))
    );

    store.remove_status("vpn");
    assert_eq!(store.get("vpn"), None);
  }

  #[test]
  fn test_notch_notification_entry() {
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let notif_state = Arc::new(slowshell_commons::notifications::NotificationState {
      dnd: std::sync::atomic::AtomicBool::new(false),
      state: Mutex::new(Arc::new(Default::default())),
      ui: Mutex::new(Arc::new(Default::default())),
      revision: std::sync::atomic::AtomicU64::new(0),
      cmd_tx: tx,
      notify_fd: std::sync::atomic::AtomicI32::new(-1),
    });
    let item = slowshell_commons::notifications::NotificationItem {
      id: 42,
      app_name: "Slack".to_string(),
      app_icon: Some("slack".to_string()),
      image: None,
      summary: "Direct Message".to_string(),
      body: "Hey there!".to_string(),
      actions: vec![],
      urgency: 1,
      created_at: std::time::Instant::now(),
      plugin: None,
    };
    notif_state.push(item, 5000);

    let guard = notif_state.state.lock().unwrap();
    let popup = guard.popups.last().unwrap();
    assert_eq!(popup.summary, "Direct Message");
    assert_eq!(popup.app_icon.as_deref(), Some("slack"));
  }

  #[test]
  fn test_render_network_with_and_without_bluetooth() {
    let config = Config::default();

    {
      let store = Store::new();
      let _net_el = render_network(&config, &store);
    }

    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let bt_state = Arc::new(slowshell_commons::bluetooth::BluetoothState {
      powered: std::sync::atomic::AtomicBool::new(true),
      discovering: std::sync::atomic::AtomicBool::new(false),
      state: Mutex::new(Arc::new(slowshell_commons::bluetooth::BluetoothInner {
        adapter_name: Some("hci0".into()),
        devices: vec![slowshell_commons::bluetooth::BluetoothDevice {
          address: "00:11:22:33:44:55".into(),
          name: "Headphones".into(),
          connected: false,
          ..Default::default()
        }],
        ..Default::default()
      })),
      menu: Mutex::new(Default::default()),
      revision: std::sync::atomic::AtomicU64::new(0),
      cmd_tx: tx,
    });

    {
      let mut store = Store::new();
      store.insert(bt_state.clone());
      let _net_without_connected_bt = render_network(&config, &store);
    }

    *bt_state.state.lock().unwrap() = Arc::new(slowshell_commons::bluetooth::BluetoothInner {
      adapter_name: Some("hci0".into()),
      devices: vec![slowshell_commons::bluetooth::BluetoothDevice {
        address: "00:11:22:33:44:55".into(),
        name: "Headphones".into(),
        connected: true,
        ..Default::default()
      }],
      ..Default::default()
    });

    {
      let mut store = Store::new();
      store.insert(bt_state.clone());
      let _net_with_connected_bt = render_network(&config, &store);
    }
  }

  #[test]
  fn test_parse_notch_elements_contains_battery() {
    let elements = parse_notch_elements(None);
    assert!(elements.contains(&"battery".to_string()));
  }

  #[test]
  fn test_parse_battery_status() {
    assert!(!parse_battery_status(None));

    let mut opts_map = std::collections::HashMap::new();
    opts_map.insert("battery-status".into(), slowshell_config::Value::Bool(true));
    let opts = ComponentOptions::from(opts_map);
    assert!(parse_battery_status(Some(&opts)));

    let mut opts_map_false = std::collections::HashMap::new();
    opts_map_false.insert("battery-status".into(), slowshell_config::Value::Bool(false));
    let opts_false = ComponentOptions::from(opts_map_false);
    assert!(!parse_battery_status(Some(&opts_false)));

    let mut opts_map_str = std::collections::HashMap::new();
    opts_map_str.insert("battery-status".into(), slowshell_config::Value::Str("true".into()));
    let opts_str = ComponentOptions::from(opts_map_str);
    assert!(parse_battery_status(Some(&opts_str)));
  }

  #[test]
  fn test_render_battery_charging_and_discharging() {
    let config = Config::default();

    #[cfg(feature = "power")]
    {
      let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel();
      let power_state = Arc::new(slowshell_commons::power::PowerState {
        data: Mutex::new(Arc::new(slowshell_commons::power::PowerData {
          percent: Some(85),
          charging: true,
          ..Default::default()
        })),
        revision: std::sync::atomic::AtomicU64::new(0),
        cmd_tx: cmd_tx.clone(),
      });

      let mut store = Store::new();
      store.insert(power_state.clone());

      let bat_el_icon_only = render_battery(&config, &store, false);
      assert!(bat_el_icon_only.is_some());

      let bat_el_with_status = render_battery(&config, &store, true);
      assert!(bat_el_with_status.is_some());

      // When there is no battery and not charging, returns None
      *power_state.data.lock().unwrap() = Arc::new(slowshell_commons::power::PowerData {
        percent: None,
        charging: false,
        ..Default::default()
      });
      let no_bat_el = render_battery(&config, &store, false);
      assert!(no_bat_el.is_none());
    }
  }
}
