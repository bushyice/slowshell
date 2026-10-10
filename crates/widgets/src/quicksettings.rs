use slowshell_commons::quicksettings::{QuickSettingsTile, TileAction};
use slowshell_config::Config;
use slowshell_core::message::ItemMessage;

use crate::{Element, Icon};
use iced::{
  Alignment, Length,
  widget::{Space, column, container, row},
};

pub fn render_tile<'a>(
  tile: &QuickSettingsTile,
  expanded: bool,
  config: &'a Config,
) -> Element<'a, ItemMessage> {
  let palette = config.palette();
  let style = config.computed(&["menu", "qs", "tile"]);
  let icon_size = style.length("icon-size").unwrap_or(18.0) as u16;

  let accent = tile
    .accent
    .as_deref()
    .and_then(|key| palette.color(key))
    .or_else(|| config.color(&["menu", "text-primary"], "color"))
    .unwrap_or(palette.primary);

  let fg = config.color(&["menu"], "color").unwrap_or(palette.text);
  let faded = config
    .color(&["menu", "text-faded"], "color")
    .unwrap_or(palette.subtext);

  let icon_color = if tile.active { accent } else { fg };
  let title_color = if tile.active { accent } else { fg };

  let tile_classes: &'static [&'static str] = if tile.active {
    &["menu", "qs", "tile", "active"]
  } else {
    &["menu", "qs", "tile"]
  };

  let icon: Element<'a, ItemMessage> = Icon::new(tile.icon.clone())
    .size(icon_size)
    .color(icon_color)
    .into();

  let mut labels = column![
    config
      .text(tile.title.clone(), tile_classes)
      .color(title_color)
  ]
  .spacing(1)
  .width(Length::Fill);

  if !tile.status.is_empty() {
    labels = labels.push(
      config
        .text(tile.status.clone(), &["menu", "qs", "tile", "status"])
        .color(faded),
    );
  }

  let mut content = row![icon, Space::new().width(8), labels,]
    .align_y(Alignment::Center)
    .width(Length::Fill);

  match tile.on_click {
    TileAction::Menu(_) => {
      content = content.push(
        config
          .text('\u{f0142}', &["menu", "qs", "tile", "status"])
          .color(faded),
      );
    }
    TileAction::Expand => {
      let arrow = if expanded { '\u{f0140}' } else { '\u{f0140}' };
      content = content.push(
        config
          .text(arrow, &["menu", "qs", "tile", "status"])
          .color(faded),
      );
    }
    _ => {}
  }

  crate::hover_row(
    config,
    &["menu", "qs", "tile"],
    container(content)
      .width(Length::Fill)
      .padding(config.sizing(tile_classes).padding()),
  )
}
