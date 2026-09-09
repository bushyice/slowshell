use iced::{
  Alignment, Color, Element, Length,
  widget::{Space, column, container, row, slider, text},
};
use iced_layershell::reexport::IcedId;
use slowshell_commons::power::{PowerProfile, SharedPowerState, battery_icon, brightness_icon};
use slowshell_config::{Config, style::Style, style::Theme};
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{ItemEffect, ItemMessage},
  types::{PayloadBox, ToUstr},
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{Backdrop, Icon, Renderable, SizedPopup, clickable, separator};

pub struct PowerBrightnessRenderable;

#[derive(Debug, Clone)]
enum PowerBrightnessAction {
  SetBrightness(u8),
  StepBrightness(i32),
  SetProfile(PowerProfile),
}

impl PowerBrightnessAction {
  fn message(self, id: IcedId) -> ItemMessage {
    let (name, payload) = match self {
      PowerBrightnessAction::SetBrightness(pct) => {
        ("power.set_brightness", Some(PayloadBox::new(pct)))
      }
      PowerBrightnessAction::StepBrightness(delta) => {
        ("power.step_brightness", Some(PayloadBox::new(delta)))
      }
      PowerBrightnessAction::SetProfile(profile) => (
        "power.set_profile",
        Some(PayloadBox::new(profile.as_str().to_ustr())),
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

impl Renderable for PowerBrightnessRenderable {
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

    if let Some(shared) = store.borrow::<SharedPowerState>() {
      let data_snapshot = {
        let lock = shared.data.lock().unwrap();
        lock.clone()
      };

      children.push(render_battery_header(&data_snapshot, id, &style, theme));

      if let Some(card) = render_battery_details(&data_snapshot, &style, theme) {
        children.push(card);
      }

      children.push(
        separator()
          .color(style.color(theme, "separator.color", theme.overlay))
          .into(),
      );

      // TODO: Hide on desktop
      children.push(render_brightness_header(&data_snapshot, id, &style, theme));
      children.push(render_brightness_slider(
        data_snapshot.brightness_percent,
        id,
        &style,
        theme,
      ));

      if !data_snapshot.available_profiles.is_empty() {
        children.push(render_profiles_section(&data_snapshot, id, &style, theme));
      }
    } else {
      children.push(
        text("Power & Brightness service not initialized")
          .size(style.number("name.font.size").unwrap_or(13.0))
          .color(style.color(theme, "color.faded", theme.subtext))
          .into(),
      );
    }

    let list_spacing = style.number("list.spacing").unwrap_or(8.0);
    let list_width = style.number("list.large.width").unwrap_or(300.0);
    let body = column(children)
      .spacing(list_spacing)
      .width(Length::Fixed(list_width));

    let inner_padding = style.number("inner.padding").unwrap_or(10.0);
    let mut popup_style = style.container_style(theme);
    if let (Some(x), Some(y)) = (
      style.number("shadow.offset.x"),
      style.number("shadow.offset.y"),
    ) {
      popup_style.shadow.offset = iced::Vector::new(x, y);
    }

    SizedPopup::new(container(
      container(body)
        .padding(inner_padding)
        .style(move |_| popup_style),
    ))
    .with_width(280.)
    .with_backdrop(Backdrop::transparent().with_close_on_click(true))
    .with_position(data.position.0.get(store), data.position.1.get(store))
    .with_panel_edge(data.panel_edge)
    .with_panel_insets(store)
    .with_on_close(ItemMessage::Effect(id, ItemEffect::Hide))
    .into()
  }
}

fn render_battery_header<'a>(
  data: &slowshell_commons::power::PowerData,
  _id: IcedId,
  style: &Style,
  theme: &Theme,
) -> Element<'a, ItemMessage> {
  let color = style.color(theme, "color", theme.text);
  let bat_icon = battery_icon(data.charging, data.percent);

  let icon_color = if data.charging {
    style.color(theme, "color.primary", theme.primary)
  } else if let Some(pct) = data.percent {
    if pct <= 20 {
      style.color(theme, "color.danger", theme.red)
    } else {
      color
    }
  } else {
    color
  };

  let icon: Element<'a, ItemMessage> = Icon::new(bat_icon).size(18).color(icon_color).into();

  let pct_str = data
    .percent
    .map(|p| format!("{p}%"))
    .unwrap_or_else(|| "AC Power".to_string());

  let status_str = if data.charging {
    "Charging".to_string()
  } else if data.percent.is_some() {
    data.status.clone()
  } else {
    "Connected".to_string()
  };

  let remaining_str = data.time_remaining.clone().unwrap_or_default();

  let title_col = column![
    row![
      text("Battery & Power")
        .size(style.number("header.font.size").unwrap_or(15.0))
        .color(color),
      Space::new().width(Length::Fill),
      text(pct_str)
        .size(style.number("header.font.size").unwrap_or(14.0))
        .color(color),
    ]
    .align_y(Alignment::Center),
    row![
      text(status_str)
        .size(style.number("status.font.size").unwrap_or(11.0))
        .color(style.color(theme, "color.faded", theme.subtext)),
      Space::new().width(Length::Fill),
      text(remaining_str)
        .size(style.number("status.font.size").unwrap_or(11.0))
        .color(style.color(theme, "color.faded", theme.subtext)),
    ]
    .align_y(Alignment::Center),
  ]
  .spacing(2)
  .width(Length::Fill);

  row![icon, Space::new().width(8), title_col]
    .align_y(Alignment::Center)
    .into()
}

fn render_battery_details<'a>(
  data: &slowshell_commons::power::PowerData,
  style: &Style,
  theme: &Theme,
) -> Option<Element<'a, ItemMessage>> {
  if data.energy_now_wh.is_none() && data.health.is_none() && data.power_w.is_none() {
    return None;
  }

  let mut rows = Vec::new();

  if let (Some(en), Some(ef)) = (data.energy_now_wh, data.energy_full_wh) {
    let txt = format!("{:.1} / {:.1} Wh", en, ef);
    rows.push(detail_kv("Capacity", txt, style, theme));
  }

  if let Some(rate) = data.power_w {
    if rate > 0.05 {
      let txt = format!("{:.2} W", rate);
      rows.push(detail_kv("Discharge / Rate", txt, style, theme));
    }
  }

  if let Some(health) = data.health {
    let txt = format!("{health}%");
    rows.push(detail_kv("Battery Health", txt, style, theme));
  }

  let card_bg = theme.mantle;
  let card_radius = style.number("row.radius").unwrap_or(6.0);

  let col = column(rows).spacing(4).width(Length::Fill);

  let container = container(col)
    .padding(style.padding([6.0, 10.0]))
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(card_bg.into()),
      border: iced::Border {
        radius: card_radius.into(),
        ..Default::default()
      },
      ..Default::default()
    });

  Some(container.into())
}

