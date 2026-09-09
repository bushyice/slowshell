use iced::{
  Alignment, Element, Length,
  widget::{Space, column, container, row, text},
};
use slowshell_config::{style::Style, style::Theme};
use slowshell_core::message::ItemMessage;

use crate::{Icon, clickable};

pub struct ConfirmDialogProps<'a> {
  pub title: &'a str,
  pub message: &'a str,
  pub icon: Option<&'a str>,
  pub confirm_label: &'a str,
  pub cancel_label: &'a str,
  pub is_danger: bool,
  pub on_confirm: ItemMessage,
  pub on_cancel: ItemMessage,
}

pub fn render_confirm_dialog<'a>(
  props: ConfirmDialogProps<'a>,
  style: &Style,
  theme: &'a Theme,
) -> Element<'a, ItemMessage> {
  let title_color = if props.is_danger {
    style.color(theme, "color.danger", theme.red)
  } else {
    style.color(theme, "color", theme.text)
  };

  let title_el = text(props.title)
    .size(style.number("header.font.size").unwrap_or(14.0))
    .color(title_color);

  let header_row = if let Some(icon_name) = props.icon {
    let icon_color = if props.is_danger {
      style.color(theme, "color.danger", theme.red)
    } else {
      style.color(theme, "color.primary", theme.primary)
    };
    row![
      Icon::new(icon_name).size(16).color(icon_color),
      Space::new().width(6),
      title_el
    ]
    .align_y(Alignment::Center)
  } else {
    row![title_el].align_y(Alignment::Center)
  };

  let msg_el = text(props.message)
    .size(style.number("name.font.size").unwrap_or(12.0))
    .color(style.color(theme, "color.faded", theme.subtext));

  let cancel_btn = {
    let bg = style.color(theme, "cancel.background", theme.mantle);
    let border_color = style.color(theme, "cancel.border", theme.overlay);
    let radius = style.number("button.radius").unwrap_or(6.0);
    let txt = text(props.cancel_label)
      .size(style.number("button.font.size").unwrap_or(12.0))
      .color(style.color(theme, "color", theme.text));

    let box_cnt = container(txt)
      .padding(style.padding([5.0, 12.0]))
      .style(move |_| container::Style {
        background: Some(bg.into()),
        border: iced::Border {
          color: border_color,
          width: 1.0,
          radius: radius.into(),
        },
        ..Default::default()
      });

    clickable(box_cnt.into(), move |_, _, _| Some(props.on_cancel.clone()))
  };

  let confirm_btn = {
    let bg = if props.is_danger {
      style.color(theme, "color.danger", theme.red)
    } else {
      style.color(theme, "color.primary", theme.primary)
    };
    let text_color = theme.base;
    let radius = style.number("button.radius").unwrap_or(6.0);
    let txt = text(props.confirm_label)
      .size(style.number("button.font.size").unwrap_or(12.0))
      .color(text_color);

    let box_cnt = container(txt)
      .padding(style.padding([5.0, 12.0]))
      .style(move |_| container::Style {
        background: Some(bg.into()),
        border: iced::Border {
          radius: radius.into(),
          ..Default::default()
        },
        ..Default::default()
      });

    clickable(box_cnt.into(), move |_, _, _| {
      Some(props.on_confirm.clone())
    })
  };

  let actions_row = row![
    Space::new().width(Length::Fill),
    cancel_btn,
    Space::new().width(8),
    confirm_btn,
  ]
  .align_y(Alignment::Center);

  column![header_row, msg_el, Space::new().height(4), actions_row]
    .spacing(style.number("spacing").unwrap_or(8.0))
    .width(Length::Fill)
    .into()
}
