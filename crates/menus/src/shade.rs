use std::{
  collections::HashMap,
  sync::{Arc, Mutex},
};

use iced::{
  Alignment, Color, Element, Length,
  widget::{Space, column, container, row, scrollable},
};
use iced_exwlshell::reexport::IcedId;
use slowshell_commons::{
  audio::{AUDIO_CHANGED, SharedAudioState},
  bluetooth::BT_CHANGED,
  desktop::DesktopEntries,
  network::NET_CHANGED,
  notifications::{NOTIF_CHANGED, SharedNotificationState},
  popups::{PopupPosition, PopupSettings},
  power::POWER_CHANGED,
  quicksettings::QuickSettingsRegistry,
  system::shared as system_shared,
};
use slowshell_compositor::CompositorStore;
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::{CompositorEvent, ListenerAction},
  message::{EventFilter, ItemEffect, ItemMessage},
  types::{PayloadBox, ToUstr},
};
use slowshell_widgets::{Backdrop, Icon, Renderable, SizedPopup, SlideIn, clickable};

use crate::quicksettings::{handle_tile_expand, render_quicksettings_content, render_user_header};

pub struct ShadeRenderable {
  expanded: Arc<Mutex<Option<String>>>,
  slots: Arc<Mutex<HashMap<usize, String>>>,
}

impl Default for ShadeRenderable {
  fn default() -> Self {
    Self {
      expanded: Arc::new(Mutex::new(None)),
      slots: Arc::new(Mutex::new(HashMap::new())),
    }
  }
}

impl Renderable for ShadeRenderable {
  fn events(&self, store: &Store) -> Vec<EventFilter> {
    let mut events = store
      .borrow::<QuickSettingsRegistry>()
      .map(|registry| registry.events())
      .unwrap_or_default();
    events.push(EventFilter::Named(AUDIO_CHANGED.into()));
    events.push(EventFilter::Named(POWER_CHANGED.into()));
    events.push(EventFilter::Named(NET_CHANGED.into()));
    events.push(EventFilter::Named(BT_CHANGED.into()));
    events.push(EventFilter::Named(NOTIF_CHANGED.into()));
    events.push(EventFilter::Compositor(
      CompositorEvent::WINDOWS | CompositorEvent::FOCUS,
    ));
    events.push(EventFilter::Named("compositor.focus".into()));
    events.push(EventFilter::Tick);
    events
  }

  fn handle_message(&mut self, message: &ItemMessage) -> Option<ItemEffect> {
    handle_tile_expand(&self.expanded, &self.slots, message)
  }

  fn initialize(&self, _config: &Config, store: &mut Store) -> Box<dyn std::any::Any + Send> {
    if let Some(comp) = store.borrow::<CompositorStore>()
      && let Ok(state) = comp.state()
      && let Some(win) = &state.active_window
    {
      slowshell_commons::window::last_active_window().set(win.clone());
    }
    Box::new(())
  }

  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    data: &'a dyn std::any::Any,
  ) -> Element<'a, ItemMessage> {
    let data = data.downcast_ref::<PopupSettings>().unwrap();

    let shade_computed = config.computed(&["menu", "shade", "popup"]);
    let width = shade_computed.length("width").unwrap_or(440.0);
    let height = shade_computed.length("height").unwrap_or(620.0);
    let pad_v = shade_computed.length("padding-v").unwrap_or(10.0);
    let pad_h = shade_computed.length("padding-h").unwrap_or(14.0);
    let gap = shade_computed.length("gap").unwrap_or(10.0);

    let shade_style = config.container_style(&["menu", "shade", "popup"]);

    let mut children: Vec<Element<'a, ItemMessage>> = Vec::new();

    children.push(render_user_header(config));

    children.push(render_calendar_btn(config, id, data));

    children.push(render_quick_nav_bar(config, store, id, data));

    children.push(render_active_window_card(config, store, id, data));

    children.push(render_sysmon_card(config, store, id, data));

    if let Some(media_card) = render_media_card(config, store, id, data) {
      children.push(media_card);
    }

    let qs_expanded = self.expanded.lock().ok().and_then(|x| x.clone());
    let qs_elements = render_quicksettings_content(
      config,
      store,
      id,
      qs_expanded.as_deref(),
      &self.slots,
      Some(&PopupSettings {
        position: (
          data.position.0.offset_map(|o| o - (o * 0.2)),
          data.position.1.clone(),
        ),
        ..data.clone()
      }),
    );
    for qs_elem in qs_elements.into_iter().skip(1) {
      children.push(qs_elem);
    }

    let content_col = column(children).spacing(gap).width(Length::Fill);

    let scroll = scrollable(content_col)
      .height(Length::Fill)
      .width(Length::Fill);

    let main_container = container(scroll)
      .width(Length::Fixed(width))
      .height(Length::Fixed(height))
      .padding([pad_v, pad_h])
      .style(move |_| shade_style);

    let animated = SlideIn::from_edge(main_container, icss::widgets::animation::Edge::Top, 40.0);

    SizedPopup::new(animated)
      .with_width(width)
      .with_backdrop(Backdrop::transparent().with_close_on_click(true))
      .with_position(data.position.0.get(store), data.position.1.get(store))
      .with_panel_edge(data.panel_edge)
      // .with_panel_insets(store, data.monitor.as_deref())
      .with_on_close(ItemMessage::Effect(id, ItemEffect::Hide))
      .into()
  }
}

