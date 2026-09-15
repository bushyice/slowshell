use iced::{
  Alignment, Color, Element, Length,
  widget::{Space, column, container, row, space, text},
};
use iced_layershell::reexport::IcedId;
use slowshell_commons::bluetooth::{BluetoothDevice, SharedBluetoothState};
use slowshell_config::{Config, style::Style, style::Theme};
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{ItemEffect, ItemMessage},
  types::{PayloadBox, ToUstr},
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{Backdrop, Icon, Renderable, SizedPopup, clickable};

pub struct BluetoothRenderable;

#[derive(Debug, Clone)]
enum BtAction {
  Toggle,
  Scan,
  Connect(String),
  Disconnect(String),
  Pair(String),
  Remove(String),
  ConfirmRemove(String),
}

impl BtAction {
  fn message(self, id: IcedId) -> ItemMessage {
    let (name, payload) = match self {
      BtAction::Toggle => ("bluetooth.toggle", None),
      BtAction::Scan => ("bluetooth.scan", None),
      BtAction::Connect(addr) => ("bluetooth.connect", Some(PayloadBox::new(addr.to_ustr()))),
      BtAction::Disconnect(addr) => (
        "bluetooth.disconnect",
        Some(PayloadBox::new(addr.to_ustr())),
      ),
      BtAction::Pair(addr) => ("bluetooth.pair", Some(PayloadBox::new(addr.to_ustr()))),
      BtAction::Remove(addr) => ("bluetooth.remove", Some(PayloadBox::new(addr.to_ustr()))),
      BtAction::ConfirmRemove(addr) => (
        "bluetooth.confirm_remove",
        Some(PayloadBox::new(addr.to_ustr())),
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

impl Renderable for BluetoothRenderable {
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

    if let Some(shared) = store.borrow::<SharedBluetoothState>() {
      let powered = shared.powered.load(std::sync::atomic::Ordering::Relaxed);
      let discovering = shared
        .discovering
        .load(std::sync::atomic::Ordering::Relaxed);
      let devices = {
        let state = shared.state.lock().unwrap();
        state.devices.clone()
      };
      let confirming_remove = {
        let menu = shared.menu.lock().unwrap();
        menu.confirming_remove.clone()
      };

      children.push(render_toggle_row(&style, theme, id, powered));

      if !powered {
        children.push(
          container(
            column![
              Icon::new("bluetooth-disabled-symbolic")
                .size(22)
                .color(style.color(theme, "color.faded", theme.subtext)),
              space().height(6),
              text("Bluetooth is off")
                .size(style.number("name.font.size").unwrap_or(13.0))
                .color(style.color(theme, "color.faded", theme.subtext)),
            ]
            .align_x(Alignment::Center),
          )
          .padding(16)
          .align_x(Alignment::Center)
          .width(Length::Fill)
          .into(),
        );
      } else if devices.is_empty() {
        children.push(
          container(
            column![
              Icon::new(if discovering {
                "view-refresh-symbolic"
              } else {
                "bluetooth-symbolic"
              })
              .size(22)
              .color(style.color(theme, "color.faded", theme.subtext)),
              space().height(6),
              text(if discovering {
                "Scanning for devices…"
              } else {
                "No devices found"
              })
              .size(style.number("name.font.size").unwrap_or(13.0))
              .color(style.color(theme, "color.faded", theme.subtext)),
            ]
            .align_x(Alignment::Center),
          )
          .padding(16)
          .align_x(Alignment::Center)
          .width(Length::Fill)
          .into(),
        );
      } else {
        let connected_devs: Vec<BluetoothDevice> =
          devices.iter().filter(|d| d.connected).cloned().collect();
        let other_devs: Vec<BluetoothDevice> =
          devices.iter().filter(|d| !d.connected).cloned().collect();

        if !connected_devs.is_empty() {
          children.push(section_header("Connected", &style, theme));
          for dev in connected_devs {
            children.push(render_device(
              &dev,
              &style,
              theme,
              id,
              confirming_remove.as_deref(),
            ));
          }
        }

        if !other_devs.is_empty() {
          children.push(section_header("Devices", &style, theme));
          for dev in other_devs {
            children.push(render_device(
              &dev,
              &style,
              theme,
              id,
              confirming_remove.as_deref(),
            ));
          }
        }
      }

      if powered {
        children.push(scan_footer_row(discovering, id, &style, theme));
      }
    } else {
      children.push(
        text("Bluetooth unavailable")
          .size(style.number("status.font.size").unwrap_or(11.0))
          .color(style.color(theme, "color.faded", theme.subtext))
          .into(),
      );
    }

    let list_spacing = style.number("list.spacing").unwrap_or(4.0);
    let list_width = style.number("list.width").unwrap_or(220.0);
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

fn render_toggle_row<'a>(
  style: &Style,
  theme: &Theme,
  id: IcedId,
  powered: bool,
) -> Element<'a, ItemMessage> {
  let color = style.color(theme, "color", theme.text);
  let checkbox_size = style.number("checkbox.size").unwrap_or(14.0) as u16;
  let toggle_color = if powered {
    style.color(theme, "color.primary", theme.primary)
  } else {
    color
  };
  let toggle = Icon::new(if powered {
    "checkbox-checked-symbolic"
  } else {
    "checkbox-symbolic"
  })
  .size(checkbox_size)
  .color(toggle_color);

  let faded = style.color(theme, "color.faded", theme.subtext);
  let trailing = text(if powered { "On" } else { "Off" })
    .size(style.number("status.font.size").unwrap_or(12.0))
    .color(faded);

  let row_spacing = style.number("row.spacing").unwrap_or(10.0);
  let bg = style.color(theme, "row.background", Color::TRANSPARENT);
  let row_radius = style.number("row.radius").unwrap_or(10.0);
  let row_border_width = style.number("row.border.width").unwrap_or(0.0);
  let row_border_color = style.color(theme, "row.border.color", theme.overlay);

  let content: Element<'a, ItemMessage> = container(
    row![
      toggle,
      text("Bluetooth")
        .size(style.number("font.size").unwrap_or(14.0))
        .color(color),
      Space::new().width(Length::Fill),
      trailing
    ]
    .spacing(row_spacing)
    .align_y(Alignment::Center),
  )
  .padding(style.padding([7.0, 12.0]))
  .width(Length::Fill)
  .style(move |_t: &iced::Theme| container::Style {
    background: Some(bg.into()),
    border: iced::Border {
      radius: row_radius.into(),
      width: row_border_width,
      color: row_border_color,
    },
    ..container::Style::default()
  })
  .into();

  clickable(content, move |_, _, _| Some(BtAction::Toggle.message(id)))
}

fn section_header<'a>(
  title: &'static str,
  style: &Style,
  theme: &Theme,
) -> Element<'a, ItemMessage> {
  text(title)
    .size(style.number("status.font.size").unwrap_or(12.0))
    .color(style.color(theme, "color.faded", theme.subtext))
    .into()
}

