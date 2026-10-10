use iced::{
  Alignment, Color, Element, Length,
  widget::{Row, Space, column, container, row, space, text_input},
};
use iced_exwlshell::reexport::IcedId;
use slowshell_commons::network::{
  AccessPoint, ConnectionInfo, EthernetState, NetworkPayload, SharedNetworkState, signal_icon,
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{EventFilter, ItemEffect, ItemMessage},
  types::{PayloadBox, ToUstr},
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{Backdrop, Icon, Renderable, SizedPopup, clickable, hover_row};

pub struct NetworkRenderable;

#[derive(Debug, Clone)]
enum RowKind {
  Toggle,
  Scan,
  Connect { ssid: String },
  Select { ssid: String },
  Info { ssid: String },
  InfoBack,
  Disconnect,
  Edit,
  Wired,
}

impl RowKind {
  fn message(self, id: IcedId) -> ItemMessage {
    let (effect, name) = match self {
      RowKind::Toggle => (ItemEffect::Redraw, "network.toggle"),
      RowKind::Scan => (ItemEffect::Redraw, "network.scan"),
      RowKind::Connect { ssid } => {
        return message_connect(id, &ssid, None);
      }
      RowKind::Select { ssid } => {
        return ItemMessage::EffectAction(
          id,
          ItemEffect::Redraw,
          ListenerAction::Payload {
            name: "network.select".to_ustr(),
            payload: Some(PayloadBox::new(NetworkPayload {
              ssid: ssid,
              ..Default::default()
            })),
          },
        );
      }
      RowKind::Info { ssid } => {
        return ItemMessage::EffectAction(
          id,
          ItemEffect::Redraw,
          ListenerAction::Payload {
            name: "network.info".to_ustr(),
            payload: Some(PayloadBox::new(NetworkPayload {
              ssid: ssid,
              ..Default::default()
            })),
          },
        );
      }
      RowKind::InfoBack => (ItemEffect::Redraw, "network.info.back"),
      RowKind::Disconnect => (ItemEffect::Hide, "network.disconnect"),
      RowKind::Edit => (ItemEffect::Hide, "network.edit"),
      RowKind::Wired => (ItemEffect::Hide, "network.wired"),
    };
    ItemMessage::EffectAction(
      id,
      effect,
      ListenerAction::Payload {
        name: name.to_ustr(),
        payload: None,
      },
    )
  }
}

fn message_connect(id: IcedId, ssid: &str, password: Option<&str>) -> ItemMessage {
  ItemMessage::EffectAction(
    id,
    ItemEffect::Hide,
    ListenerAction::Payload {
      name: "network.connect".to_ustr(),
      payload: Some(PayloadBox::new(NetworkPayload {
        password: password.map(|s| s.to_string()),
        ssid: ssid.to_string(),
        ..Default::default()
      })),
    },
  )
}

fn row_container<'a>(
  config: &'a Config,
  content: Element<'a, ItemMessage>,
) -> Element<'a, ItemMessage> {
  let compact = &["menu", "row", "compact"];
  hover_row(
    config,
    compact,
    container(content)
      .width(Length::Fill)
      .padding(config.sizing(compact).padding()),
  )
}

fn base_row<'a>(
  config: &'a Config,
  icon_name: &'static str,
  label: String,
  trailing: Option<Element<'a, ItemMessage>>,
  color: Color,
  icon_color: Color,
) -> Element<'a, ItemMessage> {
  let style = config.computed(&["menu"]);
  let icon_size = style.length("icon-size").unwrap_or(16.0) as u16;
  let row_spacing = config.sizing(&["menu", "row", "compact"]).gap;

  let mut content: Row<'a, ItemMessage> = Row::new()
    .spacing(row_spacing)
    .align_y(Alignment::Center)
    .push(Icon::new(icon_name).size(icon_size).color(icon_color))
    .push(config.text(label, &["menu"]).color(color));

  if let Some(trailing) = trailing {
    content = content
      .push(Space::new().width(Length::Fill))
      .push(trailing);
  } else {
    content = content.push(Space::new().width(Length::Fill));
  }

  row_container(config, content.into())
}

fn header_row<'a>(config: &'a Config, label: String) -> Element<'a, ItemMessage> {
  config.text(label, &["menu", "text-header"]).into()
}

