use iced::{
  Color, Element, Length,
  widget::{Space, column, container, row, text},
};
use iced_layershell::reexport::IcedId;
use slowshell_core::{
  Store,
  message::{ItemEffect, ItemMessage},
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{Backdrop, Renderable, SizedPopup};

pub struct WifiRenderable;

struct WifiNetwork {
  name: &'static str,
  signal: u8,
  connected: bool,
}

const NETWORKS: &[WifiNetwork] = &[
  WifiNetwork {
    name: "A network",
    signal: 92,
    connected: true,
  },
  WifiNetwork {
    name: "A WiFi",
    signal: 68,
    connected: false,
  },
  WifiNetwork {
    name: "Wifi",
    signal: 45,
    connected: false,
  },
  WifiNetwork {
    name: "Whai Fhai",
    signal: 31,
    connected: false,
  },
  WifiNetwork {
    name: "Network",
    signal: 74,
    connected: false,
  },
];

// temporary
fn signal_icon(strength: u8) -> &'static str {
  match strength {
    80..=100 => "󰤨",
    60..=79 => "󰤥",
    40..=59 => "󰤢",
    20..=39 => "󰤟",
    _ => "󰤯",
  }
}

fn network_row<'a>(net: &WifiNetwork) -> Element<'a, ItemMessage> {
  let icon = text(signal_icon(net.signal))
    .size(16)
    .color(if net.connected {
      Color::from_rgb(0.4, 0.8, 0.4)
    } else {
      Color::from_rgba(1.0, 1.0, 1.0, 0.7)
    });

  let name = text(net.name).size(13).color(Color::WHITE);

  let status = if net.connected {
    text("Conected")
      .size(11)
      .color(Color::from_rgb(0.4, 0.8, 0.4))
  } else {
    text(format!("{}%", net.signal))
      .size(11)
      .color(Color::from_rgba(1.0, 1.0, 1.0, 0.4))
  };

  container(
    row![icon, name, Space::new().width(Length::Fill), status]
      .spacing(8)
      .align_y(iced::Alignment::Center),
  )
  .padding([8, 12])
  .width(Length::Fill)
  .style(|_t: &iced::Theme| container::Style {
    background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.04).into()),
    border: iced::Border {
      radius: 6.0.into(),
      ..Default::default()
    },
    ..container::Style::default()
  })
  .into()
}

impl Renderable for WifiRenderable {
  fn view<'a>(
    &self,
    store: &Store,
    id: IcedId,
    data: &dyn std::any::Any,
  ) -> Element<'a, ItemMessage> {
    let data = data.downcast_ref::<PopupSettings>().unwrap();
    let header = text("WiFi").size(15).color(Color::WHITE);

    let networks: Vec<Element<'a, ItemMessage>> = NETWORKS.iter().map(|n| network_row(n)).collect();

    let list = column(networks).spacing(4);

    SizedPopup::new(container(
      container(
        column![header, list]
          .spacing(12)
          .width(Length::Fixed(280.0)),
      )
      .padding(16)
      .style(|_t: &iced::Theme| container::Style {
        background: Some(Color::from_rgba(0.12, 0.12, 0.14, 0.95).into()),
        border: iced::Border {
          radius: 12.0.into(),
          color: Color::from_rgba(1.0, 1.0, 1.0, 0.08),
          width: 1.0,
        },
        shadow: iced::Shadow {
          color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
          offset: iced::Vector::new(0.0, 4.0),
          blur_radius: 16.0,
        },
        ..container::Style::default()
      }),
    ))
    .with_width(120.)
    .with_backdrop(Backdrop::transparent().with_close_on_click(true))
    .with_position(data.position.0.get(store), data.position.1.get(store))
    .with_panel_insets(store)
    .with_on_close(ItemMessage::Effect(id, ItemEffect::Hide))
    .into()
  }
}
