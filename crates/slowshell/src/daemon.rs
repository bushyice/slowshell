use std::sync::{Mutex, OnceLock};

use futures_channel::mpsc::unbounded;
use iced_layershell::{
  reexport::{Anchor, KeyboardInteractivity, Layer},
  settings::{LayerShellSettings, StartMode},
};
use slowshell_config::{Config, ConfigParser};
use slowshell_registry::GlobalRegistry;

static REGISTRY: OnceLock<Mutex<GlobalRegistry>> = OnceLock::new();
static CONFIG: OnceLock<Mutex<Option<Config>>> = OnceLock::new();

use crate::app;

pub fn pid_path() -> Option<std::path::PathBuf> {
  std::env::var("XDG_RUNTIME_DIR")
    .map(std::path::PathBuf::from)
    .map(|p| p.join("slowshell.pid"))
    .ok()
}

pub fn daemon() -> miette::Result<()> {
  use miette::{Context, IntoDiagnostic};

  let mut reg = GlobalRegistry::default();

  slowshell_registry::include(&mut reg);
  slowshell_components::include(&mut reg);
  slowshell_components::register_vertical_style();
  slowshell_menus::include(&mut reg);
  slowshell_notifications::include(&mut reg);
  slowshell_panels::include(&mut reg);
  slowshell_popups::include(&mut reg);
  slowshell_spotlight::include(&mut reg);
  slowshell_background::wallpaper::include(&mut reg);
  slowshell_background::widgets::include(&mut reg);

  let config_parsers = reg
    .inside("config")
    .into_iter()
    .filter_map(|res| {
      res
        .as_unknown()
        .and_then(|x| x.downcast::<ConfigParser>().map(|x| *x).ok())
    })
    .collect();

  let config = Config::from_path_or_default(
    // if cfg!(debug_assertions) {
    //   Some("example.kdl")
    // } else {
    //   None
    // },
    None::<&str>,
    config_parsers,
  );

  if let Some(backend) = &config.wgpu_backend {
    unsafe {
      std::env::set_var("WGPU_BACKEND", backend);
    }
  }

  if let Some(theme) = &config.icon_theme {
    unsafe {
      std::env::set_var("SLOWSHELL_ICON_THEME", theme);
    }
  }

  let font = config.font();

  let _ = REGISTRY.set(Mutex::new(reg));
  let _ = CONFIG.set(Mutex::new(Some(config)));

  iced_layershell::daemon(
    move || {
      let (tx, rx) = unbounded();

      app::init_epoll_rx(rx);

      let mut reg = REGISTRY.get().unwrap().lock().unwrap();
      let config = CONFIG.get().unwrap().lock().unwrap().take().unwrap();

      app::App::new(&mut reg, config, tx)
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
  .font(slowshell_config::FONT)
  .default_font(font)
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
  .into_diagnostic()
  .wrap_err("Failed to run slowshell iced layer-shell daemon")?;

  Ok(())
}
