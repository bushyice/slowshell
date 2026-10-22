use std::sync::atomic::Ordering;

use iced::{
  Alignment, Color, Element, Length,
  widget::{Space, column, container, row, space},
};
use iced_exwlshell::reexport::IcedId;
use slowshell_commons::bluetooth::{
  BT_CHANGED, BluetoothCmd, BluetoothDevice, SharedBluetoothState,
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  message::{EventFilter, ItemEffect, ItemMessage},
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{Backdrop, Icon, Renderable, SizedPopup, clickable, hover_row};

pub struct BluetoothRenderable;

impl Renderable for BluetoothRenderable {
  fn events(&self, _store: &Store) -> Vec<EventFilter> {
    vec![EventFilter::Named(BT_CHANGED.into())]
  }

  fn initialize(&self, _config: &Config, _store: &mut Store) -> Box<dyn std::any::Any + Send> {
    Box::new(())
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
      let powered = shared.powered.load(Ordering::Relaxed);
      let discovering = shared.discovering.load(Ordering::Relaxed);
      let inner = shared.state.lock().unwrap().clone();
      let (confirming_remove, connecting_device) = {
        let menu = shared.menu.lock().unwrap();
        (
          menu.confirming_remove.clone(),
          menu.connecting_device.clone(),
        )
      };

      children.push(render_toggle_row(config, shared, id, powered));

      if !powered {
        children.push(
          container(
            column![
              Icon::new("bluetooth-disabled-symbolic")
                .size(24)
                .color(theme.subtext),
              space().height(6),
              config.text("Bluetooth is off", &["menu", "text-name", "text-faded"]),
            ]
            .align_x(Alignment::Center),
          )
          .padding(20)
          .align_x(Alignment::Center)
          .width(Length::Fill)
          .into(),
        );
      } else {
        children.push(render_discoverable_row(
          config,
          shared,
          id,
          inner.discoverable,
        ));

        children.push(render_scan_row(config, shared, id, discovering));

        if inner.devices.is_empty() {
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
            let is_connecting = connecting_device.as_deref() == Some(dev.address.as_str());
            children.push(render_device(
              config,
              shared,
              dev,
              id,
              confirming_remove.as_deref(),
              is_connecting,
            ));
          }

          let mut has_paired = false;
          for dev in inner.devices.iter().filter(|d| !d.connected && d.paired) {
            if !has_paired {
              children.push(section_header(config, "Paired Devices"));
              has_paired = true;
            }
            let is_connecting = connecting_device.as_deref() == Some(dev.address.as_str());
            children.push(render_device(
              config,
              shared,
              dev,
              id,
              confirming_remove.as_deref(),
              is_connecting,
            ));
          }

          let mut has_available = false;
          for dev in inner.devices.iter().filter(|d| !d.connected && !d.paired) {
            if !has_available {
              children.push(section_header(config, "Available Devices"));
              has_available = true;
            }
            let is_connecting = connecting_device.as_deref() == Some(dev.address.as_str());
            children.push(render_device(
              config,
              shared,
              dev,
              id,
              confirming_remove.as_deref(),
              is_connecting,
            ));
          }
        }
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
    let list_width = list_style.length("list-width").unwrap_or(280.0);
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
    .with_panel_insets(store, data.monitor.as_deref())
    .with_on_close(ItemMessage::Effect(id, ItemEffect::Hide))
    .into()
  }
}

fn render_toggle_row<'a>(
  config: &'a Config,
  shared: &SharedBluetoothState,
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

  let cmd_tx = shared.cmd_tx.clone();
  clickable(content, move |_, _, _| {
    let _ = cmd_tx.send(BluetoothCmd::TogglePower);
    Some(ItemMessage::Effect(id, ItemEffect::Redraw))
  })
}

fn render_discoverable_row<'a>(
  config: &'a Config,
  shared: &SharedBluetoothState,
  id: IcedId,
  discoverable: bool,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();

  let checkbox_size = config
    .computed(&["menu", "checkbox"])
    .length("checkbox-size")
    .unwrap_or(14.0) as u16;
  let toggle_color = if discoverable {
    theme.primary
  } else {
    theme.text
  };
  let toggle = Icon::new(if discoverable {
    "checkbox-checked-symbolic"
  } else {
    "checkbox-symbolic"
  })
  .size(checkbox_size)
  .color(toggle_color);

  let trailing = config.text(
    if discoverable { "Visible" } else { "Hidden" },
    &["menu", "text-status", "text-faded"],
  );

  let row_spacing = config.sizing(&["menu", "row"]).gap;

  let content: Element<'a, ItemMessage> = hover_row(
    config,
    &["menu", "row"],
    container(
      row![
        toggle,
        config.text("Visible to others", &["menu", "text-body"]),
        Space::new().width(Length::Fill),
        trailing
      ]
      .spacing(row_spacing)
      .align_y(Alignment::Center),
    )
    .padding(config.sizing(&["menu", "row"]).padding())
    .width(Length::Fill),
  );

  let cmd_tx = shared.cmd_tx.clone();
  clickable(content, move |_, _, _| {
    let _ = cmd_tx.send(BluetoothCmd::ToggleDiscoverable);
    Some(ItemMessage::Effect(id, ItemEffect::Redraw))
  })
}

