use iced::{
  Element,
  widget::{column, container, row, text},
};

use slowshell_commons::{
  audio::{AUDIO_CHANGED, SharedAudioState},
  panels::PanelOrientation,
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{EventFilter, ItemEffect, ItemMessage},
};

use slowshell_services::util::drain_signal_fd;
use slowshell_widgets::{Icon, clickable};

use crate::{Component, ComponentContext, ComponentOptions, MenuConfig, menu_trigger};

pub struct NowPlaying;

impl NowPlaying {
  pub fn new() -> Self {
    Self
  }
}

impl Default for NowPlaying {
  fn default() -> Self {
    Self::new()
  }
}

impl Component for NowPlaying {
  fn events(&self) -> Vec<EventFilter> {
    vec![EventFilter::Named(AUDIO_CHANGED.into())]
  }

  fn update(
    &mut self,
    _config: &Config,
    _store: &mut Store,
    event: &ListenerAction,
    _options: Option<&ComponentOptions>,
  ) -> anyhow::Result<ItemEffect> {
    match event {
      ListenerAction::Signal { name, fd } if &**name == AUDIO_CHANGED => {
        drain_signal_fd(*fd);
        Ok(ItemEffect::Redraw)
      }
      ListenerAction::Named(name) if &**name == AUDIO_CHANGED => Ok(ItemEffect::Redraw),
      _ => Ok(ItemEffect::None),
    }
  }

  fn check_view(&self, store: &Store, _options: Option<&ComponentOptions>) -> bool {
    let Some(shared) = store.borrow::<SharedAudioState>() else {
      return false;
    };
    let player = shared.player.lock().unwrap();
    player.as_ref().is_some_and(|p| !p.title.is_empty())
  }

  fn view<'a>(
    &self,
    config: &Config,
    store: &Store,
    ctx: &ComponentContext,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let style = ctx.resolve_style(config);
    let theme = &config.theme;
    let icon_size = style.number("icon.size").unwrap_or(14.0) as u16;
    let font_size = style.number("font.size").unwrap_or(12.0);

    let show_artist = options.and_then(|o| o.bool("show_artist")).unwrap_or(true);
    let show_controls = options.and_then(|o| o.bool("controls")).unwrap_or(false);

    let (track_label, playback_status, can_go_next, can_go_previous) = store
      .borrow::<SharedAudioState>()
      .and_then(|shared| {
        let player = shared.player.lock().ok()?;
        let p = player.as_ref()?;
        if p.title.is_empty() {
          return None;
        }
        let label = if show_artist && !p.artist.is_empty() {
          format!("{} - {}", p.artist, p.title)
        } else {
          p.title.clone()
        };
        Some((
          label,
          p.playback_status.clone(),
          p.can_go_next,
          p.can_go_previous,
        ))
      })
      .unwrap_or_default();

    let max_len = options
      .and_then(|o| o.number("max_length"))
      .map(|n| n as usize)
      .unwrap_or(40);

    let truncated = if track_label.chars().count() > max_len {
      format!(
        "{}\u{2026}",
        track_label
          .chars()
          .take(max_len.saturating_sub(1))
          .collect::<String>()
      )
    } else {
      track_label
    };

    let text_color = style.color(theme, "color", theme.text);

    let mut items: Vec<Element<'a, ItemMessage>> = Vec::new();

    if show_controls {
      let prev_color = if can_go_previous {
        style.color(theme, "color", theme.text)
      } else {
        style.color(theme, "color.faded", theme.subtext)
      };
      let prev_icon: Element<'a, ItemMessage> = Icon::new("media-skip-backward-symbolic")
        .size(icon_size)
        .color(prev_color)
        .into();
      let prev_btn = clickable(prev_icon, |_, _, _| {
        Some(ItemMessage::Action(ListenerAction::Payload {
          name: "audio.previous".into(),
          payload: None,
        }))
      });
      items.push(prev_btn);

      let play_icon_name = if playback_status == "Playing" {
        "media-playback-pause-symbolic"
      } else {
        "media-playback-start-symbolic"
      };
      let play_icon: Element<'a, ItemMessage> = Icon::new(play_icon_name)
        .size(icon_size)
        .color(style.color(theme, "color.primary", theme.primary))
        .into();
      let play_btn = clickable(play_icon, |_, _, _| {
        Some(ItemMessage::Action(ListenerAction::Payload {
          name: "audio.play_pause".into(),
          payload: None,
        }))
      });
      items.push(play_btn);

      let next_color = if can_go_next {
        style.color(theme, "color", theme.text)
      } else {
        style.color(theme, "color.faded", theme.subtext)
      };
      let next_icon: Element<'a, ItemMessage> = Icon::new("media-skip-forward-symbolic")
        .size(icon_size)
        .color(next_color)
        .into();
      let next_btn = clickable(next_icon, |_, _, _| {
        Some(ItemMessage::Action(ListenerAction::Payload {
          name: "audio.next".into(),
          payload: None,
        }))
      });
      items.push(next_btn);
    }

    if ctx.orientation == PanelOrientation::Vertical {
      items.push(menu_trigger(
        text(truncated).size(font_size).color(text_color).into(),
        MenuConfig {
          content: "menus/sound".into(),
          panel_name: None,
          position: ctx.position,
        },
      ));
    }

    let content: iced::Element<'_, _> = if ctx.orientation == PanelOrientation::Horizontal {
      row(items)
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .into()
    } else {
      column(items)
        .spacing(4)
        .align_x(iced::Alignment::Center)
        .into()
    };

    container(content).into()
  }
}
