use iced::{
  Alignment, Element, Length,
  widget::{Space, column, container, image, row, slider},
};
use iced_layershell::reexport::IcedId;
use slowshell_commons::audio::{
  AudioSink, MprisPlayer, MprisSelection, SharedAudioState, volume_icon,
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{ItemEffect, ItemMessage},
  types::{PayloadBox, ToUstr},
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{
  Backdrop, Icon, Renderable, SizedPopup, clickable, hover_row, mpris_cover_handle, truncate,
};

pub struct SoundRenderable;

#[derive(Debug, Clone)]
enum SoundAction {
  ToggleMute,
  StepVolume(f32),
  SetVolume(f32),
  SetSink(u32),
  PlayPause,
  Next,
  Previous,
  SelectPlayer { bus_name: String, identity: String },
}

impl SoundAction {
  fn message(self, id: IcedId) -> ItemMessage {
    let (name, payload) = match self {
      SoundAction::ToggleMute => ("audio.toggle_mute", None),
      SoundAction::StepVolume(delta) => ("audio.step", Some(PayloadBox::new(delta))),
      SoundAction::SetVolume(vol) => ("audio.set_volume", Some(PayloadBox::new(vol))),
      SoundAction::SetSink(sink_id) => ("audio.set_sink", Some(PayloadBox::new(sink_id))),
      SoundAction::PlayPause => ("audio.play_pause", None),
      SoundAction::Next => ("audio.next", None),
      SoundAction::Previous => ("audio.previous", None),
      SoundAction::SelectPlayer { bus_name, identity } => (
        "audio.select_player",
        Some(PayloadBox::new(MprisSelection { bus_name, identity })),
      ),
    };

    ItemMessage::EffectAction(
      id,
      ItemEffect::Redraw,
      ListenerAction::Payload {
        name: name.to_ustr(),
        payload,
      },
    )
  }
}

impl Renderable for SoundRenderable {
  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    data: &'a dyn std::any::Any,
  ) -> Element<'a, ItemMessage> {
    let data = data.downcast_ref::<PopupSettings>().unwrap();

    let mut children: Vec<Element<'a, ItemMessage>> = Vec::new();

    if let Some(shared) = store.borrow::<SharedAudioState>() {
      let state = shared.state.lock().unwrap().clone();
      let player = shared.player.lock().unwrap().clone();
      let players = shared.players.lock().unwrap().clone();

      let (vol, muted) = (state.volume, state.muted);

      children.push(render_header(config, id, vol, muted));
      children.push(render_volume_slider(config, vol, muted, id));

      let primary_bus = player.as_ref().map(|p| p.bus_name.clone());

      if let Some(player) = player {
        if !player.title.is_empty() {
          children.push(render_mpris_card(config, &player, id));
        } else {
          children.push(
            container(config.text(
              "Nothing playing right now",
              &["menu", "text-status", "text-faded"],
            ))
            .padding(14)
            .align_x(Alignment::Center)
            .width(Length::Fill)
            .into(),
          );
        }
      }

      if players.len() > 1 {
        children.push(
          config
            .text("Players", &["menu", "text-status", "text-faded"])
            .into(),
        );

        for player in players.iter() {
          let is_primary = primary_bus.as_deref() == Some(player.bus_name.as_str());
          children.push(render_player(config, player, is_primary, id));
        }
      }

      if !state.sinks.is_empty() {
        children.push(
          config
            .text("Output Device", &["menu", "text-status", "text-faded"])
            .into(),
        );

        for sink in state.sinks.iter() {
          children.push(render_sink(config, sink, id));
        }
      }
    } else {
      children.push(
        config
          .text("Audio unavailable", &["menu", "text-status", "text-faded"])
          .into(),
      );
    }

    let list_style = config.computed(&["menu", "list"]);
    let list_spacing = config.sizing(&["menu", "list"]).gap;
    let list_width = list_style.length("list-width").unwrap_or(220.0);
    let list = column(children)
      .spacing(list_spacing)
      .width(Length::Fixed(list_width));

    let inner_padding = config.sizing(&["menu", "inner"]).padding();
    let popup_style = config.container_cached(&["menu"]);

    SizedPopup::new(container(
      container(list).padding(inner_padding).style(popup_style),
    ))
    .with_width(120.)
    .with_backdrop(Backdrop::transparent().with_close_on_click(true))
    .with_position(data.position.0.get(store), data.position.1.get(store))
    .with_panel_edge(data.panel_edge)
    .with_panel_insets(store)
    .with_on_close(ItemMessage::Effect(id, ItemEffect::Hide))
    .into()
  }
}