fn render_scan_row<'a>(
  config: &'a Config,
  shared: &SharedBluetoothState,
  id: IcedId,
  discovering: bool,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();

  let icon_size = config
    .computed(&["menu", "icon"])
    .length("icon-size")
    .unwrap_or(15.0) as u16;
  let content = row![
    Icon::new("view-refresh-symbolic")
      .size(icon_size)
      .color(if discovering {
        theme.primary
      } else {
        theme.subtext
      }),
    Space::new().width(6),
    config.text(
      if discovering {
        "Stop scan"
      } else {
        "Scan for devices"
      },
      &["menu", "text-name", "text-faded"],
    ),
    Space::new().width(Length::Fill),
    if discovering {
      config.text("Scanning…", &["menu", "text-status", "text-faded"])
    } else {
      config.text("", &["menu", "text-status"])
    }
  ]
  .align_y(Alignment::Center);

  let wrapped = hover_row(
    config,
    &["menu", "row"],
    container(content)
      .padding(config.sizing(&["menu", "row"]).padding())
      .width(Length::Fill),
  );

  let cmd_tx = shared.cmd_tx.clone();
  clickable(wrapped.into(), move |_, _, _| {
    if discovering {
      let _ = cmd_tx.send(BluetoothCmd::StopDiscovery);
    } else {
      let _ = cmd_tx.send(BluetoothCmd::StartDiscovery);
    }
    Some(ItemMessage::Effect(id, ItemEffect::Redraw))
  })
}

fn section_header<'a>(config: &'a Config, title: &'static str) -> Element<'a, ItemMessage> {
  config
    .text(title, &["menu", "text-status", "text-faded"])
    .into()
}

