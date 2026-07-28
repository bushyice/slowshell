mod app;
mod eloop;

use futures_channel::mpsc::unbounded;
use slowshell_config::Config;

use iced_layershell::{
  reexport::{Anchor, KeyboardInteractivity, Layer},
  settings::{LayerShellSettings, StartMode},
};

fn main() {
  iced_layershell::daemon(
    move || {
      let (tx, rx) = unbounded();

      app::init_epoll_rx(rx);

      app::App::new(Config, tx)
    },
    "slowshell",
    app::App::update,
    app::view,
  )
  .subscription(app::App::subscription)
  .style(|_, _| iced::theme::Style {
    background_color: iced::Color::from_rgba(0.0, 0.0, 0.0, 0.0),
    text_color: iced::Color::WHITE,
  })
  .layer_settings(LayerShellSettings {
    anchor: Anchor::Top | Anchor::Left,
    layer: Layer::Overlay,
    exclusive_zone: -1,
    size: Some((1, 1)),
    margin: (0, 0, 0, 0),
    keyboard_interactivity: KeyboardInteractivity::None,
    events_transparent: true,
    start_mode: StartMode::Active,
  })
  .run()
  .expect("failed to run iced");
}