fn scan_footer_row<'a>(
  discovering: bool,
  id: IcedId,
  style: &Style,
  theme: &Theme,
) -> Element<'a, ItemMessage> {
  let faded = style.color(theme, "color.faded", theme.subtext);
  let content = row![
    Icon::new("view-refresh-symbolic")
      .size(style.number("icon.size").unwrap_or(16.0) as u16)
      .color(faded),
    Space::new().width(6),
    text(if discovering {
      "Scanning…"
    } else {
      "Scan for devices…"
    })
    .size(style.number("name.font.size").unwrap_or(14.0))
    .color(faded),
    Space::new().width(Length::Fill),
  ]
  .align_y(Alignment::Center);

  let bg = style.color(theme, "row.background", Color::TRANSPARENT);
  let row_radius = style.number("row.radius").unwrap_or(10.0);
  let row_border_width = style.number("row.border.width").unwrap_or(0.0);
  let row_border_color = style.color(theme, "row.border.color", theme.overlay);

  let wrapped = container(content)
    .padding(style.padding([10.0, 12.0]))
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(bg.into()),
      border: iced::Border {
        radius: row_radius.into(),
        width: row_border_width,
        color: row_border_color,
      },
      ..Default::default()
    });

  if discovering {
    return wrapped.into();
  }
  clickable(wrapped.into(), move |_, _, _| {
    Some(BtAction::Scan.message(id))
  })
  .into()
}

