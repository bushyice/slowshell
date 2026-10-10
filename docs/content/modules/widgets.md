+++
title = "Desktop Widgets"
description = "Like on KDE."
+++

If you're into putting stuff on your background, well- good for you!. 


## Configuration

```kdl
widgets {
  widget "MyClock" {
    width 300
    height 200
    x 50
    y 100
    anchor "top-right"
    layer "bottom"
    monitor "HDMI-A-1"
    inner "widgets/clock"
    root #false

    // anything else is passed will be considered specific widget options
    format "%H:%M"
    size 64
  }
}
```

| Node | Type | Default | Hot-Reload | Notes |
|---|---|---|---|---|
| `"<name>"` | string | *required* | No | .. |
| `inner` | string | none | No | The content. Check [[#The content]]. |
| `width` | int (px) | `200` | No | |
| `height` | int (px) | `200` | No | |
| `x` | int (px) | `0` | No | Offset |
| `y` | int (px) | `0` | No | Offset |
| `anchor` | string | `"top-left"` | No | Where the box hangs. |
| `layer` | string | `"bottom"` | No | `"bottom"`, `"top"`, or `"overlay"`. |
| `monitor` | string | first monitor | No | Output name, or `"*"`. |
| `root` | bool | `#false` | No | wrapped or not. |

**Where**: `anchor` can be: `"top-left"`, `"top-center"`, `"top-right"`,  `"top"`,
`"center-left"`, `"center-center"`, `"center-right"`, `"left"`, `"center"`, `"right"`,
`"bottom-left"`, `"bottom-center"`, `"bottom-right"`, `"bottom"`

### The content

A [[Architecture#Renderables|renderable widget]] to be rendered within the constraints
of your widget. All widgets can be found in this list:

```sh
slowshell list renderables
```

Built-ins are:
- **`widgets/icon`**: `name` (.desktop name), `icon`, `label`, `icons`,
`size` (default `48`), `label-size` (default `12`), `color`, `show-label` (default `#true`).

- **`widgets/clock`**: `format` (default `"%H:%M"`), `format-2` (no default), `interval` (default `60`,
or `1` when the format has `%S`; clamped `1-3600`), `size` (default `28`),
`analog` (default `#false`), `label-size` (default `11`), `color`, `color-2`, `label`.

- **`widgets/nowplaying`**: `cover` (default `#true`), `cover-size` (default `72`, clamped
`24-256`), `show-artist` (default `#true`), `controls` (default `#true`),
`max-length` (default `32`, clamped `4-200`).

- **`widgets/sysmon`**: `interval` (default `5`, clamped `1-3600`), `processes`
(default `#false`), `max-processes` (default `8`, clamped `1-64`), `temp`
(default `#true`).

```kdl
widgets {
  widget "Date" {
    width 320
    height 160
    anchor "top-center"
    y 40
    inner "widgets/clock"
    format "%A, %B %-d"
    size 44
    color "#eba0ac"
  }

  widget "Stats" {
    width 360
    height 220
    anchor "bottom-right"
    x 24
    y 24
    inner "widgets/sysmon"
    interval 5
    processes #true
    max-processes 6
  }
}
```

## Style classes

```css
.clock.analog.face {
  background-color: var(--mantle);
  border-width: 3px;
  border-color: var(--overlay);
}

.clock-hand.hour   { width: 6px; height: 26px; border-radius: 3px; }
.clock-hand.minute { width: 4px; height: 40px; border-radius: 2px; }
```

## IPC

```sh
slowshell ipc widget.<name>.toggle
slowshell ipc widget.<name>.hide
slowshell ipc widget.<name>.show
slowshell ipc widget.<name>.destroy
```

---