fn toggle_row<'a>(config: &'a Config, wifi_enabled: bool, id: IcedId) -> Element<'a, ItemMessage> {
  let style = config.computed(&["menu"]);
  let theme = config.palette();
  let color = style
    .color("color")
    .map(|c| c.to_iced())
    .unwrap_or(theme.text);
  let checkbox_size = style.length("checkbox-size").unwrap_or(14.0) as u16;
  let toggle = Icon::new(if wifi_enabled {
    "checkbox-checked-symbolic"
  } else {
    "checkbox-symbolic"
  })
  .size(checkbox_size)
  .color(color);

  let trailing = config.text(
    if wifi_enabled { "On" } else { "Off" },
    &["menu", "text-status", "text-faded"],
  );

  let row_spacing = config.sizing(&["menu", "row", "compact"]).gap;

  let compact = &["menu", "row", "compact"];
  let content: Element<'a, ItemMessage> = hover_row(
    config,
    compact,
    container(
      row![
        toggle,
        config.text("WiFi", &["menu"]).color(color),
        Space::new().width(Length::Fill),
        trailing
      ]
      .spacing(row_spacing)
      .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .padding(config.sizing(compact).padding()),
  );

  clickable(content, move |_, _, _| Some(RowKind::Toggle.message(id)))
}

fn scan_row<'a>(config: &'a Config, scanning: bool, id: IcedId) -> Element<'a, ItemMessage> {
  let faded = config
    .color(&["menu"], "color-faded")
    .unwrap_or(config.palette().subtext);
  let base = base_row(
    config,
    "view-refresh-symbolic",
    if scanning {
      "Scanning…".to_string()
    } else {
      "Scan for networks".to_string()
    },
    None,
    faded,
    faded,
  );
  if scanning {
    return base;
  }
  clickable(base, move |_, _, _| Some(RowKind::Scan.message(id)))
}

fn ap_row<'a>(config: &'a Config, net: &AccessPoint, id: IcedId) -> Element<'a, ItemMessage> {
  let style = config.computed(&["menu"]);
  let theme = config.palette();
  let connected_color = style
    .color("connected-color")
    .map(|c| c.to_iced())
    .unwrap_or(theme.green);
  let faded = style
    .color("color-faded")
    .map(|c| c.to_iced())
    .unwrap_or(theme.subtext);
  let icon_size = style.length("icon-size").unwrap_or(16.0) as u16;
  let row_spacing = config.sizing(&["menu", "row", "compact"]).gap;

  let icon = Icon::new(signal_icon(net.signal))
    .size(icon_size)
    .color(if net.in_use { connected_color } else { faded });

  let name = config.text(net.ssid.clone(), &["menu"]);

  let trailing: Element<'a, ItemMessage> = if net.in_use {
    config
      .text(
        format!("Connected · {}%", net.signal),
        &["menu", "text-status"],
      )
      .color(connected_color)
      .into()
  } else {
    config
      .text(
        format!("{}%", net.signal),
        &["menu", "text-status", "text-faded"],
      )
      .into()
  };

  let name: Element<'a, ItemMessage> = if net.secured {
    let lock = Icon::new("network-wireless-encrypted-symbolic")
      .size((icon_size - 4).max(8))
      .color(faded);
    row![name, lock].spacing(4).into()
  } else {
    name.into()
  };

  let content: Element<'a, ItemMessage> = hover_row(
    config,
    &["menu", "row", "compact"],
    container(
      row![icon, name, Space::new().width(Length::Fill), trailing]
        .spacing(row_spacing)
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .padding(config.sizing(&["menu", "row", "compact"]).padding()),
  );

  let kind = if net.in_use {
    RowKind::Info {
      ssid: net.ssid.clone(),
    }
  } else if !net.secured || net.saved {
    RowKind::Connect {
      ssid: net.ssid.clone(),
    }
  } else {
    RowKind::Select {
      ssid: net.ssid.clone(),
    }
  };

  clickable(content, move |_, _, _| Some(kind.clone().message(id)))
}

