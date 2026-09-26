use iced::{
  Alignment, Element, Length,
  widget::{Space, column, container, row, slider},
};
use iced_layershell::reexport::IcedId;
use slowshell_commons::power::{PowerProfile, SharedPowerState, battery_icon, brightness_icon};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{ItemEffect, ItemMessage},
  types::{PayloadBox, ToUstr},
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{Backdrop, Icon, Renderable, SizedPopup, clickable, hover_row, separator};

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
    let theme = config.palette();

    let mut children: Vec<Element<'a, ItemMessage>> = Vec::new();

    if let Some(shared) = store.borrow::<SharedPowerState>() {
      let data_snapshot = {
        let lock = shared.data.lock().unwrap();
        lock.clone()
      };

      children.push(render_battery_header(config, &data_snapshot, id));

      if let Some(card) = render_battery_details(config, &data_snapshot) {
        children.push(card);
      }

      children.push(
        separator()
          .color(
            config
              .color(&["menu", "separator"], "color")
              .unwrap_or(theme.base),
          )
          .into(),
      );

      // TODO: Hide on desktop
      children.push(render_brightness_header(config, &data_snapshot, id));
      children.push(render_brightness_slider(
        config,
        data_snapshot.brightness_percent,
        id,
      ));

      if !data_snapshot.available_profiles.is_empty() {
        children.push(render_profiles_section(config, &data_snapshot, id));
      }
    } else {
      children.push(
        config
          .text(
            "Power & Brightness service not initialized",
            &["menu", "text-name", "text-faded"],
          )
          .into(),
      );
    }

    let list_spacing = config.sizing(&["menu", "list"]).gap;
    let list_width = config
      .computed(&["menu", "list", "large"])
      .length("list-large-width")
      .unwrap_or(320.0);
    let body = column(children)
      .spacing(list_spacing)
      .width(Length::Fixed(list_width));

    let inner_padding = config.sizing(&["menu", "inner"]).padding();
    let popup_style = config.container_cached(&["menu"]);

    SizedPopup::new(container(
      container(body).padding(inner_padding).style(popup_style),
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
  config: &'a Config,
  data: &slowshell_commons::power::PowerData,
  _id: IcedId,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();
  let color = theme.text;
  let bat_icon = battery_icon(data.charging, data.percent);

  let icon_color = if data.charging {
    theme.primary
  } else if let Some(pct) = data.percent {
    if pct <= 20 { theme.red } else { color }
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
      config.text("Battery & Power", &["menu", "text-header"]),
      Space::new().width(Length::Fill),
      config.text(pct_str, &["menu", "text-header"]),
    ]
    .align_y(Alignment::Center),
    row![
      config.text(status_str, &["menu", "text-status", "text-faded"]),
      Space::new().width(Length::Fill),
      config.text(remaining_str, &["menu", "text-status", "text-faded"]),
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
  config: &'a Config,
  data: &slowshell_commons::power::PowerData,
) -> Option<Element<'a, ItemMessage>> {
  if data.energy_now_wh.is_none() && data.health.is_none() && data.power_w.is_none() {
    return None;
  }

  let mut rows = Vec::new();

  if let (Some(en), Some(ef)) = (data.energy_now_wh, data.energy_full_wh) {
    let txt = format!("{:.1} / {:.1} Wh", en, ef);
    rows.push(detail_kv(config, "Capacity", txt));
  }

  if let Some(rate) = data.power_w {
    if rate > 0.05 {
      let txt = format!("{:.2} W", rate);
      rows.push(detail_kv(config, "Discharge / Rate", txt));
    }
  }

  if let Some(health) = data.health {
    let txt = format!("{health}%");
    rows.push(detail_kv(config, "Battery Health", txt));
  }

  let col = column(rows).spacing(4).width(Length::Fill);

  let card = hover_row(
    config,
    &["menu", "row", "solid"],
    container(col)
      .padding(config.sizing(&["menu", "row"]).padding())
      .width(Length::Fill),
  );

  Some(card)
}

fn detail_kv<'a>(config: &'a Config, label: &'static str, val: String) -> Element<'a, ItemMessage> {
  row![
    config.text(label, &["menu", "text-status", "text-faded"]),
    Space::new().width(Length::Fill),
    config.text(val, &["menu", "text-status"]),
  ]
  .align_y(Alignment::Center)
  .into()
}

fn render_brightness_header<'a>(
  config: &'a Config,
  data: &slowshell_commons::power::PowerData,
  id: IcedId,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();
  let icon_name = brightness_icon(data.brightness_percent);

  let icon: Element<'a, ItemMessage> = Icon::new(icon_name).size(16).color(theme.yellow).into();

  let minus_icon: Element<'a, ItemMessage> = Icon::new("list-remove-symbolic")
    .size(12)
    .color(theme.subtext)
    .into();
  let minus_btn = clickable(minus_icon, move |_, _, _| {
    Some(PowerBrightnessAction::StepBrightness(-5).message(id))
  });

  let pct_str = format!("{}%", data.brightness_percent);
  let pct_text = config.text(pct_str, &["menu", "text-header"]);

  let plus_icon: Element<'a, ItemMessage> = Icon::new("list-add-symbolic")
    .size(12)
    .color(theme.subtext)
    .into();
  let plus_btn = clickable(plus_icon, move |_, _, _| {
    Some(PowerBrightnessAction::StepBrightness(5).message(id))
  });

  row![
    icon,
    Space::new().width(8),
    config.text("Brightness", &["menu", "text-header"]),
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
  config: &'a Config,
  pct: u8,
  id: IcedId,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();
  let bar_color = theme.yellow;
  let track_bg = theme.mantle;
  let handle_color = theme.text;
  let handle_border = theme.overlay;

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

  container(slider)
    .padding(config.sizing(&["menu"]).padding())
    .into()
}

fn render_profiles_section<'a>(
  config: &'a Config,
  data: &slowshell_commons::power::PowerData,
  id: IcedId,
) -> Element<'a, ItemMessage> {
  let header = config.text("Power Mode", &["menu", "text-header"]);

  let mut rows: Vec<Element<'_, _>> = Vec::new();
  let mut iter = data.available_profiles.iter().map(|p| {
    let is_active = data.active_profile.as_ref() == Some(p);
    render_profile_pill(config, p.clone(), is_active, id)
  });

  while let Some(first) = iter.next() {
    let row = match iter.next() {
      Some(second) => row![first, second],
      None => row![first].width(Length::Fill),
    };

    rows.push(row.spacing(6).width(Length::Fill).into());
  }

  column![header, column(rows).spacing(6).width(Length::Fill),]
    .spacing(6)
    .into()
}

fn render_profile_pill<'a>(
  config: &'a Config,
  profile: PowerProfile,
  active: bool,
  id: IcedId,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();

  let icon_color = if active { theme.primary } else { theme.subtext };

  let icon: Element<'a, ItemMessage> = Icon::new(profile.icon()).size(14).color(icon_color).into();

  let label_str = profile.label().to_string();
  let label_classes: &[&str] = if active {
    &["menu", "text-status"]
  } else {
    &["menu", "text-status", "text-faded"]
  };
  let label_text = config.text(label_str, label_classes);

  let pill_content = row![icon, Space::new().width(4), label_text].align_y(Alignment::Center);

  let classes: &'static [&'static str] = if active {
    &["menu", "pill", "active"]
  } else {
    &["menu", "pill"]
  };

  let box_cnt = hover_row(
    config,
    classes,
    container(pill_content)
      .padding(config.sizing(classes).padding())
      .width(Length::Fill),
  );

  let action_prof = profile.clone();
  clickable(box_cnt, move |_, _, _| {
    Some(PowerBrightnessAction::SetProfile(action_prof.clone()).message(id))
  })
}
