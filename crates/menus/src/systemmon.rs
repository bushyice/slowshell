use iced::{
  Element, Length,
  widget::{column, container, text},
};
use iced_layershell::reexport::IcedId;
use slowshell_config::Config;
use slowshell_core::{
  Store,
  message::{ItemEffect, ItemMessage},
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{
  Backdrop, Renderable, SizedPopup, SystemMonOptions, render_system_mon_widget,
};

pub struct SystemMonRenderable;

impl Renderable for SystemMonRenderable {
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

    let header = text("System Monitor")
      .size(style.number("header.font.size").unwrap_or(15.0))
      .color(style.color(theme, "color", theme.text));

    let options = SystemMonOptions {
      show_processes: true,
      max_processes: 10,
      show_temp: true,
    };

    let widget = render_system_mon_widget(config, store, options);

    let body = column![header, widget,]
      .spacing(style.number("spacing").unwrap_or(14.0))
      .width(Length::Fixed(
        style.number("list.large.width").unwrap_or(320.0),
      ));

    let inner_padding = style.number("inner.padding").unwrap_or(8.0);
    let mut popup_style = style.container_style(theme);
    if let (Some(x), Some(y)) = (
      style.number("shadow.offset.x"),
      style.number("shadow.offset.y"),
    ) {
      popup_style.shadow.offset = iced::Vector::new(x, y);
    }

    SizedPopup::new(container(
      container(body)
        .padding(inner_padding)
        .style(move |_| popup_style),
    ))
    .with_width(280.)
    .with_backdrop(Backdrop::transparent().with_close_on_click(true))
    .with_position(data.position.0.get(store), data.position.1.get(store))
    .with_panel_edge(data.panel_edge)
    .with_panel_insets(store)
    .with_on_close(ItemMessage::Effect(id, ItemEffect::Hide))
    .into()
  }
}
