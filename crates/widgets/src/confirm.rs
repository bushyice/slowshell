use iced::{
  Alignment, Element, Length,
  widget::{Space, column, container, row},
};
use slowshell_config::Config;
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
  config: &'a Config,
) -> Element<'a, ItemMessage> {
  let palette = config.palette();
  let title_classes: &[&str] = if props.is_danger {
    &["menu", "text-header", "text-danger"]
  } else {
    &["menu", "text-header"]
  };
  let title_el = config.text(props.title, title_classes);

  let icon_size = config
    .computed(&["menu", "icon"])
    .length("icon-size")
    .unwrap_or(16.0) as u16;

  let header_row = if let Some(icon_name) = props.icon {
    let icon_color = if props.is_danger {
      palette.red
    } else {
      palette.primary
    };
    row![
      Icon::new(icon_name).size(icon_size).color(icon_color),
      Space::new().width(6),
      title_el
    ]
    .align_y(Alignment::Center)
  } else {
    row![title_el].align_y(Alignment::Center)
  };

  let msg_el = config.text(props.message, &["menu", "text-name", "text-faded"]);

  let button_padding = config.sizing(&["menu", "button"]).padding();

  let cancel_btn = {
    let txt = config.text(props.cancel_label, &["menu", "button"]);
    let box_cnt = container(txt)
      .padding(button_padding)
      .style(config.container_cached(&["menu", "button"]));

    clickable(box_cnt.into(), move |_, _, _| Some(props.on_cancel.clone()))
  };

  let confirm_btn = {
    let label_classes: &[&str] = if props.is_danger {
      &["menu", "button", "text-danger"]
    } else {
      &["menu", "button", "text-primary"]
    };
    let txt = config.text(props.confirm_label, label_classes);

    let box_cnt = container(txt)
      .padding(button_padding)
      .style(config.container_cached(&["menu", "button"]));

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
    .spacing(config.sizing(&["menu"]).gap)
    .width(Length::Fill)
    .into()
}
