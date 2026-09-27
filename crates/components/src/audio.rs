use std::{
  os::fd::AsRawFd,
  sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
  },
};

use iced::{
  Element, Event, mouse,
  widget::{container, row},
};
use nix::{
  fcntl::{FcntlArg, OFlag, fcntl},
  sys::epoll::EpollFlags,
};
use slowshell_commons::audio::{
  AUDIO_CHANGED, AudioCmd, AudioState, MprisSelection, SharedAudioState, volume_icon,
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::{FdHandle, ListenerAction},
  message::{EventFilter, ItemEffect, ItemMessage},
  persistence,
  types::{OptionalPayloadBox, PayloadBox},
};
use slowshell_services::util::drain_signal_fd;
use slowshell_widgets::{EventWrapper, Icon};

use crate::{
  Component, ComponentContext, ComponentOptions, MenuConfig, popup_open_action, spaced_component,
};

const PRIMARY_PLAYER_FILE: &str = "audio-primary-player";

struct GlobalAudio {
  shared: SharedAudioState,
  rx: Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<AudioCmd>>>,
}

static GLOBAL: OnceLock<GlobalAudio> = OnceLock::new();

fn global() -> &'static GlobalAudio {
  GLOBAL.get_or_init(|| {
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();

    let selected = persistence::load::<(String, String)>(PRIMARY_PLAYER_FILE)
      .map(|(bus_name, identity)| MprisSelection { bus_name, identity });

    GlobalAudio {
      shared: Arc::new(AudioState {
        state: Mutex::new(Default::default()),
        player: Mutex::new(None),
        players: Mutex::new(Vec::new()),
        selected: Mutex::new(selected),
        revision: AtomicU64::new(0),
        cmd_tx,
      }),
      rx: Mutex::new(Some(cmd_rx)),
    }
  })
}

pub fn ensure(store: &mut Store) -> SharedAudioState {
  let shared = global().shared.clone();
  store.insert(shared.clone());

  let Some(cmd_rx) = global().rx.lock().unwrap().take() else {
    return shared;
  };

  let notify = store.borrow::<FdHandle>().and_then(|handle| {
    let (read_fd, write_fd) = nix::unistd::pipe().ok()?;
    fcntl(&read_fd, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).ok()?;

    let raw = read_fd.as_raw_fd();
    handle.watch_with_flags(
      read_fd,
      EpollFlags::EPOLLIN | EpollFlags::EPOLLET,
      ListenerAction::Signal {
        name: AUDIO_CHANGED.into(),
        fd: raw,
      },
    );

    Some(write_fd)
  });

  slowshell_services::audio::run(shared.clone(), cmd_rx, notify);

  shared
}

pub struct Audio {
  shared: SharedAudioState,
  last_revision: u64,
  shared_in_store: bool,
}

impl Audio {
  pub fn new() -> Self {
    Self {
      shared: global().shared.clone(),
      last_revision: 0,
      shared_in_store: false,
    }
  }
}

impl Default for Audio {
  fn default() -> Self {
    Self::new()
  }
}

impl Component for Audio {
  fn events(&self) -> Vec<EventFilter> {
    vec![
      EventFilter::Named(AUDIO_CHANGED.into()),
      EventFilter::Payload("audio.step".into()),
      EventFilter::Payload("audio.toggle_mute".into()),
      EventFilter::Payload("audio.set_volume".into()),
      EventFilter::Payload("audio.set_sink".into()),
      EventFilter::Payload("audio.play_pause".into()),
      EventFilter::Payload("audio.next".into()),
      EventFilter::Payload("audio.previous".into()),
      EventFilter::Payload("audio.select_player".into()),
    ]
  }

  fn watch(&mut self, store: &mut Store, _options: Option<&ComponentOptions>) {
    self.shared = ensure(store);
    self.shared_in_store = true;
  }

