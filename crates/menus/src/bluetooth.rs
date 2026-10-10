use iced::{
  Alignment, Color, Element, Length,
  widget::{Space, column, container, row, space},
};
use iced_exwlshell::reexport::IcedId;
use slowshell_commons::bluetooth::{BT_CHANGED, BluetoothDevice, SharedBluetoothState};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{EventFilter, ItemEffect, ItemMessage},
  types::{PayloadBox, ToUstr},
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{Backdrop, Icon, Renderable, SizedPopup, clickable, hover_row};

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
  fn events(&self, _store: &Store) -> Vec<EventFilter> {
    vec![EventFilter::Named(BT_CHANGED.into())]
  }

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

    if let Some(shared) = store.borrow::<SharedBluetoothState>() {
      let powered = shared.powered.load(std::sync::atomic::Ordering::Relaxed);
      let discovering = shared
        .discovering
        .load(std::sync::atomic::Ordering::Relaxed);
      let inner = shared.state.lock().unwrap().clone();
      let confirming_remove = {
        let menu = shared.menu.lock().unwrap();
        menu.confirming_remove.clone()
      };

      children.push(render_toggle_row(config, id, powered));

      if !powered {
        children.push(
          container(
            column![
              Icon::new("bluetooth-disabled-symbolic")
                .size(22)
                .color(theme.subtext),
              space().height(6),
              config.text("Bluetooth is off", &["menu", "text-name", "text-faded"]),
            ]
            .align_x(Alignment::Center),
          )
          .padding(16)
          .align_x(Alignment::Center)
          .width(Length::Fill)
          .into(),
        );
      } else if inner.devices.is_empty() {
        children.push(
          container(
            column![
              Icon::new(if discovering {
                "view-refresh-symbolic"
              } else {
                "bluetooth-symbolic"
              })
              .size(22)
              .color(theme.subtext),
              space().height(6),
              config.text(
                if discovering {
                  "Scanning for devices…"
                } else {
                  "No devices found"
                },
                &["menu", "text-name", "text-faded"],
              ),
            ]
            .align_x(Alignment::Center),
          )
          .padding(16)
          .align_x(Alignment::Center)
          .width(Length::Fill)
          .into(),
        );
      } else {
        let mut has_connected = false;
        for dev in inner.devices.iter().filter(|d| d.connected) {
          if !has_connected {
            children.push(section_header(config, "Connected"));
            has_connected = true;
          }
          children.push(render_device(config, dev, id, confirming_remove.as_deref()));
        }

        let mut has_other = false;
        for dev in inner.devices.iter().filter(|d| !d.connected) {
          if !has_other {
            children.push(section_header(config, "Devices"));
            has_other = true;
          }
          children.push(render_device(config, dev, id, confirming_remove.as_deref()));
        }
      }

      if powered {
        children.push(scan_footer_row(config, discovering, id));
      }
    } else {
      children.push(
        config
          .text(
            "Bluetooth unavailable",
            &["menu", "text-status", "text-faded"],
          )
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

fn render_toggle_row<'a>(
  config: &'a Config,
  id: IcedId,
  powered: bool,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();

  let checkbox_size = config
    .computed(&["menu", "checkbox"])
    .length("checkbox-size")
    .unwrap_or(14.0) as u16;
  let toggle_color = if powered { theme.primary } else { theme.text };
  let toggle = Icon::new(if powered {
    "checkbox-checked-symbolic"
  } else {
    "checkbox-symbolic"
  })
  .size(checkbox_size)
  .color(toggle_color);

  let trailing = config.text(
    if powered { "On" } else { "Off" },
    &["menu", "text-status", "text-faded"],
  );

  let row_spacing = config.sizing(&["menu", "row"]).gap;

  let content: Element<'a, ItemMessage> = hover_row(
    config,
    &["menu", "row"],
    container(
      row![
        toggle,
        config.text("Bluetooth", &["menu", "text-body"]),
        Space::new().width(Length::Fill),
        trailing
      ]
      .spacing(row_spacing)
      .align_y(Alignment::Center),
    )
    .padding(config.sizing(&["menu", "row"]).padding())
    .width(Length::Fill),
  );

  clickable(content, move |_, _, _| Some(BtAction::Toggle.message(id)))
}

fn section_header<'a>(config: &'a Config, title: &'static str) -> Element<'a, ItemMessage> {
  config
    .text(title, &["menu", "text-status", "text-faded"])
    .into()
}

fn scan_footer_row<'a>(
  config: &'a Config,
  discovering: bool,
  id: IcedId,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();

  let icon_size = config
    .computed(&["menu", "icon"])
    .length("icon-size")
    .unwrap_or(16.0) as u16;
  let content = row![
    Icon::new("view-refresh-symbolic")
      .size(icon_size)
      .color(theme.subtext),
    Space::new().width(6),
    config.text(
      if discovering {
        "Scanning…"
      } else {
        "Scan for devices…"
      },
      &["menu", "text-name", "text-faded"],
    ),
    Space::new().width(Length::Fill),
  ]
  .align_y(Alignment::Center);

  let wrapped = hover_row(
    config,
    &["menu", "row"],
    container(content)
      .padding(config.sizing(&["menu", "row"]).padding())
      .width(Length::Fill),
  );

  if discovering {
    return wrapped;
  }
  clickable(wrapped.into(), move |_, _, _| {
    Some(BtAction::Scan.message(id))
  })
  .into()
}

fn render_device<'a>(
  config: &'a Config,
  dev: &BluetoothDevice,
  id: IcedId,
  confirming_remove: Option<&str>,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();

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
    theme.primary
  } else {
    theme.subtext
  };

  let dev_icon: Element<'a, ItemMessage> =
    Icon::new(dev_icon_name).size(14).color(icon_color).into();

  let dev_name = config.text(dev.name.clone(), &["menu", "text-name"]);

  let mut right_items: Vec<Element<'a, ItemMessage>> = Vec::new();

  if let Some(bat) = dev.battery {
    right_items.push(
      config
        .text(format!("{bat}%"), &["menu", "text-status", "text-faded"])
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
      .color(theme.subtext)
      .into()
  } else {
    Icon::new("network-connect-symbolic")
      .size(12)
      .color(theme.primary)
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
      Color::from_rgb(0.9, 0.3, 0.3)
    } else {
      theme.subtext
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
      .color(theme.primary)
      .into();
    right_items.push(clickable(pair_icon, move |_, _, _| {
      Some(BtAction::Pair(addr.clone()).message(id))
    }));
  }

  let action_row = row(right_items).spacing(6).align_y(Alignment::Center);

  let confirm_row = if confirming_remove == Some(dev.address.as_str()) {
    let addr = dev.address.clone();
    let confirm_text: Element<'a, ItemMessage> = config
      .text("Confirm removal?", &["menu", "text-status"])
      .color(Color::from_rgb(0.9, 0.3, 0.3))
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

  hover_row(
    config,
    &["menu", "row"],
    container(full_row)
      .padding(config.sizing(&["menu", "row"]).padding())
      .width(Length::Fill),
  )
}