fn render_header<'a>(
  config: &'a Config,
  id: IcedId,
  vol: f32,
  muted: bool,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();
  let icon_name = volume_icon(vol, muted);

  let mute_icon: Element<'a, ItemMessage> = Icon::new(icon_name)
    .size(16)
    .color(if muted { theme.subtext } else { theme.primary })
    .into();

  let mute_btn = clickable(mute_icon, move |_, _, _| {
    Some(SoundAction::ToggleMute.message(id))
  });

  let pct_str = format!("{}%", (vol * 100.0).round() as u32);
  let vol_text = config.text(pct_str, &["menu", "text-header"]);

  let minus_icon: Element<'a, ItemMessage> = Icon::new("list-remove-symbolic")
    .size(12)
    .color(theme.subtext)
    .into();
  let minus_btn = clickable(minus_icon, move |_, _, _| {
    Some(SoundAction::StepVolume(-0.05).message(id))
  });

  let plus_icon: Element<'a, ItemMessage> = Icon::new("list-add-symbolic")
    .size(12)
    .color(theme.subtext)
    .into();
  let plus_btn = clickable(plus_icon, move |_, _, _| {
    Some(SoundAction::StepVolume(0.05).message(id))
  });

  row![
    config.text("Sound", &["menu", "text-header"]),
    Space::new().width(Length::Fill),
    mute_btn,
    Space::new().width(8),
    minus_btn,
    Space::new().width(4),
    vol_text,
    Space::new().width(4),
    plus_btn,
  ]
  .align_y(Alignment::Center)
  .into()
}

fn render_volume_slider<'a>(
  config: &'a Config,
  vol: f32,
  muted: bool,
  id: IcedId,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();
  let bar_color = if muted { theme.subtext } else { theme.primary };
  let track_bg = theme.mantle;
  let handle_color = theme.text;
  let handle_border = theme.overlay;

  let slider = slider(0.0..=1.5, vol, move |v| {
    SoundAction::SetVolume(v).message(id)
  })
  .step(0.01f32)
  .width(Length::Fill)
  .style(move |_theme, _status| iced::widget::slider::Style {
    rail: iced::widget::slider::Rail {
      backgrounds: (bar_color.into(), track_bg.into()),
      width: 6.0,
      border: iced::Border {
        radius: 3.0.into(),
        ..Default::default()
      },
    },
    handle: iced::widget::slider::Handle {
      shape: iced::widget::slider::HandleShape::Circle { radius: 7.0 },
      background: handle_color.into(),
      border_width: 1.0,
      border_color: handle_border.into(),
    },
  });

  container(slider)
    .width(Length::Fill)
    .padding(config.sizing(&["menu"]).padding())
    .into()
}

fn render_mpris_card<'a>(
  config: &'a Config,
  player: &MprisPlayer,
  id: IcedId,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();

  let title = config.text(truncate(&player.title, 20), &["menu", "text-body"]);

  let artist = config.text(
    truncate(
      if !player.artist.is_empty() {
        &player.artist
      } else {
        &player.identity
      },
      20,
    ),
    &["menu", "text-status", "text-faded"],
  );

  let prev_color = if player.can_go_previous {
    theme.text
  } else {
    theme.subtext
  };
  let prev_icon: Element<'a, ItemMessage> = Icon::new("media-skip-backward-symbolic")
    .size(14)
    .color(prev_color)
    .into();
  let prev_btn = clickable(prev_icon, move |_, _, _| {
    Some(SoundAction::Previous.message(id))
  });

  let play_icon_name = if player.playback_status == "Playing" {
    "media-playback-pause-symbolic"
  } else {
    "media-playback-start-symbolic"
  };
  let play_icon: Element<'a, ItemMessage> = Icon::new(play_icon_name)
    .size(18)
    .color(theme.primary)
    .into();
  let play_btn = clickable(play_icon, move |_, _, _| {
    Some(SoundAction::PlayPause.message(id))
  });

  let next_color = if player.can_go_next {
    theme.text
  } else {
    theme.subtext
  };
  let next_icon: Element<'a, ItemMessage> = Icon::new("media-skip-forward-symbolic")
    .size(14)
    .color(next_color)
    .into();
  let next_btn = clickable(next_icon, move |_, _, _| {
    Some(SoundAction::Next.message(id))
  });

  let controls = row![prev_btn, play_btn, next_btn]
    .spacing(12)
    .align_y(Alignment::Center);

  let mut media_lines = column![title, artist].spacing(2);
  if !player.album.is_empty() {
    media_lines = media_lines.push(config.text(
      truncate(&player.album, 20),
      &["menu", "text-status", "text-faded"],
    ));
  }

  let cover: Option<Element<'a, ItemMessage>> = player
    .art_url
    .as_deref()
    .and_then(mpris_cover_handle)
    .map(|handle| {
      image(handle)
        .width(Length::Fixed(48.0))
        .height(Length::Fixed(48.0))
        .border_radius(8.0)
        .into()
    });

  let media: Element<'a, ItemMessage> = match cover {
    Some(cover) => row![cover, media_lines]
      .spacing(10)
      .align_y(Alignment::Center)
      .into(),
    None => media_lines.into(),
  };

  hover_row(
    config,
    &["menu", "row"],
    container(
      column![
        media,
        container(controls)
          .align_x(Alignment::Center)
          .width(Length::Fill),
      ]
      .spacing(8),
    )
    .padding(config.sizing(&["menu", "row"]).padding())
    .width(Length::Fill),
  )
}