fn menu_open_message(id: IcedId, content: String, settings: Option<&PopupSettings>) -> ItemMessage {
  ItemMessage::EffectAction(
    id,
    ItemEffect::Hide,
    ListenerAction::Payload {
      name: "popup.open".to_ustr(),
      payload: Some(PayloadBox::new(
        settings
          .map(|p| PopupSettings {
            content: content.to_ustr(),
            position: (
              p.position.0.offset_map(|o| o - (o * 0.2)),
              p.position.1.clone(),
            ),
            ..p.clone()
          })
          .unwrap_or(PopupSettings {
            content: content.to_ustr(),
            panel_edge: None,
            position: (PopupPosition::Fixed(0.0), PopupPosition::Fixed(0.0)),
            insets: false,
            monitor: None,
          }),
      )),
    },
  )
}

fn render_calendar_btn<'a>(
  config: &'a Config,
  id: IcedId,
  data: &PopupSettings,
) -> Element<'a, ItemMessage> {
  let date_str = chrono::Local::now().format("%A, %B %-d").to_string();

  let computed = config.computed(&["menu", "shade", "card", "calendar"]);
  let pad_v = computed.length("padding-v").unwrap_or(5.0);
  let pad_h = computed.length("padding-h").unwrap_or(8.0);

  let btn = container(config.text(date_str, &["menu", "text-status", "text-header"]))
    .padding([pad_v, pad_h])
    .width(Length::Fill);

  let open_clock = menu_open_message(id, "menus/clock".into(), Some(data));
  clickable(btn.into(), move |_, _, _| Some(open_clock.clone()))
}

fn render_quick_nav_bar<'a>(
  config: &'a Config,
  store: &'a Store,
  id: IcedId,
  data: &PopupSettings,
) -> Element<'a, ItemMessage> {
  let palette = config.palette();

  let unread_count = store
    .borrow::<SharedNotificationState>()
    .and_then(|s| s.state.lock().ok().map(|g| g.unread_count))
    .unwrap_or(0);
  let notif_label = if unread_count > 0 {
    format!("{unread_count} new")
  } else {
    "Notifications".to_string()
  };
  let notif_icon_color = if unread_count > 0 {
    palette.yellow
  } else {
    palette.text
  };
  let notif_btn = nav_tile(
    "preferences-system-notifications-symbolic",
    notif_label,
    notif_icon_color,
    config,
    id,
    "menus/notifications",
    data,
  );

  let sound_btn = nav_tile(
    "audio-volume-high-symbolic",
    "Sound".to_string(),
    palette.primary,
    config,
    id,
    "menus/sound",
    data,
  );

  let power_btn = nav_tile(
    "system-shutdown-symbolic",
    "Power".to_string(),
    palette.red,
    config,
    id,
    "menus/power",
    data,
  );

  row![
    container(notif_btn).width(Length::FillPortion(1)),
    container(sound_btn).width(Length::FillPortion(1)),
    container(power_btn).width(Length::FillPortion(1)),
  ]
  .spacing(6)
  .width(Length::Fill)
  .into()
}

fn nav_tile<'a>(
  icon_name: &'static str,
  label: String,
  icon_color: Color,
  config: &'a Config,
  id: IcedId,
  target_menu: &'static str,
  data: &PopupSettings,
) -> Element<'a, ItemMessage> {
  let content = row![
    Icon::new(icon_name).size(13).color(icon_color),
    Space::new().width(5),
    config.text(label, &["menu", "text-status"]),
  ]
  .align_y(Alignment::Center);

  let computed = config.computed(&["menu", "shade", "nav-tile"]);
  let pad_v = computed.length("padding-v").unwrap_or(6.0);
  let pad_h = computed.length("padding-h").unwrap_or(8.0);
  let tile_style = config.container_style(&["menu", "shade", "nav-tile"]);

  let btn = container(content)
    .padding([pad_v, pad_h])
    .center_x(Length::Fill)
    .style(move |_| tile_style);

  let open_msg = menu_open_message(id, target_menu.to_string(), Some(data));
  clickable(btn.into(), move |_, _, _| Some(open_msg.clone()))
}

