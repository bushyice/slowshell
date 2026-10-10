+++
title = "Building from source"
weight = 3
description = "Compile Slowshell with custom cargo workspace feature flags."
+++

[[Slowshell]] has many optional compile-time options as cargo workspace
features. The default feature set offers a fairly complete desktop shell,
but you can build smaller binaries by selecting only the features you need.

## Examples

### Niri

If you're only intending to use slowshell with niri, you could do:

```sh
cargo build --release --no-default-features \
  --features base,compositor-niri,components,animations
```

### Minimal

A minimal shell with panels, a background, and widgets:

```sh
cargo install slowshell --no-default-features \
  --features panels,background,widgets
```

### I will build my own shell

If you only need the core and can extend the rest with [[Plugins]]:

```sh
cargo install slowshell --no-default-features \
  --features compositors,plugins
```

## All features

| Feature                    | Description                                                                                      |
| -------------------------- | -------------------------------------------------------------------------------------------------|
| `base`                     | The standard shell features: panels, background, widgets, plugins, Spotlight, and notifications. |
| `compositors`              | All supported compositor integrations.                                                           |
| `components`               | All built-in components.                                                                         |
| `animations`               | Animations.                                                                                      |
| `panels`                   | Panels, popups, and built-in panel components.                                                   |
| `popups`                   | Popup surfaces.                                                                                  |
| `background`               | Desktop background and wallpaper support.                                                        |
| `widgets`                  | Desktop widgets. (with `background`)                                                             |
| `plugins`                  | Plugin support.                                                                                  |
| `spotlight`                | [[Spotlight]] application launcher.                                                              |
| `notifications`            | Desktop notifications.                                                                           |
| `compositor-niri`          | Niri compositor integration.                                                                     |
| `compositor-sway`          | Sway compositor integration.                                                                     |
| `components-audio`         | Audio controls and information.                                                                  |
| `components-bluetooth`     | Bluetooth controls and information.                                                              |
| `components-network`       | Network controls and information.                                                                |
| `components-notifications` | Notification-related components.                                                                 |
| `components-power`         | Power and battery controls and information.                                                      |
| `components-tray`          | System tray support.                                                                             |