fn render_device<'a>(
  config: &'a Config,
  shared: &SharedBluetoothState,
  dev: &BluetoothDevice,
  id: IcedId,
  confirming_remove: Option<&str>,
  connecting: bool,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();

  let dev_icon_name = dev.icon.as_deref().unwrap_or_else(|| {
    let lower = dev.name.to_lowercase();
    if lower.contains("mouse") {
      "input-mouse-symbolic"
    } else if lower.contains("keyboard") {
      "input-keyboard-symbolic"
    } else if lower.contains("headset")
      || lower.contains("headphone")
      || lower.contains("buds")
      || lower.contains("airpod")
    {
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
  } else if connecting {
    theme.yellow
  } else {
    theme.subtext
  };

  let dev_icon: Element<'a, ItemMessage> =
    Icon::new(dev_icon_name).size(15).color(icon_color).into();

  let dev_name = config.text(dev.name.clone(), &["menu", "text-name"]);

  let mut subtitle_parts: Vec<String> = Vec::new();
  if connecting {
    subtitle_parts.push("Connecting…".to_string());
  } else if dev.connected {
    subtitle_parts.push("Connected".to_string());
    if let Some(bat) = dev.battery {
      subtitle_parts.push(format!("{bat}%"));
    }
  } else if dev.paired {
    subtitle_parts.push("Paired".to_string());
  } else {
    subtitle_parts.push("Available".to_string());
  }

  if dev.trusted {
    subtitle_parts.push("Trusted".to_string());
  }

  let subtitle_text = subtitle_parts.join(" • ");
  let subtitle_elem = config
    .text(subtitle_text, &["menu", "text-status", "text-faded"])
    .size(11.0);

  let info_col = column![dev_name, subtitle_elem].spacing(1);

  let cmd_tx = shared.cmd_tx.clone();
  let addr = dev.address.clone();
  let connected = dev.connected;
  let paired = dev.paired;

  let left_area = clickable(
    row![dev_icon, Space::new().width(8), info_col]
      .align_y(Alignment::Center)
      .width(Length::Fill)
      .into(),
    move |_, _, _| {
      if connected {
        let _ = cmd_tx.send(BluetoothCmd::Disconnect(addr.clone()));
      } else if paired {
        let _ = cmd_tx.send(BluetoothCmd::Connect(addr.clone()));
      } else {
        let _ = cmd_tx.send(BluetoothCmd::Pair(addr.clone()));
      }
      Some(ItemMessage::Effect(id, ItemEffect::Redraw))
    },
  );

  let mut right_items: Vec<Element<'a, ItemMessage>> = Vec::new();

  let cmd_tx_connect = shared.cmd_tx.clone();
  let addr_connect = dev.address.clone();
  let (action_icon_name, action_color) = if dev.connected {
    ("media-playback-stop-symbolic", theme.subtext)
  } else {
    ("network-connect-symbolic", theme.primary)
  };
  let action_btn = clickable(
    container(Icon::new(action_icon_name).size(13).color(action_color))
      .padding(4)
      .into(),
    move |_, _, _| {
      if connected {
        let _ = cmd_tx_connect.send(BluetoothCmd::Disconnect(addr_connect.clone()));
      } else {
        let _ = cmd_tx_connect.send(BluetoothCmd::Connect(addr_connect.clone()));
      }
      Some(ItemMessage::Effect(id, ItemEffect::Redraw))
    },
  );
  right_items.push(action_btn);

  if dev.paired || dev.connected {
    let cmd_tx_trust = shared.cmd_tx.clone();
    let addr_trust = dev.address.clone();
    let trusted = dev.trusted;
    let trust_icon_color = if trusted {
      theme.primary
    } else {
      Color {
        a: 0.35,
        ..theme.subtext
      }
    };
    let trust_btn = clickable(
      container(
        Icon::new("security-high-symbolic")
          .size(13)
          .color(trust_icon_color),
      )
      .padding(4)
      .into(),
      move |_, _, _| {
        let _ = cmd_tx_trust.send(BluetoothCmd::Trust(addr_trust.clone(), !trusted));
        Some(ItemMessage::Effect(id, ItemEffect::Redraw))
      },
    );
    right_items.push(trust_btn);
  }

  let is_confirming = confirming_remove == Some(dev.address.as_str());
  if dev.paired {
    let cmd_tx_remove = shared.cmd_tx.clone();
    let shared_menu = shared.clone();
    let addr_remove = dev.address.clone();
    let remove_color = if is_confirming {
      theme.red
    } else {
      theme.subtext
    };
    let remove_btn = clickable(
      container(
        Icon::new(if is_confirming {
          "edit-delete-symbolic"
        } else {
          "list-remove-symbolic"
        })
        .size(13)
        .color(remove_color),
      )
      .padding(4)
      .into(),
      move |_, _, _| {
        if is_confirming {
          let _ = cmd_tx_remove.send(BluetoothCmd::Remove(addr_remove.clone()));
          let mut menu = shared_menu.menu.lock().unwrap();
          menu.confirming_remove = None;
        } else {
          let mut menu = shared_menu.menu.lock().unwrap();
          menu.confirming_remove = Some(addr_remove.clone());
        }
        Some(ItemMessage::Effect(id, ItemEffect::Redraw))
      },
    );
    right_items.push(remove_btn);
  }

  let actions_row = row(right_items).spacing(2).align_y(Alignment::Center);

  let confirm_banner: Option<Element<'a, ItemMessage>> = if is_confirming {
    let cmd_tx_forget = shared.cmd_tx.clone();
    let shared_menu = shared.clone();
    let addr_forget = dev.address.clone();

    let forget_btn = clickable(
      config
        .text("Forget device", &["menu", "text-status"])
        .color(theme.red)
        .size(11.0)
        .into(),
      move |_, _, _| {
        let _ = cmd_tx_forget.send(BluetoothCmd::Remove(addr_forget.clone()));
        let mut menu = shared_menu.menu.lock().unwrap();
        menu.confirming_remove = None;
        Some(ItemMessage::Effect(id, ItemEffect::Redraw))
      },
    );

    let shared_cancel = shared.clone();
    let cancel_btn = clickable(
      config
        .text("Cancel", &["menu", "text-status", "text-faded"])
        .size(11.0)
        .into(),
      move |_, _, _| {
        let mut menu = shared_cancel.menu.lock().unwrap();
        menu.confirming_remove = None;
        Some(ItemMessage::Effect(id, ItemEffect::Redraw))
      },
    );

    Some(
      row![forget_btn, Space::new().width(8), cancel_btn]
        .align_y(Alignment::Center)
        .into(),
    )
  } else {
    None
  };

  let main_row = row![left_area, actions_row]
    .align_y(Alignment::Center)
    .width(Length::Fill);

  let row_content: Element<'a, ItemMessage> = if let Some(banner) = confirm_banner {
    column![main_row, space().height(2), banner].into()
  } else {
    main_row.into()
  };

  hover_row(
    config,
    &["menu", "row"],
    container(row_content)
      .padding(config.sizing(&["menu", "row"]).padding())
      .width(Length::Fill),
  )
}
