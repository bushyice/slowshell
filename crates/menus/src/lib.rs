pub mod actions;
pub mod bluetooth;
pub mod clock;
pub mod network;
pub mod notifications;
pub mod power;
pub mod quicksettings;
pub mod sound;
pub mod systemmon;
pub mod tray;

slowshell_registry::register_resources!(
  menu: Style(
    slowshell_config::style! {
      "background" => "crust",
      "background.opacity" => 0.95,
      "radius" => 12,
      "border.width" => 1,
      "border.color" => "overlay",
      "border.color.opacity" => 0.15,
      "shadow.color" => "base",
      "shadow.opacity" => 0.5,
      "shadow.blur" => 28,
      "shadow.offset.x" => 0,
      "shadow.offset.y" => 4,
      "padding" => 16,
      "spacing" => 12,
      "inner.padding" => 12,
      "list.spacing" => 8,
      "list.width" => 220,
      "list.large.width" => 320.0,
      "list.indent" => true,
      "color" => "text",
      "color.faded" => "subtext",
      "color.primary" => "primary",
      "color.secondary" => "yellow",
      "color.danger" => "red",
      "arrow.color" => "overlay",
      "connected.color" => "green",
      "separator.color" => "base",
      "separator.color.opacity" => 1.0,
      "row.background" => "mantle",
      "row.background.opacity" => 0.5,
      "row.radius" => 6,
      "row.border.width" => 0,
      "row.border.color" => "overlay",
      "row.border.color.opacity" => 0.15,
      "row.padding.x" => 12,
      "row.padding.y" => 7,
      "row.spacing" => 8,
      "font.size" => 13,
      "header.font.size" => 15,
      "name.font.size" => 13,
      "status.font.size" => 11,
      "icon.size" => 16,
      "checkbox.size" => 14,
      "arrow.size" => 12,
      "button.radius" => 8,
      "button.font.size" => 12,
      "hover.background" => "mantle",
      "hover.background.opacity" => 0.7,
      "qs.width" => 340,
      "qs.avatar.size" => 42,
      "qs.tile.radius" => 12,
      "qs.tile.background" => "mantle",
      "qs.tile.background.opacity" => 1.0,
      "qs.tile.active" => "mantle",
      "qs.tile.border" => "overlay",
      "qs.tile.border.opacity" => 0.15,
      "clock.time.size" => 38,
      "clock.seconds.size" => 15,
      "calendar.cell.height" => 28,
      "calendar.spacing" => 2,
      "calendar.today.radius" => 999
    }
  ),
  slider: Style(
    slowshell_config::style! {
      "background" => "primary",
      "background.rail" => "mantle",
      "background.rail.filled" => "primary",
      "border.color" => "transparent",
      "border.width" => 0.0,
      "rail.width" => 4.0,
      "rail.border.width" => 0.0,
      "rail.border.color" => 0.0,
      "rail.border.radius" => 2.0,
      "rect" => false,
      "slider.size" => 7.0
    }
  ),
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
  app: Custom(|store| {
    quicksettings::register_builtin_providers(store);
  }),
);
