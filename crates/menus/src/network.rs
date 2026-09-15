use iced::{
  Alignment, Color, Element, Length,
  widget::{Row, Space, column, container, row, space, text, text_input},
};
use iced_layershell::reexport::IcedId;
use slowshell_commons::network::{
  AccessPoint, ConnectionInfo, EthernetState, NetworkPayload, SharedNetworkState, signal_icon,
};
use slowshell_config::{Config, style::Style, style::Theme};
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{ItemEffect, ItemMessage},
  types::{PayloadBox, ToUstr},
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{Backdrop, Icon, Renderable, SizedPopup, clickable};

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

fn row_padding(style: &Style) -> [f32; 2] {
  [
    style.number("row.padding.y").unwrap_or(7.0),
    style.number("row.padding.x").unwrap_or(12.0),
  ]
}

fn row_container<'a>(
  content: Element<'a, ItemMessage>,
  style: &Style,
  theme: &Theme,
) -> Element<'a, ItemMessage> {
  let row_radius = style.number("row.radius").unwrap_or(6.0);
  let row_border_width = style.number("row.border.width").unwrap_or(0.0);
  let row_border_color = style.color(theme, "row.border.color", theme.overlay);
  let bg = style.color(theme, "row.background", Color::TRANSPARENT);
  container(content)
    .padding(row_padding(style))
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
    .into()
}

fn base_row<'a>(
  icon_name: &'static str,
  label: String,
  trailing: Option<Element<'a, ItemMessage>>,
  color: Color,
  icon_color: Color,
  style: &Style,
  theme: &Theme,
) -> Element<'a, ItemMessage> {
  let icon_size = style.number("icon.size").unwrap_or(16.0) as u16;
  let font_size = style.number("font.size").unwrap_or(13.0);
  let row_spacing = style.number("row.spacing").unwrap_or(8.0);

  let mut content: Row<'a, ItemMessage> = Row::new()
    .spacing(row_spacing)
    .align_y(Alignment::Center)
    .push(Icon::new(icon_name).size(icon_size).color(icon_color))
    .push(text(label).size(font_size).color(color));

  if let Some(trailing) = trailing {
    content = content
      .push(Space::new().width(Length::Fill))
      .push(trailing);
  } else {
    content = content.push(Space::new().width(Length::Fill));
  }

  row_container(content.into(), style, theme)
}

fn header_row<'a>(label: String, style: &Style, theme: &Theme) -> Element<'a, ItemMessage> {
  text(label)
    .size(style.number("header.font.size").unwrap_or(17.0))
    .color(style.color(theme, "color", theme.text))
    .into()
}

fn toggle_row<'a>(
  wifi_enabled: bool,
  id: IcedId,
  style: &Style,
  theme: &Theme,
) -> Element<'a, ItemMessage> {
  let color = style.color(theme, "color", theme.text);
  let checkbox_size = style.number("checkbox.size").unwrap_or(14.0) as u16;
  let toggle = Icon::new(if wifi_enabled {
    "checkbox-checked-symbolic"
  } else {
    "checkbox-symbolic"
  })
  .size(checkbox_size)
  .color(color);

  let status_size = style.number("status.font.size").unwrap_or(11.0);
  let faded = style.color(theme, "color.faded", theme.subtext);
  let trailing = text(if wifi_enabled { "On" } else { "Off" })
    .size(status_size)
    .color(faded);

  let row_spacing = style.number("row.spacing").unwrap_or(8.0);
  let bg = style.color(theme, "row.background", Color::TRANSPARENT);
  let row_radius = style.number("row.radius").unwrap_or(6.0);
  let row_border_width = style.number("row.border.width").unwrap_or(0.0);
  let row_border_color = style.color(theme, "row.border.color", theme.overlay);

  let content: Element<'a, ItemMessage> = container(
    row![
      toggle,
      text("WiFi")
        .size(style.number("font.size").unwrap_or(13.0))
        .color(color),
      Space::new().width(Length::Fill),
      trailing
    ]
    .spacing(row_spacing)
    .align_y(iced::Alignment::Center),
  )
  .padding(row_padding(style))
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

  clickable(content, move |_, _, _| Some(RowKind::Toggle.message(id)))
}

