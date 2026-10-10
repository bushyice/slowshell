+++
title = "Configuration"
description = "Details for the slowshell config."
weight = 1
+++

[[Slowshell]] looks for a few paths before falling back to the
[default config](https://tangled.org/bushyice.com/slowshell/blob/main/crates/config/src/default.kdl).
Here are the paths where it expects a config file:

{% <tree> %}
~/.config/      # ~/.config/slowshell/config.kdl
  slowshell/
    config.kdl
~/.local/share/ # ~/.local/share/slowshell/config.kdl
  slowshell/
    config.kdl
/usr/share/     # /usr/share/slowshell/config.kdl
  slowshell/
    config.kdl
{% </tree> %}

Once it finds the configuration, it [watches](#watching) the parent (the folder) where `config.kdl`
resides.

> **NOTE**: The config's language is [KDL](https://kdl.dev/), you can read about it [here](https://kdl.dev/).

## What to configure?

Out of the box, you can configure:
- [[Panels]] and [[Components Overview|Components]]
- The [[Spotlight]]
- [[Notifications]]
- [[Wallpaper|Wallpapers]]
- [[Desktop Widgets]]
- [[#Styles|Styles]]
- [[#General Settings|Miscellaneous]]

## Config structure

Here is a basic config structure:

```kdl
font "DejaVu Sans" 
theme "catppuccin-mocha"
tick-interval 3

panels {
  panel "Main" position="top" {
    components {
      right {
        component "core/clock" { label "Clock" format "%I:%M %p" }
      }
    }
  }
}
```

At any time, if you need to validate your config structure, or need to check your current working
config, run this:

```sh
slowshell config --validate   # parse and report the first error
slowshell config --show       # General settings + active stylesheet
slowshell config --current    # which file is in use
```

## Config levels

Slowshell has two distinct levels of configuration, the [General settings](#general-settings) and the [Module settings](#module-settings).

**For example**:
```kdl
// From the previous example, these are general settings:
font "DejaVu Sans" 
theme "catppuccin-mocha"
tick-interval 3

// This is the "panels" module settings:
panels {
  panel "Main" position="top" {
    components {
      right {
        component "core/clock" { label "Clock" format "%I:%M %p" }
      }
    }
  }
}
```

**The difference**:
- General settings are parsed and known by the core itself.
- Module settings are parsed by the module that introduced them.
- General settings always exist.
- Module settings can be compiled-out alongside the module. (See [[Building from source]]).
- General settings determine core and mostly shared functionality. (either by the module's choice or not).
  - e.g: font.
- Module settings **mostly** are self-contained, meaning that one module's settings only affect that module.
  - e.g: panels.

## General settings

| Node | Type | Default | Hot-Reload | Notes |
|--------------------------------|---------------|------------|----|---------------------------------------------------------------------- |
| `font "Name"`                  | string        | `"Lexend"` | No | Font used. `"Lexend"` being embdedded and the default. |
| `tick-interval N`              | int seconds   | `3`        | No | Passive wakeup period for the whole shell |
| `wgpu-backend "vulkan"`        | `vulkan`/`gl` | auto       | No | Sets `WGPU_BACKEND` |
| `icons { theme "Adwaita" }`    | string        | GTK theme  | No | Freedesktop icons used by the shell. Sets `SLOWSHELL_ICON_THEME`. |
| `animations { enabled #true }` | bool          | `true`     | No | Global animation switch (also requires the `animations` cargo feature) |

**Exmaple:**
```kdl
font "DejaVu Sans"
tick-interval 60
wgpu-backend "vulkan"

icons {
  theme "Tux-on-Pixels"
}

animations {
  enabled #true
}
```

## Module settings

Module settings, as the name implies, are settings introduced by self-contained modules. They
are supposed to be parsed by the module, and affect the module itself (or any other module that derives the settings).

**Example**:
```kdl
wallpaper {
  file "bla bla bla"
}
```
In this example, `wallpaper` only affects the wallpaper module. Your panels do not know of this setting, and wouldn't 
care what happens to it.

Another thing about module settings is that they are extensible via [[Plugins]] and some settings can be compiled-out 
alongside the built-in module that introduced it, per your build options within [[Building from source]].

**Cool, what are the built-ins?**
- [[Panels#Configuration|Panels]]
- [[Spotlight#Configuration|Spotlight]]
- [[Notifications#Configuration|Notifications]]
- [[Wallpaper#Configuration|Wallpaper]]
- [[Desktop Widgets#Configuration|Desktop Widgets]]

## Styles

The style in slowshell has two parts, the [[Theme and palette]] and [[CSS]].
Both are applied with the `theme` and `css`(and `include-css`) options as shown below.


> **Note**: Both the `theme` and `css`(and `include-css`) options are hot-reloadable,
  and WILL prompt a refresh to every element in the shell.

###  Applying a theme

You can apply a theme in one of two ways:

- **inline**: inside of your `config.kdl`, you can put the `theme` with all 13 keys (required) anywhere. 
    ```kdl
    theme {
      base "#1e1e2e"
      crust "#11111b"
      mantle "#181825"
    
      primary "#eba0ac"
      secondary "#cba6f7"
    
      green "#94e2d5"
      red "#f38ba8"
      blue "#89dceb"
      yellow "#f9e2af"
      orange "#fab387"
    
      text "#cdd6f4"
      subtext "#bac2de"
      overlay "#9399b2"
    }
    ```
- **from a separate file**: in the parent of your `config.kdl`, you can refer to files under the `themes` folder.    
    ```kdl
    // config.kdl
    theme "catppuccin-mocha" // no .kdl, just the file stem
    ```

    ```kdl
    // themes/catppuccin-mocha.kdl
    base "#1e1e2e"
    crust "#11111b"
    mantle "#181825"
  
    primary "#eba0ac"
    secondary "#cba6f7"
  
    green "#94e2d5"
    red "#f38ba8"
    blue "#89dceb"
    yellow "#f9e2af"
    orange "#fab387"
  
    text "#cdd6f4"
    subtext "#bac2de"
    overlay "#9399b2"
    ```

<div style="padding: 0px 0px 0px 25px;">
{% <tree> %}
~/.config/
  slowshell/
    themes/
      catppuccin-mocha.kdl
    config.kdl
    {% </tree> %}
</div>

---

###  Applying CSS

Just like themes, CSS also can be applied inline or via including a css file:

- **inline**:
    ```kdl
    // within config.kdl
    css """
    .component { font-size: 13px; }
    .panel { background-color: var(--mantle-a60); }
    """
    ```
- **from a separate stylesheet file**: 
    ```kdl
    // config.kdl
    include-css "style.css"
    // you can include more than one .css file
    include-css "extra.css"
    ```

    ```css
    /* style.css */
    .component { font-size: 13px; }
    .panel { background-color: var(--mantle-a60); }
    ```

<div style="padding: 0px 0px 0px 25px;">
{% <tree> %}
~/.config/
  slowshell/
    config.kdl
    style.css
    {% </tree> %}
</div>

---

> **Note**: You can read more about [[Theme and palette]] and [[CSS]] in their respective pages.

## Watching

When slowshell finds a `config.kdl` from the lookup paths, it will watch the parent folder.
Let's take `~/.config/slowshell/config.kdl` as an example. Whenever there's changes to the `config.kdl`
slowshell issues a hot-reload to all modules giving them the option to change their state live without a restart.

Hot reloadable settings within [[#General settings|general settings]] and [[#Styles|style settings]] have already 
been listed above. As for [[#Module settings|module settings]], those are managed internally by modules. Check the
module's configuration details to check if it supports hot-reloading.