fn password_row<'a>(
  config: &'a Config,
  ssid: &str,
  password: &str,
  id: IcedId,
) -> Element<'a, ItemMessage> {
  let style = config.computed(&["menu"]);
  let theme = config.palette();
  let font_size = config.sizing(&["menu"]).font_size;
  let row_spacing = config.sizing(&["menu", "row", "compact"]).gap;

  let password = password.to_string();
  let input = text_input("Password", &password)
    .secure(true)
    .size(font_size)
    .width(Length::Fill)
    .padding(6)
    .on_input(move |value| {
      ItemMessage::EffectAction(
        id,
        ItemEffect::Redraw,
        ListenerAction::Payload {
          name: "network.password".to_ustr(),
          payload: Some(PayloadBox::new(NetworkPayload {
            value: Some(value),
            ..Default::default()
          })),
        },
      )
    })
    .on_submit(message_connect(id, ssid, Some(&password)));

  let ssid = ssid.to_string();
  let connect_password = password.clone();
  let connect_button = {
    let color = style
      .color("color")
      .map(|c| c.to_iced())
      .unwrap_or(theme.text);
    clickable(
      base_row(
        config,
        "network-offline-symbolic",
        "Connect".to_string(),
        None,
        color,
        color,
      ),
      move |_, _, _| Some(message_connect(id, &ssid, Some(&connect_password))),
    )
  };

  let content: Element<'a, ItemMessage> =
    column![input, connect_button].spacing(row_spacing).into();

  column![
    space().height(2),
    hover_row(
      config,
      &["menu", "row", "compact"],
      container(content)
        .width(Length::Fill)
        .padding(config.sizing(&["menu", "row", "compact"]).padding()),
    )
  ]
  .spacing(0)
  .into()
}

fn event_row<'a>(
  config: &'a Config,
  icon_name: &'static str,
  label: &str,
  kind: RowKind,
  id: IcedId,
) -> Element<'a, ItemMessage> {
  let color = config
    .color(&["menu"], "color")
    .unwrap_or(config.palette().text);
  let base = base_row(config, icon_name, label.to_string(), None, color, color);
  clickable(base, move |_, _, _| Some(kind.clone().message(id)))
}

fn eth_row<'a>(config: &'a Config, eth: &EthernetState, id: IcedId) -> Element<'a, ItemMessage> {
  let style = config.computed(&["menu"]);
  let theme = config.palette();
  let connected_color = style
    .color("connected-color")
    .map(|c| c.to_iced())
    .unwrap_or(theme.green);
  let faded = style
    .color("color-faded")
    .map(|c| c.to_iced())
    .unwrap_or(theme.subtext);

  let trailing = if eth.connected {
    let label = if eth.speed > 0 {
      format!("{} Mb/s", eth.speed)
    } else {
      "Connected".to_string()
    };
    config
      .text(label, &["menu", "text-status"])
      .color(connected_color)
      .into()
  } else if !eth.carrier {
    config
      .text("No cable", &["menu", "text-status", "text-faded"])
      .into()
  } else {
    config
      .text("Disconnected", &["menu", "text-status", "text-faded"])
      .into()
  };

  let icon_name = if eth.connected {
    "network-wired-symbolic"
  } else {
    "network-symbolic"
  };
  let icon_color = if eth.connected {
    connected_color
  } else {
    faded
  };

  let base = base_row(
    config,
    icon_name,
    format!(
      "Ethernet{}",
      if eth.iface.is_empty() {
        String::new()
      } else {
        format!(" ({})", eth.iface)
      }
    ),
    Some(trailing),
    style
      .color("color")
      .map(|c| c.to_iced())
      .unwrap_or(theme.text),
    icon_color,
  );

  clickable(base, move |_, _, _| Some(RowKind::Wired.message(id)))
}

fn info_text_row<'a>(
  config: &'a Config,
  label: &'a str,
  value: String,
) -> Element<'a, ItemMessage> {
  row_container(
    config,
    row![
      config.text(label, &["menu", "text-status", "text-faded"]),
      Space::new().width(Length::Fill),
      config.text(value, &["menu"]),
    ]
    .align_y(Alignment::Center)
    .into(),
  )
}