fn scan_row<'a>(
  scanning: bool,
  id: IcedId,
  style: &Style,
  theme: &Theme,
) -> Element<'a, ItemMessage> {
  let faded = style.color(theme, "color.faded", theme.subtext);
  let base = base_row(
    "view-refresh-symbolic",
    if scanning {
      "Scanning…".to_string()
    } else {
      "Scan for networks".to_string()
    },
    None,
    faded,
    faded,
    style,
    theme,
  );
  if scanning {
    return base;
  }
  clickable(base, move |_, _, _| Some(RowKind::Scan.message(id)))
}

fn ap_row<'a>(
  net: &AccessPoint,
  id: IcedId,
  style: &Style,
  theme: &Theme,
) -> Element<'a, ItemMessage> {
  let connected_color = style.color(theme, "connected.color", theme.green);
  let faded = style.color(theme, "color.faded", theme.subtext);
  let name_color = style.color(theme, "color", theme.text);
  let status_size = style.number("status.font.size").unwrap_or(11.0);
  let icon_size = style.number("icon.size").unwrap_or(16.0) as u16;
  let row_spacing = style.number("row.spacing").unwrap_or(8.0);
  let font_size = style.number("font.size").unwrap_or(13.0);

  let icon = Icon::new(signal_icon(net.signal))
    .size(icon_size)
    .color(if net.in_use { connected_color } else { faded });

  let name = text(net.ssid.clone()).size(font_size).color(name_color);

  let trailing: Element<'a, ItemMessage> = if net.in_use {
    text(format!("Connected · {}%", net.signal))
      .size(status_size)
      .color(connected_color)
      .into()
  } else {
    text(format!("{}%", net.signal))
      .size(status_size)
      .color(faded)
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

  let bg = style.color(theme, "row.background", Color::TRANSPARENT);
  let row_radius = style.number("row.radius").unwrap_or(6.0);
  let row_border_width = style.number("row.border.width").unwrap_or(0.0);
  let row_border_color = style.color(theme, "row.border.color", theme.overlay);
  let content: Element<'a, ItemMessage> = container(
    row![icon, name, Space::new().width(Length::Fill), trailing]
      .spacing(row_spacing)
      .align_y(iced::Alignment::Center),
  )
  .padding(row_padding(style))
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
  ssid: &str,
  password: &str,
  id: IcedId,
  style: &Style,
  theme: &Theme,
) -> Element<'a, ItemMessage> {
  let font_size = style.number("font.size").unwrap_or(13.0);
  let row_spacing = style.number("row.spacing").unwrap_or(8.0);

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
    let color = style.color(theme, "color", theme.text);
    clickable(
      base_row(
        "network-offline-symbolic",
        "Connect".to_string(),
        None,
        color,
        color,
        style,
        theme,
      ),
      move |_, _, _| Some(message_connect(id, &ssid, Some(&connect_password))),
    )
  };

  let content: Element<'a, ItemMessage> =
    column![input, connect_button].spacing(row_spacing).into();

  column![
    space().height(2),
    container(content)
      .padding(row_padding(style))
      .width(Length::Fill)
  ]
  .spacing(0)
  .into()
}

fn event_row<'a>(
  icon_name: &'static str,
  label: &str,
  kind: RowKind,
  id: IcedId,
  style: &Style,
  theme: &Theme,
) -> Element<'a, ItemMessage> {
  let color = style.color(theme, "color", theme.text);
  let base = base_row(
    icon_name,
    label.to_string(),
    None,
    color,
    color,
    style,
    theme,
  );
  clickable(base, move |_, _, _| Some(kind.clone().message(id)))
}

