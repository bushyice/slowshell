+++
title = "Setup"
weight = 2
description = "Configure your slowshell bar, widgets, and wallpapers."
+++

[[Slowshell]] is an empty-slate until configured. The
[default config](https://tangled.org/bushyice.com/slowshell/blob/main/crates/config/src/default.kdl)
has only one bar without a wallpaper.

> **NOTE**: The config's language is [KDL](https://kdl.dev/), you can read about it [here](https://kdl.dev/).

To start, create your config file at `~/.config/slowshell`:

{% <tree> %}
~/.config/
  slowshell/
    config.kdl
{% </tree> %}

Here's a very basic config:
```kdl
// If you don't want your wallpaper managed by slowshell,
// you can remove this block 
wallpaper {
  file "/path/to/file" // Path to your wallpaper. e.g: ~/Pictures/Wallpapers/firewatch.jpg
}

panels {
  // Basic panel for all monitors
  panel "Main" position="top" transparent=#true {
    components {
      left {
        component "core/workspaces" {
          label "Workspaces"
        }
        component "core/window" {
          label "Window"
        }
      }

      center {
        component "core/nowplaying" {
          label "Now Playing"
          "show-artist" #true
          "controls" #true
          "max-length" 40
        }
        component "core/clock" {
          label "Clock"
          format "%I:%M %p · %b %-d"
        }
      }

      right {
        component "core/tray" {
          label "Tray"
        }
        group "Media" {
          item "core/audio" {
            label "Audio"
            "show-percent" #true
          }
          item "core/separator" {
            label "Sep"
            opacity 0.05
          }
          item "core/network" {
            label "Network"
            icon #true
            ssid #true
          }
          item "core/separator" {
            label "Sep"
            opacity 0.05
          }
          item "core/notifications" {
            label "Notifs"
          }
        }
        component "core/system" {
          label "Sysmon"
          interval 5
          icons #true
          progress #true
          labels #true
          progress-direction "vertical"
          items "cpu" "mem" // load, net, temp
        }
        component "core/icon" {
          label "Power"
          menu "menus/actions"
        }
      }
    }
  }
}
```

## Integrating actions with your Compositor

You can integrate slowshell with your compositor via [[IPC]].
For example, with niri:
```kdl
// niri/config.kdl

Mod+A hotkey-overlay-title="Slowshell Spotlight" {
  spawn "slowshell" "ipc" "spotlight.toggle"
}

Mod+V hotkey-overlay-title="Slowshell Clipboard Spotlight" {
  spawn "slowshell" "ipc" "spotlight.toggle clipboard"
}
```

---

Once ready, you can see [[Configuration]] (and how [[Configuration#Watching|watching works]]) for more details, check [[Configuration#Config structure]], and check
[[Theme and palette]] and [[CSS]] for styling your shell.
