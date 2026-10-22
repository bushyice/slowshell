pub mod actions;
pub mod bluetooth;
pub mod clock;
pub mod network;
pub mod notifications;
pub mod power;
pub mod quicksettings;
pub mod shade;
pub mod sound;
pub mod systemmon;
pub mod tray;
pub mod window;

slowshell_registry::register_resources!(
  menu: Style(include_str!("./styles/menus.icss").into()),
  renderables: Renderable {
    name: "menus/system".into(),
    create: |_, _| Some(Box::new(systemmon::SystemMonRenderable))
  },
  renderables: Renderable {
    name: "widgets/systemmon".into(),
    create: |_, _| Some(Box::new(slowshell_widgets::SystemMonWidget))
  },
  renderables: Renderable {
    name: "menus/actions".into(),
    create: |_, _| Some(Box::new(actions::ActionsRenderable::default()))
  },
  renderables: Renderable {
    name: "menus/clock".into(),
    create: |_, _| Some(Box::new(clock::ClockRenderable::default()))
  },
  renderables: Renderable {
    name: "menus/quick".into(),
    create: |_, _| Some(Box::new(quicksettings::QuickSettingsRenderable::default()))
  },
  renderables: Renderable {
    name: "menus/power".into(),
    create: |_, _| Some(Box::new(power::PowerBrightnessRenderable))
  },
  renderables: Renderable {
    name: "menus/bluetooth".into(),
    create: |_, _| Some(Box::new(bluetooth::BluetoothRenderable))
  },
  renderables: Renderable {
    name: "menus/notifications".into(),
    create: |_, _| Some(Box::new(notifications::NotificationsRenderable))
  },
  renderables: Renderable {
    name: "menus/sound".into(),
    create: |_, _| Some(Box::new(sound::SoundRenderable))
  },
  renderables: Renderable {
    name: "menus/network".into(),
    create: |_, _| Some(Box::new(network::NetworkRenderable))
  },
  renderables: Renderable {
    name: "menus/tray".into(),
    create: |_, _| Some(Box::new(tray::TrayRenderable))
  },
  renderables: Renderable {
    name: "menus/window".into(),
    create: |_, _| Some(Box::new(window::WindowRenderable::default()))
  },
  renderables: Renderable {
    name: "menus/shade".into(),
    create: |_, _| Some(Box::new(shade::ShadeRenderable::default()))
  },
  app: Custom(|store| {
    quicksettings::register_builtin_providers(store);
  }),
);