fn detail_kv<'a>(
  label: &'static str,
  val: String,
  style: &Style,
  theme: &Theme,
) -> Element<'a, ItemMessage> {
  row![
    text(label)
      .size(style.number("status.font.size").unwrap_or(11.0))
      .color(style.color(theme, "color.faded", theme.subtext)),
    Space::new().width(Length::Fill),
    text(val)
      .size(style.number("status.font.size").unwrap_or(11.0))
      .color(style.color(theme, "color", theme.text)),
  ]
  .align_y(Alignment::Center)
  .into()
}

fn render_brightness_header<'a>(
  data: &slowshell_commons::power::PowerData,
  id: IcedId,
  style: &Style,
  theme: &Theme,
) -> Element<'a, ItemMessage> {
  let color = style.color(theme, "color", theme.text);
  let icon_name = brightness_icon(data.brightness_percent);

  let icon: Element<'a, ItemMessage> = Icon::new(icon_name)
    .size(16)
    .color(style.color(theme, "color.secondary", theme.yellow))
    .into();

  let minus_icon: Element<'a, ItemMessage> = Icon::new("list-remove-symbolic")
    .size(12)
    .color(style.color(theme, "color.faded", theme.subtext))
    .into();
  let minus_btn = clickable(minus_icon, move |_, _, _| {
    Some(PowerBrightnessAction::StepBrightness(-5).message(id))
  });

  let pct_str = format!("{}%", data.brightness_percent);
  let pct_text = text(pct_str)
    .size(style.number("header.font.size").unwrap_or(13.0))
    .color(color);

  let plus_icon: Element<'a, ItemMessage> = Icon::new("list-add-symbolic")
    .size(12)
    .color(style.color(theme, "color.faded", theme.subtext))
    .into();
  let plus_btn = clickable(plus_icon, move |_, _, _| {
    Some(PowerBrightnessAction::StepBrightness(5).message(id))
  });

  row![
    icon,
    Space::new().width(8),
    text("Brightness")
      .size(style.number("header.font.size").unwrap_or(14.0))
      .color(color),
    Space::new().width(Length::Fill),
    minus_btn,
    Space::new().width(6),
    pct_text,
    Space::new().width(6),
    plus_btn,
  ]
  .align_y(Alignment::Center)
  .into()
}