  fn update(
    &mut self,
    _config: &Config,
    store: &mut Store,
    event: &ListenerAction,
    _options: Option<&ComponentOptions>,
  ) -> miette::Result<ItemEffect> {
    if !self.shared_in_store {
      self.shared_in_store = true;
      store.insert(self.shared.clone());
    }

    match event {
      ListenerAction::Signal { name, fd } if &**name == AUDIO_CHANGED => {
        drain_signal_fd(*fd);
        let rev = self.shared.revision.load(Ordering::SeqCst);
        if rev != self.last_revision {
          self.last_revision = rev;
          Ok(ItemEffect::Redraw)
        } else {
          Ok(ItemEffect::None)
        }
      }

      ListenerAction::Named(name) if &**name == AUDIO_CHANGED => {
        let rev = self.shared.revision.load(Ordering::SeqCst);
        if rev != self.last_revision {
          self.last_revision = rev;
          Ok(ItemEffect::Redraw)
        } else {
          Ok(ItemEffect::None)
        }
      }

      ListenerAction::Payload { name, payload } if &**name == "audio.step" => {
        if let Some(delta) = payload.transform::<f32>() {
          let _ = self.shared.cmd_tx.send(AudioCmd::StepVolume(*delta));
        }
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, .. } if &**name == "audio.toggle_mute" => {
        let _ = self.shared.cmd_tx.send(AudioCmd::ToggleMute);
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, payload } if &**name == "audio.set_volume" => {
        if let Some(vol) = payload.transform::<f32>() {
          let _ = self.shared.cmd_tx.send(AudioCmd::SetVolume(*vol));
        }
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, payload } if &**name == "audio.set_sink" => {
        if let Some(id) = payload.transform::<u32>() {
          let _ = self.shared.cmd_tx.send(AudioCmd::SetDefaultSink(*id));
        }
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, .. } if &**name == "audio.play_pause" => {
        let _ = self.shared.cmd_tx.send(AudioCmd::PlayPause);
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, .. } if &**name == "audio.next" => {
        let _ = self.shared.cmd_tx.send(AudioCmd::Next);
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, .. } if &**name == "audio.previous" => {
        let _ = self.shared.cmd_tx.send(AudioCmd::Previous);
        Ok(ItemEffect::Redraw)
      }

      ListenerAction::Payload { name, payload } if &**name == "audio.select_player" => {
        if let Some(selection) = payload.transform::<MprisSelection>() {
          let selection = selection.clone();
          let _ = self
            .shared
            .cmd_tx
            .send(AudioCmd::SelectPlayer(selection.clone()));

          if let Err(e) = persistence::save(
            PRIMARY_PLAYER_FILE,
            &(selection.bus_name, selection.identity),
          ) {
            eprintln!("[audio] failed to persist the primary player: {e}");
          }
        }
        Ok(ItemEffect::Redraw)
      }

      _ => Ok(ItemEffect::None),
    }
  }

  fn view<'a>(
    &'a self,
    config: &'a Config,
    _store: &'a Store,
    ctx: &ComponentContext,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let style = config.computed(&["component"]);
    let theme = config.palette();

    let icon_size = style.length("icon-size").unwrap_or(14.0) as u16;

    let text_color = style
      .color("color")
      .map(|c| c.to_iced())
      .unwrap_or(theme.text);
    let faded = style
      .color("color-faded")
      .map(|c| c.to_iced())
      .unwrap_or(theme.subtext);
    let primary = style
      .color("color-primary")
      .map(|c| c.to_iced())
      .unwrap_or(theme.primary);

    let (vol, muted) = {
      let state = self.shared.state.lock().unwrap();
      (state.volume, state.muted)
    };

    let track_info = {
      let player = self.shared.player.lock().unwrap();
      player.as_ref().and_then(|p| {
        if p.playback_status == "Playing" || p.playback_status == "Paused" {
          if !p.title.is_empty() {
            let label = if !p.artist.is_empty() {
              format!("{} - {}", p.artist, p.title)
            } else {
              p.title.clone()
            };
            Some((label, p.playback_status == "Playing"))
          } else {
            None
          }
        } else {
          None
        }
      })
    };

    let show_track = options.and_then(|o| o.bool("show-track")).unwrap_or(false);
    let show_percent = options
      .and_then(|o| o.bool("show-percent"))
      .unwrap_or(false);

    let icon_name = volume_icon(vol, muted);
    let color = if muted { faded } else { text_color };

    let icon = Icon::new(icon_name).size(icon_size).color(color);

    let mut items = vec![icon.into()];

    if show_track && track_info.is_some() {
      let (track_label, is_playing) = track_info.unwrap();
      let track_color = if is_playing { primary } else { faded };

      let truncated: String = if track_label.chars().count() > 30 {
        format!("{}…", track_label.chars().take(28).collect::<String>())
      } else {
        track_label
      };

      items.push(
        config
          .text(truncated, &["component"])
          .color(track_color)
          .into(),
      );
    } else if show_percent {
      let pct_str = format!("{}%", (vol * 100.0).round() as u32);
      items.push(config.text(pct_str, &["component"]).color(color).into());
    }

    let content = container(row(items).spacing(4).align_y(iced::Alignment::Center));

    let menu_config = MenuConfig {
      content: "menus/sound".into(),
      panel_name: None,
      position: ctx.position,
    };

    let menu_suppressed = ctx.menu_suppressed;

    let action =
      move |event: &Event, layout: iced::advanced::layout::Layout<'_>, cursor: mouse::Cursor| {
        if !cursor.is_over(layout.bounds()) {
          return None;
        }

        // TODO: Fix audio scroll and slider
        match event {
          Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
            let y = match delta {
              mouse::ScrollDelta::Lines { y, .. } => *y,
              mouse::ScrollDelta::Pixels { y, .. } => *y / 20.0,
            };
            let step = if y > 0.0 { 0.05 } else { -0.05 };
            Some(ItemMessage::Action(ListenerAction::Payload {
              name: "audio.step".into(),
              payload: Some(PayloadBox::new(step as f32)),
            }))
          }
          Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
            Some(ItemMessage::Action(ListenerAction::Payload {
              name: "audio.toggle_mute".into(),
              payload: None,
            }))
          }
          Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if !menu_suppressed => {
            let point = cursor.position()?;
            Some(ItemMessage::Action(popup_open_action(&menu_config, point)))
          }
          _ => None,
        }
      };

    EventWrapper::new(spaced_component(config, ctx, content.into(), true), action).into()
  }
}