fn render_active_window_card<'a>(
  config: &'a Config,
  store: &'a Store,
  id: IcedId,
  data: &PopupSettings,
) -> Element<'a, ItemMessage> {
  let palette = config.palette();

  let active_window = if let Some(comp) = store.borrow::<CompositorStore>()
    && let Ok(state) = comp.state()
  {
    if let Some(win) = &state.active_window {
      slowshell_commons::window::last_active_window().set(win.clone());
      Some(win.clone())
    } else if let Some(last) = slowshell_commons::window::last_active_window().get() {
      if let Some(updated) = state.active_windows.iter().find(|w| w.id == last.id) {
        Some(updated.clone())
      } else {
        Some(last)
      }
    } else {
      state.active_windows.iter().find(|w| w.is_active).cloned()
    }
  } else {
    slowshell_commons::window::last_active_window().get()
  };

  let (app_name, title, class) = if let Some(win) = &active_window {
    let name = if let Some(entries) = store.borrow::<DesktopEntries>() {
      entries.resolve_name(&win.class).to_string()
    } else if !win.class.is_empty() {
      win.class.clone()
    } else {
      "Application".to_string()
    };
    let title_str = if win.title.is_empty() {
      name.clone()
    } else {
      win.title.clone()
    };
    (name, title_str, win.class.clone())
  } else {
    (
      "Desktop".to_string(),
      "Workspace Overview".to_string(),
      "user-desktop".to_string(),
    )
  };

  let title_truncated = if title.chars().count() > 36 {
    format!("{}…", title.chars().take(35).collect::<String>())
  } else {
    title
  };

  let icon_elem = Icon::any([class.as_str(), "window", "application-x-executable"]).size(24);

  let details = column![
    config.text(app_name, &["menu", "text-header"]),
    config.text(title_truncated, &["menu", "text-status", "text-faded"]),
  ]
  .spacing(1)
  .width(Length::Fill);

  let content = row![
    icon_elem,
    Space::new().width(10),
    details,
    Icon::new("go-next-symbolic")
      .size(12)
      .color(palette.subtext),
  ]
  .align_y(Alignment::Center)
  .width(Length::Fill);

  let computed = config.computed(&["menu", "shade", "card"]);
  let pad_v = computed.length("padding-v").unwrap_or(8.0);
  let pad_h = computed.length("padding-h").unwrap_or(10.0);
  let card_style = config.container_style(&["menu", "shade", "card"]);

  let card = container(content)
    .padding([pad_v, pad_h])
    .width(Length::Fill)
    .style(move |_| card_style);

  let open_win = menu_open_message(id, "menus/window".into(), Some(data));
  clickable(card.into(), move |_, _, _| Some(open_win.clone()))
}

fn render_sysmon_card<'a>(
  config: &'a Config,
  _store: &'a Store,
  id: IcedId,
  data: &PopupSettings,
) -> Element<'a, ItemMessage> {
  let palette = config.palette();
  let sys = system_shared();
  let snapshot = sys.snapshot.lock().unwrap();

  let cpu_pct = snapshot.cpu_usage.clamp(0.0, 100.0).round() as u32;
  let mem_used_gb = (snapshot.mem_used as f32) / (1024.0 * 1024.0 * 1024.0);
  let mem_total_gb = (snapshot.mem_total as f32) / (1024.0 * 1024.0 * 1024.0);

  let badges: Vec<String> = vec![
    format!("CPU {cpu_pct}%"),
    format!("RAM {mem_used_gb:.1}/{mem_total_gb:.1} GB"),
  ];

  let stats_text = badges.join("  •  ");

  let content = row![
    Icon::new("utilities-system-monitor-symbolic")
      .size(22)
      .color(palette.green),
    Space::new().width(10),
    column![
      config.text("System Monitor", &["menu", "text-header"]),
      config.text(stats_text, &["menu", "text-status", "text-faded"]),
    ]
    .spacing(1)
    .width(Length::Fill),
    Icon::new("go-next-symbolic")
      .size(12)
      .color(palette.subtext),
  ]
  .align_y(Alignment::Center)
  .width(Length::Fill);

  let computed = config.computed(&["menu", "shade", "card"]);
  let pad_v = computed.length("padding-v").unwrap_or(8.0);
  let pad_h = computed.length("padding-h").unwrap_or(10.0);
  let card_style = config.container_style(&["menu", "shade", "card"]);

  let card = container(content)
    .padding([pad_v, pad_h])
    .width(Length::Fill)
    .style(move |_| card_style);

  let open_sys = menu_open_message(id, "menus/system".into(), Some(data));
  clickable(card.into(), move |_, _, _| Some(open_sys.clone()))
}