fn back_row_net<'a>(config: &'a Config, id: IcedId) -> Element<'a, ItemMessage> {
  let color = config
    .color(&["menu"], "color")
    .unwrap_or(config.palette().text);
  let base = base_row(
    config,
    "go-previous-symbolic",
    "Back".to_string(),
    None,
    color,
    color,
  );
  clickable(base, move |_, _, _| Some(RowKind::InfoBack.message(id)))
}

fn info_submenu<'a>(
  config: &'a Config,
  conn: ConnectionInfo,
  id: IcedId,
) -> Vec<Element<'a, ItemMessage>> {
  let mut children = Vec::new();
  children.push(header_row(config, conn.label.clone()));
  children.push(info_text_row(
    config,
    "IP Address",
    if conn.ip4.is_empty() {
      "—".to_string()
    } else {
      conn.ip4.join(", ")
    },
  ));
  children.push(info_text_row(
    config,
    "MAC Address",
    if conn.mac.is_empty() {
      "—".to_string()
    } else {
      conn.mac.clone()
    },
  ));
  children.push(event_row(
    config,
    "network-offline-symbolic",
    "Disconnect",
    RowKind::Disconnect,
    id,
  ));
  children.push(event_row(
    config,
    "preferences-system-network-symbolic",
    "Edit Connection…",
    RowKind::Edit,
    id,
  ));
  children.push(back_row_net(config, id));
  children
}

impl Renderable for NetworkRenderable {
  fn events(&self, _store: &Store) -> Vec<EventFilter> {
    vec![
      EventFilter::Named("component/network.changed".into()),
      EventFilter::Payload("network.password".into()),
    ]
  }

  fn on_event(
    &mut self,
    _config: &Config,
    _store: &mut Store,
    _event: &ListenerAction,
  ) -> Option<ItemEffect> {
    Some(ItemEffect::Redraw)
  }

  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    data: &'a dyn std::any::Any,
  ) -> Element<'a, ItemMessage> {
    let data = data.downcast_ref::<PopupSettings>().unwrap();
    let style = config.computed(&["menu"]);
    let theme = config.palette();

    let mut children: Vec<Element<'a, ItemMessage>> = Vec::new();

    if let Some(shared) = store.borrow::<SharedNetworkState>() {
      let inner = shared.state.lock().unwrap().clone();
      let wifi_enabled = shared
        .wifi_enabled
        .load(std::sync::atomic::Ordering::Relaxed);
      let scanning = shared.scanning.load(std::sync::atomic::Ordering::Relaxed);
      let (selected, password, info_ssid) = {
        let menu = shared.menu.lock().unwrap();
        (
          menu.selected_ssid.clone(),
          menu.password.clone(),
          menu.info_ssid.clone(),
        )
      };

      if let (Some(info_ssid), Some(conn)) = (info_ssid, &inner.connected)
        && conn.is_wifi
        && conn.label == info_ssid
      {
        children.extend(info_submenu(config, conn.clone(), id));
      }

      if children.is_empty() {
        children.push(header_row(config, "Network".to_string()));

        children.push(toggle_row(config, wifi_enabled, id));

        if inner.networks.is_empty() {
          children.push(
            container(
              column![
                Icon::new(if scanning {
                  "view-refresh-symbolic"
                } else {
                  "network-wireless-signal-none-symbolic"
                })
                .size(22)
                .color(
                  style
                    .color("color-faded")
                    .map(|c| c.to_iced())
                    .unwrap_or(theme.subtext)
                ),
                space().height(6),
                config.text(
                  if scanning {
                    "Scanning for networks…"
                  } else {
                    "No networks found"
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
          for net in &inner.networks {
            if net.ssid.is_empty() {
              continue;
            }
            children.push(ap_row(config, net, id));
            if let (true, true, true) = (net.secured, !net.in_use, !net.saved)
              && selected.as_deref() == Some(net.ssid.as_str())
            {
              children.push(password_row(config, &net.ssid, &password, id));
            }
          }
        }

        children.push(scan_row(config, scanning, id));

        if let Some(eth) = &inner.ethernet {
          children.push(eth_row(config, eth, id));
        }
      }
    } else {
      children.push(
        config
          .text(
            "network unavailable",
            &["menu", "text-status", "text-faded"],
          )
          .into(),
      );
    }

    let list_spacing = config.sizing(&["menu", "list"]).gap;
    let list_width = style.length("list-width").unwrap_or(220.0);
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