fn render_sink<'a>(config: &'a Config, sink: &AudioSink, id: IcedId) -> Element<'a, ItemMessage> {
  let theme = config.palette();
  let is_default = sink.is_default;
  let color = if is_default {
    theme.primary
  } else {
    theme.text
  };

  let icon_name = if is_default {
    "checkbox-checked-symbolic"
  } else {
    "checkbox-symbolic"
  };

  let checkbox_size = config
    .computed(&["menu", "checkbox"])
    .length("checkbox-size")
    .unwrap_or(14.0) as u16;
  let check_icon: Element<'a, ItemMessage> =
    Icon::new(icon_name).size(checkbox_size).color(color).into();

  let desc = if sink.description.chars().count() > 24 {
    format!("{}…", sink.description.chars().take(22).collect::<String>())
  } else {
    sink.description.clone()
  };

  let name_classes: &[&str] = if is_default {
    &["menu", "text-name", "text-primary"]
  } else {
    &["menu", "text-name"]
  };
  let sink_name = config.text(desc, name_classes);

  let content = row![check_icon, Space::new().width(6), sink_name]
    .align_y(Alignment::Center)
    .width(Length::Fill);

  let sink_id = sink.id;
  let wrapped = hover_row(
    config,
    &["menu", "row"],
    container(content)
      .padding(config.sizing(&["menu", "row"]).padding())
      .width(Length::Fill),
  );

  clickable(wrapped, move |_, _, _| {
    Some(SoundAction::SetSink(sink_id).message(id))
  })
}

fn render_player<'a>(
  config: &'a Config,
  player: &MprisPlayer,
  is_primary: bool,
  id: IcedId,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();
  let color = if is_primary {
    theme.primary
  } else {
    theme.text
  };

  let icon_name = if is_primary {
    "checkbox-checked-symbolic"
  } else {
    "checkbox-symbolic"
  };

  let checkbox_size = config
    .computed(&["menu", "checkbox"])
    .length("checkbox-size")
    .unwrap_or(14.0) as u16;
  let check_icon: Element<'a, ItemMessage> =
    Icon::new(icon_name).size(checkbox_size).color(color).into();

  let identity = if player.identity.is_empty() {
    player.bus_name.clone()
  } else {
    player.identity.clone()
  };

  let name_classes: &[&str] = if is_primary {
    &["menu", "text-name", "text-primary"]
  } else {
    &["menu", "text-name"]
  };
  let name = config.text(truncate(&identity, 18), name_classes);

  let mut content = row![check_icon, Space::new().width(6), name]
    .align_y(Alignment::Center)
    .width(Length::Fill);

  if !player.title.is_empty() {
    content = content.push(Space::new().width(6)).push(config.text(
      truncate(&player.title, 16),
      &["menu", "text-status", "text-faded"],
    ));
  }

  let bus_name = player.bus_name.clone();
  let identity = player.identity.clone();

  let wrapped = hover_row(
    config,
    &["menu", "row"],
    container(content)
      .padding(config.sizing(&["menu", "row"]).padding())
      .width(Length::Fill),
  );

  clickable(wrapped, move |_, _, _| {
    Some(
      SoundAction::SelectPlayer {
        bus_name: bus_name.clone(),
        identity: identity.clone(),
      }
      .message(id),
    )
  })
}
