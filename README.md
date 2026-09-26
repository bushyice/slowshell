<div align="center">
  <img src="https://tangled.org/bushyice.com/slowshell/raw/main/assets/icon.png" width="100" alt="slowshell Logo" />
  <h1><code>slowshell</code></h1>
  <p>
    <strong>A wayland desktop shell. Tweakable, pluggable, lightweight, etc etc.</strong>
  </p>
</div>

![screenshot](https://tangled.org/bushyice.com/slowshell/raw/main/assets/screenshot.png)

> [!CAUTION]
> **Status:** Slowshell is in its early stages and moving pretty quickly. Expect rough
edges and breaking changes to the config schema and the plugin ABI between versions.

## What's in the box?

Simply:
- Lightwight
- Panels
- Application launcher
- Notifications
- Wallpapers
- Plugins

## Why?

Well, as someone who likes customizing their shell, I have tinkered around a 
few wayland desktop shells before. Simply put, I wanted a lightweight, tweakable/pluggable
wayland shell that had my most liked features.

So, I made slowshell. Simply: It's supposed to have decent defaults, but also supposed
to be able to be tweaked significantly to one's liking.

## Core principle

Slowshell is majorly update-based. It is made in-mind so that things only refresh when they 
absolutely need to. Giving us an event-driven system where even communication between components
requires "waking up" the sleeping process in order to make a change.

## Modules

The built-in stuff.

### Desktop Items

Everything in slowshell is a `DesktopItem`, and desktop items are required to provide their
cause for updates otherwise they will be stagnant and sleeping as per the update system. 

Simply: They tell slowshell when to update them, they draw when necessary, they mutate/change
when needed, they draw when needed, otherwise they sleep.

### Panels

Ah, yes. The panels. Almost every platform has these, we use them as the central location
of which we keep track of live stats: time, battery, network, etc. Following that, I decided
to make bars a component-based arena for these stats.

```kdl
panels {
  panel "Main" position="top" transparent=#true monitor="HDMI-A-1" {}
}
```

### Panel Components

If you have used something like waybar before, you already know what these mean. In slowshell, 
they are like modular [Desktop Items](#Desktop-Items), but specific to being socketed to a bar
to turn it into a status bar from just a- bar.

```kdl
panel "Main" {
  right {
    // A component
    component "core/workspaces" {
      label "Workspaces"
      text #true
    }

    // A group
    group "SomeGroup" {
      item "core/tray" { label "Tray" }
    }
  }
}
```

> Note: run `slowshell list components` to see all components


### Notifications

A notification daemon. You can enable it with:

```kdl
notifications {
  enabled #true
}
```

### Wallpapers

I won't bore you with text- just look at this:

```kdl
wallpaper {
  backend "swaybg" // "slowshell" | "swaybg" | "swww" | "hyprpaper" | "custom"
  file "/path/to/something"
  // paths {
  //   "HDMI-A-1" "/path/to/something"
  //   "eDP-1" "/path/to/something-else"
  // }
  // spawn-args "command $PATH" // for backend = "custom"
  // max-width 1920
  // max-height 1080
}
```
If this isn't in config, then no wallpapers will be set.

### Spotlight

A launcher thingie. As of today, fully-keyboard based, no other way to control it.
Has "modes" where for example "applications" mode is an application launcher, 
"clipboard" mode is a clipboard list (using cliphist).

```kdl
spotlight {
  cache #true
}
```

To launch, you need to send an IPC command:
```bash
slowshell ipc spotlight.toggle # or do style=grid
# or
slowshell ipc spotlight.toggle clipboard
```

Note: Do `slowshell list spotlights` for all the modes.

## Getting started

### Optional Requirements
- Audio: PipeWire / `wpctl`
- Bluetooth: BlueZ
- Networking: NetworkManager
- Power: UPower + power-profiles-daemon
- Brightness: `brightnessctl`
- Wallpapers: `swaybg`, `swww`, or `hyprpaper`
- Clipboard: `cliphist`

### Installing

**Nix**

Since slowshell has a flake, you could either run it or install it from github:

```sh
nix run github:bushyice/slowshell
nix profile install github:bushyice/slowshell
```

Or as an input in your flake:

```nix
{
  inputs.slowshell.url = "github:bushyice/slowshell";

  outputs = { self, nixpkgs, slowshell, ... }: {
    nixosConfigurations.machine = nixpkgs.lib.nixosSystem {
      modules = [{
        environment.systemPackages = [
          slowshell.packages.${pkgs.system}.default
        ];
      }];
    };
  };
}
```

**Other distros**

```sh
curl -fsSL https://raw.githubusercontent.com/bushyice/slowshell/main/install.sh | sh
# or
curl -fsSL .../install.sh | sh -s -- --version 0.0.1
# or
curl -fsSL .../install.sh | sh -s -- --path /usr/local/bin
```

### Building

### Nix

```sh
nix develop        # dev shell
nix build .#       # build the bin
nix run .#         # run it
```

### Cargo

Requires wayland platform libs, xkbcommon, GL and vulkan.

```sh
cargo build --release
cargo run -- daemon
```

You can also compile with only the features you prefer:

```sh
# only sway, panels without the service components
cargo build --release --no-default-features --features compositor-wlr,panels

# niri only, panels with just audio and the tray
cargo build --release --no-default-features \
  --features compositor-niri,panels,components-audio,components-tray
```

> Note: For a list of all components, check the cargo.toml

## Usage

```text
slowshell daemon                 run in the foreground
slowshell start                  start the daemon in the background
slowshell stop                   stop the running daemon

slowshell ipc <Command> [args]   send a command to the daemon
slowshell list <resource>        styles | plugins | renderables | components | items | spotlights
slowshell config [--validate] [--show] [--current]
```

**Examples**:

```sh
slowshell ipc panel.main.toggle
slowshell ipc panel.create name=Secondary position=left height=48
slowshell ipc panel.secondary.add section=center label=CPU component=core/cpu
slowshell ipc popup.open content=wifi x=panel,Main y=cursor,10.0
slowshell ipc spotlight.toggle
```

The socket is at `$XDG_RUNTIME_DIR/slowshell.sock` (or
`/tmp/slowshell.sock`), so you can use `socat` too:

```sh
echo -n "exec spotlight.toggle" | socat - UNIX-CONNECT:/tmp/slowshell.sock
```

## Config

Configuration is looked up in `$HOME/.config/slowshell/config.kdl` (falling back to
`$HOME/.local/share/slowshell/config.kdl` and `/usr/share/slowshell/config.kdl`)
and hot-reloads on change. Look at [`example.kdl`](https://tangled.org/bushyice.com/slowshell/blob/main/example.kdl) for a full example.

```kdl
font "Lexend"
theme "catppuccin-mocha"

panels {
  panel "Main" position="top" transparent=#true {
    components {
      left  { component "core/workspaces" { label "Workspaces" text #true } }
      center { component "core/clock" { label "Clock" format "%I:%M %p" } }
      right { component "core/network" { label "Network" icon #true ssid #true } }
    }
  }
}
```
