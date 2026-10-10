+++
title = "Wallpaper"
description = "Wallpapers."
+++

Yeah i'm not explaining what a wallpaper is.

## Configuration

```kdl
wallpaper {
  backend "swaybg"
  file "/home/you/Pictures/bg/cat_leaves.png"

  paths {
    "HDMI-A-1" "/home/you/Pictures/bg/1.png"
    "eDP-1"    "/home/you/Pictures/bg/2.png"
  }

  spawn-args "command $PATH"
  max-width 1920
  max-height 1080
}
```

| Node | Type | Default | Hot-Reload | Notes |
|---|---|---|---|---|
| `backend` | string | `"slowshell"` | Yes | `"slowshell"`/`"swaybg"`/`"swww"`/`"hyprpaper"`/`"custom"`. |
| `file` | string | - | Yes | The image used for all outputs. |
| `paths` | table | - | Yes | Per-output paths. |
| `spawn-args` | string | - | Yes | Only for `backend "custom"`. |
| `max-width` | int | `1920` | Yes | Downscale cap. (`slowshell` only) |
| `max-height` | int | `1080` | Yes | Downscale cap. (`slowshell` only) |

> **Note**: For the `custom` backend, `spawn-args` can have `$PATH`, `$FILE` and `$OUTPUT` or `$MONITOR`.

---
