use iced::{
  Alignment, Color, Element, Length,
  widget::{Space, column, container, row, slider, text},
};
use iced_layershell::reexport::IcedId;
use slowshell_commons::audio::{AudioSink, MprisPlayer, SharedAudioState, volume_icon};
use slowshell_config::{Config, style::Style, style::Theme};
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{ItemEffect, ItemMessage},
  types::{PayloadBox, ToUstr},
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{Backdrop, Icon, Renderable, SizedPopup, clickable};

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
    let style = config.style("menu");
    let theme = &config.theme;

    let mut children: Vec<Element<'a, ItemMessage>> = Vec::new();

    if let Some(shared) = store.borrow::<SharedAudioState>() {
      let (vol, muted, sinks) = {
        let state = shared.state.lock().unwrap();
        (state.volume, state.muted, state.sinks.clone())
      };

      let player = {
        let p = shared.player.lock().unwrap();
        p.clone()
      };

      children.push(render_header(&style, theme, id, vol, muted));
      children.push(render_volume_slider(vol, muted, id, &style, theme));

      if let Some(player) = player {
        if !player.title.is_empty() {
          children.push(render_mpris_card(&player, &style, theme, id));
        } else {
          children.push(
            container(
              text("Nothing playing right now")
                .size(style.number("status.font.size").unwrap_or(12.0))
                .color(style.color(theme, "color.faded", theme.subtext)),
            )
            .padding(14)
            .align_x(Alignment::Center)
            .width(Length::Fill)
            .into(),
          );
        }
      }

      if !sinks.is_empty() {
        children.push(
          text("Output Device")
            .size(style.number("status.font.size").unwrap_or(11.0))
            .color(style.color(theme, "color.faded", theme.subtext))
            .into(),
        );

        for sink in sinks {
          children.push(render_sink(&sink, &style, theme, id));
        }
      }
    } else {
      children.push(
        text("Audio unavailable")
          .size(style.number("status.font.size").unwrap_or(11.0))
          .color(style.color(theme, "color.faded", theme.subtext))
          .into(),
      );
    }

    let list_spacing = style.number("list.spacing").unwrap_or(6.0);
    let list_width = style.number("list.width").unwrap_or(240.0);
    let list = column(children)
      .spacing(list_spacing)
      .width(Length::Fixed(list_width));

    let inner_padding = style.number("inner.padding").unwrap_or(6.0);
    let mut popup_style = style.container_style(theme);
    if let (Some(x), Some(y)) = (
      style.number("shadow.offset.x"),
      style.number("shadow.offset.y"),
    ) {
      popup_style.shadow.offset = iced::Vector::new(x, y);
    }

    SizedPopup::new(container(
      container(list)
        .padding(inner_padding)
        .style(move |_| popup_style),
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
  style: &Style,
  theme: &Theme,
  id: IcedId,
  vol: f32,
  muted: bool,
) -> Element<'a, ItemMessage> {
  let color = style.color(theme, "color", theme.text);
  let icon_name = volume_icon(vol, muted);

  let mute_icon: Element<'a, ItemMessage> = Icon::new(icon_name)
    .size(16)
    .color(if muted {
      style.color(theme, "color.faded", theme.subtext)
    } else {
      style.color(theme, "color.primary", theme.primary)
    })
    .into();

  let mute_btn = clickable(mute_icon, move |_, _, _| {
    Some(SoundAction::ToggleMute.message(id))
  });

  let pct_str = format!("{}%", (vol * 100.0).round() as u32);
  let vol_text = text(pct_str)
    .size(style.number("header.font.size").unwrap_or(14.0))
    .color(color);

  let minus_icon: Element<'a, ItemMessage> = Icon::new("list-remove-symbolic")
    .size(12)
    .color(style.color(theme, "color.faded", theme.subtext))
    .into();
  let minus_btn = clickable(minus_icon, move |_, _, _| {
    Some(SoundAction::StepVolume(-0.05).message(id))
  });

  let plus_icon: Element<'a, ItemMessage> = Icon::new("list-add-symbolic")
    .size(12)
    .color(style.color(theme, "color.faded", theme.subtext))
    .into();
  let plus_btn = clickable(plus_icon, move |_, _, _| {
    Some(SoundAction::StepVolume(0.05).message(id))
  });

  row![
    text("Sound")
      .size(style.number("header.font.size").unwrap_or(15.0))
      .color(color),
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
  vol: f32,
  muted: bool,
  id: IcedId,
  style: &Style,
  theme: &Theme,
) -> Element<'a, ItemMessage> {
  let bar_color = if muted {
    style.color(theme, "color.faded", theme.subtext)
  } else {
    style.color(theme, "color.primary", theme.primary)
  };
  let track_bg = theme.mantle;
  let handle_color = style.color(theme, "color", theme.text);
  let handle_border = style.color(theme, "color.border", theme.overlay);

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
    .padding(style.padding([2.0, 0.0]))
    .into()
}