fn render_media_card<'a>(
  config: &'a Config,
  store: &'a Store,
  id: IcedId,
  data: &PopupSettings,
) -> Option<Element<'a, ItemMessage>> {
  let shared = store.borrow::<SharedAudioState>()?;
  let (title, artist, status) = {
    let player_lock = shared.player.lock().ok()?;
    let player = player_lock.as_ref()?;
    if player.title.is_empty() {
      return None;
    }
    (
      player.title.clone(),
      player.artist.clone(),
      player.playback_status.clone(),
    )
  };

  let palette = config.palette();

  let is_playing = status.eq_ignore_ascii_case("playing");
  let play_icon = if is_playing {
    "media-playback-pause-symbolic"
  } else {
    "media-playback-start-symbolic"
  };

  let title_text = config.text(title, &["menu", "text-header"]);
  let artist_display = if artist.is_empty() {
    "Unknown Artist".to_string()
  } else {
    artist
  };
  let artist_text = config.text(artist_display, &["menu", "text-status", "text-faded"]);

  let track_info = column![title_text, artist_text].spacing(1);

  let prev_btn = media_control_btn("media-skip-backward-symbolic", "audio.previous", config);
  let play_btn = media_control_btn(play_icon, "audio.play_pause", config);
  let next_btn = media_control_btn("media-skip-forward-symbolic", "audio.next", config);

  let controls = row![prev_btn, play_btn, next_btn]
    .spacing(6)
    .align_y(Alignment::Center);

  let top_media_content = row![
    Icon::new("audio-x-generic-symbolic")
      .size(24)
      .color(palette.primary),
    Space::new().width(10),
    container(track_info).width(Length::Fill),
    controls,
  ]
  .align_y(Alignment::Center)
  .width(Length::Fill);

  let open_sound = menu_open_message(id, "menus/sound".into(), Some(data));
  let more_link = row![
    config.text("Sound settings", &["menu", "text-status", "text-faded"]),
    Space::new().width(2),
    Icon::new("go-next-symbolic")
      .size(10)
      .color(palette.subtext),
  ]
  .align_y(Alignment::Center);

  let more_comp = config.computed(&["menu", "shade", "more-btn"]);
  let more_pad_v = more_comp.length("padding-v").unwrap_or(2.0);
  let more_pad_h = more_comp.length("padding-h").unwrap_or(4.0);
  let more_style = config.container_style(&["menu", "shade", "more-btn"]);

  let more_btn = clickable(
    container(more_link)
      .padding([more_pad_v, more_pad_h])
      .style(move |_| more_style)
      .into(),
    move |_, _, _| Some(open_sound.clone()),
  );

  let bottom_row = row![
    config.text(
      if is_playing { "Playing" } else { "Paused" },
      &["menu", "text-status", "text-faded"]
    ),
    Space::new().width(Length::Fill),
    more_btn,
  ]
  .align_y(Alignment::Center)
  .width(Length::Fill);

  let card_content = column![top_media_content, bottom_row].spacing(6);

  let card_comp = config.computed(&["menu", "shade", "card"]);
  let card_pad_v = card_comp.length("padding-v").unwrap_or(8.0);
  let card_pad_h = card_comp.length("padding-h").unwrap_or(10.0);
  let card_style = config.container_style(&["menu", "shade", "card"]);

  Some(
    container(card_content)
      .padding([card_pad_v, card_pad_h])
      .width(Length::Fill)
      .style(move |_| card_style)
      .into(),
  )
}

fn media_control_btn<'a>(
  icon_name: &'static str,
  command: &'static str,
  config: &'a Config,
) -> Element<'a, ItemMessage> {
  let palette = config.palette();
  let text_color = palette.text;

  let computed = config.computed(&["menu", "shade", "media-btn"]);
  let pad_v = computed.length("padding-v").unwrap_or(4.0);
  let pad_h = computed.length("padding-h").unwrap_or(6.0);
  let btn_style = config.container_style(&["menu", "shade", "media-btn"]);

  let btn = container(Icon::new(icon_name).size(14).color(text_color))
    .padding([pad_v, pad_h])
    .style(move |_| btn_style);

  clickable(btn.into(), move |_, _, _| {
    Some(ItemMessage::Action(ListenerAction::Payload {
      name: command.to_ustr(),
      payload: None,
    }))
  })
}