fn render_device<'a>(
  dev: &BluetoothDevice,
  style: &Style,
  theme: &Theme,
  id: IcedId,
  confirming_remove: Option<&str>,
) -> Element<'a, ItemMessage> {
  let dev_icon_name = dev.icon.as_deref().unwrap_or_else(|| {
    let lower = dev.name.to_lowercase();
    if lower.contains("mouse") {
      "input-mouse-symbolic"
    } else if lower.contains("keyboard") {
      "input-keyboard-symbolic"
    } else if lower.contains("headset") || lower.contains("headphone") || lower.contains("buds") {
      "audio-headphones-symbolic"
    } else if lower.contains("controller") || lower.contains("gamepad") {
      "input-gaming-symbolic"
    } else if lower.contains("phone") {
      "phone-symbolic"
    } else {
      "bluetooth-symbolic"
    }
  });

  let icon_color = if dev.connected {
    style.color(theme, "color.primary", theme.primary)
  } else {
    style.color(theme, "color.faded", theme.subtext)
  };

  let dev_icon: Element<'a, ItemMessage> =
    Icon::new(dev_icon_name).size(14).color(icon_color).into();

  let dev_name = text(dev.name.clone())
    .size(style.number("name.font.size").unwrap_or(13.0))
    .color(style.color(theme, "color", theme.text));

  let mut right_items: Vec<Element<'a, ItemMessage>> = Vec::new();

  if let Some(bat) = dev.battery {
    right_items.push(
      text(format!("{bat}%"))
        .size(style.number("status.font.size").unwrap_or(10.0))
        .color(style.color(theme, "color.faded", theme.subtext))
        .into(),
    );
  }

  let action = if dev.connected {
    BtAction::Disconnect(dev.address.clone())
  } else {
    BtAction::Connect(dev.address.clone())
  };

  let action_icon: Element<'a, ItemMessage> = if dev.connected {
    Icon::new("media-playback-stop-symbolic")
      .size(12)
      .color(style.color(theme, "color.faded", theme.subtext))
      .into()
  } else {
    Icon::new("network-connect-symbolic")
      .size(12)
      .color(style.color(theme, "color.primary", theme.primary))
      .into()
  };

  right_items.push(clickable(action_icon, move |_, _, _| {
    Some(action.clone().message(id))
  }));

  if dev.paired {
    let is_confirming = confirming_remove == Some(dev.address.as_str());
    let remove_icon_name = if is_confirming {
      "edit-delete-symbolic"
    } else {
      "list-remove-symbolic"
    };
    let remove_color = if is_confirming {
      style.color(theme, "color.error", Color::from_rgb(0.9, 0.3, 0.3))
    } else {
      style.color(theme, "color.faded", theme.subtext)
    };
    let remove_icon: Element<'a, ItemMessage> = Icon::new(remove_icon_name)
      .size(12)
      .color(remove_color)
      .into();

    let addr = dev.address.clone();
    let confirming = is_confirming;
    right_items.push(clickable(remove_icon, move |_, _, _| {
      if confirming {
        Some(BtAction::Remove(addr.clone()).message(id))
      } else {
        Some(BtAction::ConfirmRemove(addr.clone()).message(id))
      }
    }));
  } else if dev.connected {
    // if paired=false, no fucking way it connected
  } else {
    let addr = dev.address.clone();
    let pair_icon: Element<'a, ItemMessage> = Icon::new("bluetooth-symbolic")
      .size(12)
      .color(style.color(theme, "color.primary", theme.primary))
      .into();
    right_items.push(clickable(pair_icon, move |_, _, _| {
      Some(BtAction::Pair(addr.clone()).message(id))
    }));
  }

  let action_row = row(right_items).spacing(6).align_y(Alignment::Center);

  let confirm_row = if confirming_remove == Some(dev.address.as_str()) {
    let addr = dev.address.clone();
    let confirm_text: Element<'a, ItemMessage> = text("Confirm removal?")
      .size(style.number("status.font.size").unwrap_or(11.0))
      .color(style.color(theme, "color.error", Color::from_rgb(0.9, 0.3, 0.3)))
      .into();
    Some(clickable(confirm_text, move |_, _, _| {
      Some(BtAction::Remove(addr.clone()).message(id))
    }))
  } else {
    None
  };

  let full_row: Element<'a, ItemMessage> = if let Some(confirm_row) = confirm_row {
    column![
      row![
        dev_icon,
        Space::new().width(6),
        dev_name,
        Space::new().width(Length::Fill),
        action_row,
      ]
      .align_y(Alignment::Center),
      row![
        Space::new().width(0),
        confirm_row,
        Space::new().width(Length::Fill),
      ],
    ]
    .spacing(2)
    .into()
  } else {
    row![
      dev_icon,
      Space::new().width(6),
      dev_name,
      Space::new().width(Length::Fill),
      action_row,
    ]
    .align_y(Alignment::Center)
    .into()
  };

  let bg = style.color(theme, "row.background", Color::TRANSPARENT);
  let row_radius = style.number("row.radius").unwrap_or(6.0);
  let row_border_width = style.number("row.border.width").unwrap_or(0.0);
  let row_border_color = style.color(theme, "row.border.color", theme.overlay);

  container(full_row)
    .padding(style.padding([4.0, 8.0]))
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(bg.into()),
      border: iced::Border {
        radius: row_radius.into(),
        width: row_border_width,
        color: row_border_color,
      },
      ..Default::default()
    })
    .into()
}
