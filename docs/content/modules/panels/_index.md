+++
title = "Panels"
description = "The bars where our statuses reside."
+++

Ah yes, panels. The quintessential part of every desktop. 
I wouldn't bore you explaining what panels are; But to describe 
panels in [[Slowshell|slowshell]], I'd say a bar with custom
components that display a some status.

What makes a panel important is what's inside, the statuses. Because,
without those, a panel is just- a bar. Nothing to show, sad and empty inside.

And in slowshell, those statuses are [[Components Overview|components]], which
are the actual parts that hold real service.

## Configuration

```kdl
panels {
  panel "Main" position="top" monitor="*" height=32 {
    components {
      left {
        component "core/workspaces" { label "Workspaces" }
      }
      center {
        component "core/clock" { label "Clock" format "%I:%M %p" }
      }
      right {
        component "core/tray" { label "Tray" }
      }
    }
  }
}
```

### What?

Underlings will explain.

### `panel`

| Attribute | Type | Default | Hot-Reload |
|---|---|---|---|
| `"<name>"` | string | *required* | none | . |
| `position` | `"top"`/`"bottom"`/`"left"`/`"right"` | `"top"` | Yes |
| `monitor` | string | `"*"` | No |
| `height` | int (px) | `32` | Yes |
| `transparent` | bool | `#false` | Yes |
| `overlay` | bool | `#false` | Yes |
| `autohide` | bool | `#false` | Yes |
| `autohide-trigger` | int (px) | `4` | Yes |

**simple example**:
```kdl
panel "Dock" position="bottom" height=48 transparent=#true autohide=#true autohide-trigger=8 {
  // sections go here
  // left/center/right {
  // and here goes one of these:
  // component "<id>"
  // component "<id>" { ... }
  // group "<label>" {
  //  item "<id>" (same as components)
  //  item "<id>" { ... }
  // }
}
```

### Component Options

```kdl
component "core/system" {
  label "Sysmon"      // mandatory, not an option
  interval 5          // option
  progress #true      // option
  items "cpu" "mem"   // option
  "progress-direction" "vertical" // yes, another option.
}
```

### Groups

```kdl
group "Media" {
  // optional: make the whole group open a menu
  //    note: will override the menus for components within
  menu "menus/quick"

  // optional
  prefix "core/icon" { label "Media" icon "preferences-system-symbolic" }

  item "core/audio" { label "Audio" "show-percent" #true }
  item "core/separator" { label "Sep" opacity 0.05 }
  item "core/network" { label "Network" ssid #true }
}
```

## Style classes

```css
.panel {
  background-color: var(--base-a45);
  border-width: 0px;
  border-color: var(--overlay-a15);
  padding-v: 8px;
  padding-h: 8px;
  gap: 4px;
}

.panel.items { gap: 4px; }

.item {
  background-color: var(--mantle-a60);
  border-radius: 4px;
  border-width: 0px;
  border-color: var(--overlay-a15);
  padding-v: 4px;
  padding-h: 8px;
  margin: 4px;
  font-size: 13px;
  color: var(--text);
  gap: 8px;
}

.item.prefix { border-radius: 4px; }
```

## IPC

### Panel commands

If you need to create pannels externally (with no configs):

```sh
slowshell ipc panel.create name=Secondary position=left height=48
```

| Command | Args |
|---|---|
| `panel.create` | `name=` (required), `position=`, `height=`, `monitor=` |

### Per-panel commands

You can also toggle/configure/remove panels by their name:

```sh
slowshell ipc panel.main.toggle
slowshell ipc panel.main.hide
slowshell ipc panel.main.configure height=40 position=bottom enabled=true
```

| Command | Args |
|---|---|
| `panel.<name>.show` | none |
| `panel.<name>.hide` | none |
| `panel.<name>.toggle` | none |
| `panel.<name>.destroy` | none |
| `panel.<name>.configure` | `height=` (default `32`), `position=` (default `top`), `enabled=` (default `#false`), `transparent=`, `monitor=` |
| `panel.<name>.add` | `section=` (default `center`), `label=` (required), `component=`, `options=` |
| `panel.<name>.remove` | `section=` (required), `label=` (required) |

> **Note**: panel names are lowercased and __sluggified__, so for example "The Bar" would be "the-bar".

**Example**:

```sh
slowshell ipc panel.main.add section=right label=CPU component=core/system
slowshell ipc panel.main.remove section=right label=CPU
```

---