fn render_brightness_slider<'a>(
  pct: u8,
  id: IcedId,
  style: &Style,
  theme: &Theme,
) -> Element<'a, ItemMessage> {
  let bar_color = style.color(theme, "color.secondary", theme.yellow);
  let track_bg = theme.mantle;
  let handle_color = style.color(theme, "color", theme.text);
  let handle_border = style.color(theme, "color.border", theme.overlay);

  let slider = slider(1..=100, pct, move |v| {
    PowerBrightnessAction::SetBrightness(v).message(id)
  })
  .step(1u8)
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
      border_width: 1.5,
      border_color: handle_border,
    },
  });

  container(slider).padding(style.padding([0.0, 4.0])).into()
}

fn render_profiles_section<'a>(
  data: &slowshell_commons::power::PowerData,
  id: IcedId,
  style: &Style,
  theme: &Theme,
) -> Element<'a, ItemMessage> {
  let header = text("Power Mode")
    .size(style.number("header.font.size").unwrap_or(13.0))
    .color(style.color(theme, "color", theme.text));

  let mut profile_btns = Vec::new();
  for p in &data.available_profiles {
    let is_active = data.active_profile.as_ref() == Some(p);
    profile_btns.push(render_profile_pill(p.clone(), is_active, id, style, theme));
  }

  column![header, row(profile_btns).spacing(6).width(Length::Fill)]
    .spacing(6)
    .into()
}

fn render_profile_pill<'a>(
  profile: PowerProfile,
  active: bool,
  id: IcedId,
  style: &Style,
  theme: &Theme,
) -> Element<'a, ItemMessage> {
  let icon_color = if active {
    style.color(theme, "color.primary", theme.primary)
  } else {
    style.color(theme, "color.faded", theme.subtext)
  };

  let icon: Element<'a, ItemMessage> = Icon::new(profile.icon()).size(14).color(icon_color).into();

  let label_str = profile.label().to_string();
  let label_text = text(label_str)
    .size(style.number("status.font.size").unwrap_or(11.0))
    .color(if active {
      style.color(theme, "color", theme.text)
    } else {
      style.color(theme, "color.faded", theme.subtext)
    });

  let pill_content = row![icon, Space::new().width(4), label_text].align_y(Alignment::Center);

  let bg = if active {
    theme.mantle
  } else {
    Color::TRANSPARENT
  };

  let border_color = if active {
    style.color(theme, "color.primary", theme.primary)
  } else {
    style.color(theme, "color.border", theme.overlay)
  };

  let radius = style.number("row.radius").unwrap_or(6.0);

  let box_cnt = container(pill_content)
    .padding(style.padding([5.0, 8.0]))
    .style(move |_| container::Style {
      background: Some(bg.into()),
      border: iced::Border {
        color: border_color,
        width: if active { 1.0 } else { 0.5 },
        radius: radius.into(),
      },
      ..Default::default()
    });

  let action_prof = profile.clone();
  clickable(box_cnt.into(), move |_, _, _| {
    Some(PowerBrightnessAction::SetProfile(action_prof.clone()).message(id))
  })
}
