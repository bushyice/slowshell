+++
title = "Components Overview"
description = "Built-in panel components."
+++

Components are the parts of the [[Panels|panels]] that make the panel live. They
show status, serve as the gates for menus, etc etc.

## A component

```kdl
component "core/clock" {
  label "Clock"     // reserved: the item's name
  format "%I:%M %p" // everything else is an option
}
```

**Where**:
- `label` is an internal field used by the panels to identify the component.
- Every other attribute serves as an option for the component.

### Style classes

```css
.component {
  color: var(--text);
  color-faded: var(--subtext);
  color-invert: var(--base);
  color-primary: var(--primary);
  font-size: 12px;
  icon-size: 14px;
  gap: 5px;
  padding-v: 5px;
  padding-h: 9px;
  hover-background: var(--primary-a18);
  hover-radius: 4px;
}

.component.vertical { font-size: 15px; icon-size: 18px; gap: 8px; padding-v: 4px; padding-h: 5px; }

.component.faded     { color: var(--subtext); }
.component.invert    { color: var(--base); }
.component.primary   { color: var(--primary); }
.component.secondary { color: var(--yellow); }
.component.danger    { color: var(--red); }
```

---

## The built-in components

### core/icon

A simple clickable icon.

| Option | Type | Default | Notes |
|---|---|---|---|
| `action` | string | none | [[IPC]] action dispatched on click, e.g. `"panel.main.toggle"`. |
| `menu` | string | none | Menu to open, e.g. `"menus/quick"`. |
| `exec`/`exec-alt` | string array | none | System commands. (left/right click) |
| `icon` | string | `"applications-system-symbolic"` | A free-desktop icon name. |
| `icons` | table | none | "one of these" |

```kdl
component "core/icon" {
  label "Quick settings"
  menu "menus/quick"
  // or
  exec "sh" "-c" "some command here"
  icon "preferences-system-symbolic"
}
```

---

### core/clock

Clock.

| Option | Type | Default | Notes |
|---|---|---|---|
| `format` | string | `"%H:%M"` | [chrono](https://docs.rs/chrono/latest/chrono/format/strftime) format specifier. |
| `update-time` | int (s) | `60` | Seconds between repaints. |

```kdl
component "core/clock" {
  label "Clock"
  format "%I:%M %p · %b %-d"
  "update-time" 30
}
```

---

### core/workspaces

Compositor workspaces.

| Option | Type | Default | Notes |
|---|---|---|---|
| `all` | bool | `#false` | Workspaces from every output. |
| `text` | bool | `#false` | Include the name. |
| `"override-text"` | table | none | List of workspace index/name to replacement text. |

```kdl
component "core/workspaces" {
  label "Workspaces"
  text #true
  "override-text" {
    "1" "󰈹"
    "2" ""
  }
}
```

---

### core/window

The focused window.

| Option | Type | Default | Notes |
|---|---|---|---|
| `icon` | bool | `#true` | Show or not the icon. |
| `name` | bool | `#true` | App name. |
| `title` | bool | `#true` | Window title. |
| `"name-width"` | number (px) | `180` | Clipped on overflow. `0` disables the cap. |
| `"title-width"` | number (px) | `320` | Same as `"name-width"`. |

---

### core/windows

A simple dock-able window list.

| Option | Type | Default | Notes |
|---|---|---|---|
| `"icon-size"` | number | the `.windows` `icon-size` (`22px`) | Overrides the style. |
| `pinned` | string array | none | Apps always shown, even when closed. (Takes the desktop app-id) |
| `"show-titles"` | bool | `#false` | App name under each icon. |
| `"title-length"` | int | `8` | Characters before the ellipsis. |

**The clicks**:
- **Left click**: focus the app's window, cycle through its instances, or launch it when
  a pinned app has no open window.
- **Shift + click**: cycle to the previous instance.
- **Right click**: pin / unpin the app (persisted between sessions).

> **Note**: App id provided are the freedesktop app ids, such as "com.mitchellh.ghostty".

---

### core/system

A simple system monitor.

| Option | Type | Default | Notes |
|---|---|---|---|
| `interval` | int (s) | `2` | Seconds between refreshes. |
| `items` | string array | `"cpu" "mem"` | `cpu`, `mem`, `temp`, `net`, `load`. |
| `icons` | bool | `#true` ||
| `labels` | bool | `#true` ||
| `percents` | bool | `#true` ||
| `progress` | bool | `#false` | Progress bar next to the component. |
| `progress-direction` | string | `"horizontal"` | `"horizontal"`/`"vertical"`. |
| `icon_names` | string array | none | Icon overrides. |
| `processes` | bool | `#false` | Processes list (costlier) |

```kdl
component "core/system" {
  label "Sysmon"
  interval 5
  items "cpu" "mem" "temp"
  progress #true
  progress-direction "vertical"
}
```

---

### core/audio

Volume and mute, with optional currently playing track title.

| Option | Type | Default | Notes |
|---|---|---|---|
| `"show-percent"` | bool | `#false` | Print the volume next to the icon. |
| `"show-track"` | bool | `#false` | Print the current track instead. Hard-truncated at 30 chars. |

**The clicks**:
- **Scroll**: volume +/-5%.
- **Middle click**: toggle mute.
- **Left click**: open sounds menu.

---

### core/power

Battery and brightness.

| Option | Type | Default |
|---|---|---|
| `battery` | bool | auto |
| `brightness` | bool | auto |

**The clicks**:
- **scroll**: brightness +/-5%,
- **left click**: opens brightness menu.

---

### core/network

A simple network status.

| Option | Type | Default | Notes |
|---|---|---|---|
| `icon` | bool | `#true` | The connection icon. |
| `ssid` | bool | `#false` | Print the SSID (or interface) next to it. |

---

### core/bluetooth

Bluetooth status.

Yeah this one's bare, no options.

---

### core/notifications

Simple [[Notifications|notifications]] status.

**The clicks**:
- **Left click**: open notifications list.
- **Middle / right click**: toggle do not disturb.

---

### core/tray

The system tray.

| Option | Type | Default |
|---|---|---|
| `spacing` | number | the `.component` `gap` |

---

### core/separator

A thin line (or a dot) between items.

| Option | Type | Default |
|---|---|---|
| `circle` | bool | `#false` |
| `size` | number | `2.0` |
| `opacity` | number | `0.1` |

---

### core/nowplaying

The current playing track as `artist - title`.

| Option | Type | Default |
|---|---|---|
| `"show-artist"` | bool | `#true` | 
| `controls` | bool | `#false` |
| `"max-length"` | number | `40` |

```kdl
component "core/nowplaying" {
  label "Now Playing"
  "show-artist" #true
  controls #true
  "max-length" 40
}
```

---