fn eth_row<'a>(
  eth: &EthernetState,
  id: IcedId,
  style: &Style,
  theme: &Theme,
) -> Element<'a, ItemMessage> {
  let connected_color = style.color(theme, "connected.color", theme.green);
  let faded = style.color(theme, "color.faded", theme.subtext);
  let status_size = style.number("status.font.size").unwrap_or(11.0);

  let trailing = if eth.connected {
    let label = if eth.speed > 0 {
      format!("{} Mb/s", eth.speed)
    } else {
      "Connected".to_string()
    };
    text(label).size(status_size).color(connected_color).into()
  } else if !eth.carrier {
    text("No cable").size(status_size).color(faded).into()
  } else {
    text("Disconnected").size(status_size).color(faded).into()
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
    style.color(theme, "color", theme.text),
    icon_color,
    style,
    theme,
  );

  clickable(base, move |_, _, _| Some(RowKind::Wired.message(id)))
}

fn info_text_row<'a>(
  label: &'a str,
  value: String,
  style: &Style,
  theme: &Theme,
) -> Element<'a, ItemMessage> {
  let faded = style.color(theme, "color.faded", theme.subtext);
  let color = style.color(theme, "color", theme.text);
  let font_size = style.number("font.size").unwrap_or(13.0);
  let status_size = style.number("status.font.size").unwrap_or(11.0);

  row_container(
    row![
      text(label).size(status_size).color(faded),
      Space::new().width(Length::Fill),
      text(value).size(font_size).color(color),
    ]
    .align_y(Alignment::Center)
    .into(),
    style,
    theme,
  )
}

fn back_row_net<'a>(id: IcedId, style: &Style, theme: &Theme) -> Element<'a, ItemMessage> {
  let color = style.color(theme, "color", theme.text);
  let base = base_row(
    "go-previous-symbolic",
    "Back".to_string(),
    None,
    color,
    color,
    style,
    theme,
  );
  clickable(base, move |_, _, _| Some(RowKind::InfoBack.message(id)))
}

fn info_submenu<'a>(
  conn: ConnectionInfo,
  id: IcedId,
  style: &Style,
  theme: &Theme,
) -> Vec<Element<'a, ItemMessage>> {
  let mut children = Vec::new();
  children.push(header_row(conn.label.clone(), style, theme));
  children.push(info_text_row(
    "IP Address",
    if conn.ip4.is_empty() {
      "—".to_string()
    } else {
      conn.ip4.join(", ")
    },
    style,
    theme,
  ));
  children.push(info_text_row(
    "MAC Address",
    if conn.mac.is_empty() {
      "—".to_string()
    } else {
      conn.mac.clone()
    },
    style,
    theme,
  ));
  children.push(event_row(
    "network-offline-symbolic",
    "Disconnect",
    RowKind::Disconnect,
    id,
    style,
    theme,
  ));
  children.push(event_row(
    "preferences-system-network-symbolic",
    "Edit Connection…",
    RowKind::Edit,
    id,
    style,
    theme,
  ));
  children.push(back_row_net(id, style, theme));
  children
}

impl Renderable for NetworkRenderable {
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
        children.extend(info_submenu(conn.clone(), id, &style, theme));
      }

      if children.is_empty() {
        children.push(header_row("Network".to_string(), &style, theme));

        children.push(toggle_row(wifi_enabled, id, &style, theme));

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
                .color(style.color(theme, "color.faded", theme.subtext)),
                space().height(6),
                text(if scanning {
                  "Scanning for networks…"
                } else {
                  "No networks found"
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
          for net in &inner.networks {
            if net.ssid.is_empty() {
              continue;
            }
            children.push(ap_row(net, id, &style, theme));
            if let (true, true, true) = (net.secured, !net.in_use, !net.saved)
              && selected.as_deref() == Some(net.ssid.as_str())
            {
              children.push(password_row(&net.ssid, &password, id, &style, theme));
            }
          }
        }

        children.push(scan_row(scanning, id, &style, theme));

        if let Some(eth) = &inner.ethernet {
          children.push(eth_row(eth, id, &style, theme));
        }
      }
    } else {
      children.push(
        text("network unavailable")
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
