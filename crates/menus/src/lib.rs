pub mod actions;
pub mod bluetooth;
pub mod network;
pub mod notifications;
pub mod power;
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
      "hover.background.opacity" => 0.7
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
  }
);