fn render_mpris_card<'a>(
  player: &MprisPlayer,
  style: &Style,
  theme: &Theme,
  id: IcedId,
) -> Element<'a, ItemMessage> {
  let title = text(player.title.clone())
    .size(style.number("font.size").unwrap_or(13.0))
    .color(style.color(theme, "color", theme.text));

  let artist = text(if !player.artist.is_empty() {
    player.artist.clone()
  } else {
    player.identity.clone()
  })
  .size(style.number("status.font.size").unwrap_or(12.0))
  .color(style.color(theme, "color.faded", theme.subtext));

  let prev_color = if player.can_go_previous {
    style.color(theme, "color", theme.text)
  } else {
    style.color(theme, "color.faded", theme.subtext)
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
    .color(style.color(theme, "color.primary", theme.primary))
    .into();
  let play_btn = clickable(play_icon, move |_, _, _| {
    Some(SoundAction::PlayPause.message(id))
  });

  let next_color = if player.can_go_next {
    style.color(theme, "color", theme.text)
  } else {
    style.color(theme, "color.faded", theme.subtext)
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
    media_lines = media_lines.push(
      text(player.album.clone())
        .size(style.number("status.font.size").unwrap_or(11.0))
        .color(style.color(theme, "color.faded", theme.subtext)),
    );
  }

  let bg = style.color(theme, "row.background", Color::TRANSPARENT);
  let row_radius = style.number("row.radius").unwrap_or(10.0);

  container(
    column![
      media_lines,
      container(controls)
        .align_x(Alignment::Center)
        .width(Length::Fill),
    ]
    .spacing(8),
  )
  .padding(style.padding([10.0, 12.0]))
  .width(Length::Fill)
  .style(move |_| container::Style {
    background: Some(bg.into()),
    border: iced::Border {
      radius: row_radius.into(),
      ..Default::default()
    },
    ..Default::default()
  })
  .into()
}

fn render_sink<'a>(
  sink: &AudioSink,
  style: &Style,
  theme: &Theme,
  id: IcedId,
) -> Element<'a, ItemMessage> {
  let is_default = sink.is_default;
  let color = if is_default {
    style.color(theme, "color.primary", theme.primary)
  } else {
    style.color(theme, "color", theme.text)
  };

  let icon_name = if is_default {
    "checkbox-checked-symbolic"
  } else {
    "checkbox-symbolic"
  };

  let check_icon: Element<'a, ItemMessage> = Icon::new(icon_name).size(14).color(color).into();

  let desc = if sink.description.chars().count() > 24 {
    format!("{}…", sink.description.chars().take(22).collect::<String>())
  } else {
    sink.description.clone()
  };

  let sink_name = text(desc)
    .size(style.number("name.font.size").unwrap_or(12.0))
    .color(color);

  let content = row![check_icon, Space::new().width(6), sink_name]
    .align_y(Alignment::Center)
    .width(Length::Fill);

  let sink_id = sink.id;
  let bg = style.color(theme, "row.background", Color::TRANSPARENT);
  let row_radius = style.number("row.radius").unwrap_or(6.0);

  let wrapped = container(content)
    .padding(style.padding([7.0, 12.0]))
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(bg.into()),
      border: iced::Border {
        radius: row_radius.into(),
        ..Default::default()
      },
      ..Default::default()
    });

  clickable(wrapped.into(), move |_, _, _| {
    Some(SoundAction::SetSink(sink_id).message(id))
  })
}
