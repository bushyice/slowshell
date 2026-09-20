use slowshell_commons::quicksettings::{QuickSettingsTile, TileAction};
use slowshell_core::message::ItemMessage;

use crate::{Element, Icon};
use iced::{
  Alignment, Length,
  widget::{Space, column, container, row, text},
};

pub fn render_tile<'a>(
  tile: &QuickSettingsTile,
  expanded: bool,
  style: &slowshell_config::style::Style,
  theme: &slowshell_config::style::Theme,
) -> Element<'a, ItemMessage> {
  let radius = style.number("qs.tile.radius").unwrap_or(12.0);
  let font_size = style.number("font.size").unwrap_or(13.0);
  let status_size = style.number("status.font.size").unwrap_or(11.0);
  let icon_size = style.number("icon.size").unwrap_or(18.0) as u16;

  let accent = tile
    .accent
    .as_deref()
    .map(|key| style.color(theme, key, theme.primary))
    .unwrap_or_else(|| style.color(theme, "color.primary", theme.primary));

  let fg = style.color(theme, "color", theme.text);
  let faded = style.color(theme, "color.faded", theme.subtext);

  let icon_color = if tile.active { accent } else { fg };
  let title_color = if tile.active { accent } else { fg };

  let bg = if tile.active {
    style.color(theme, "qs.tile.active", theme.mantle)
  } else {
    style.color(theme, "qs.tile.background", theme.mantle)
  };

  let border_color = if tile.active {
    accent
  } else {
    style.color(theme, "qs.tile.border", theme.overlay)
  };

  let icon: Element<'a, ItemMessage> = Icon::new(tile.icon.clone())
    .size(icon_size)
    .color(icon_color)
    .into();

  let mut labels = column![text(tile.title.clone()).size(font_size).color(title_color)]
    .spacing(1)
    .width(Length::Fill);

  if !tile.status.is_empty() {
    labels = labels.push(text(tile.status.clone()).size(status_size).color(faded));
  }

  let mut content = row![icon, Space::new().width(8), labels,]
    .align_y(Alignment::Center)
    .width(Length::Fill);

  match tile.on_click {
    TileAction::Menu(_) => {
      content = content.push(text('\u{f0142}').size(status_size).color(faded));
    }
    TileAction::Expand => {
      let arrow = if expanded { '\u{f0140}' } else { '\u{f0140}' };
      content = content.push(text(arrow).size(status_size).color(faded));
    }
    _ => {}
  }

  let body = container(content)
    .padding(style.padding([10.0, 12.0]))
    .width(Length::Fill)
    .style(move |_| container::Style {
      background: Some(bg.into()),
      border: iced::Border {
        radius: radius.into(),
        width: 1.0,
        color: border_color,
      },
      ..Default::default()
    });

  body.into()
}
