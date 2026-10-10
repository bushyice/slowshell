+++
title = "Getting Started"
weight = 1
description = "Install and run slowshell on your wayland compositor."
+++

[[Slowshell]] is(currently) a self-contained single binary, so the installation process
is pretty much straight-forward.

## What do I need?

Slowshell is a wayland **layer-shell** client. Which means that you will need a wayland
session on a compositor that supports said clients. Currently **niri** and **sway**
are supported out of the box.

**Optional Dependencies**: These are not fatal requirements, but provide inner machinery for
the various components within:

| Feature          | Requires                                                       |
|------------------|----------------------------------------------------------------|
| Audio + MPRIS    | PipeWire and `wpctl`                                           |
| Bluetooth        | BlueZ                                                          |
| Networking       | NetworkManager                                                 |
| Power/brightness | UPower, `power-profiles-daemon`, `brightnessctl`               |
| Wallpapers       | `swaybg`, `swww` or `hyprpaper` (or use the built-in renderer) |
| Clipboard        | `cliphist` (+ `wl-paste`, `wl-copy`)                           |

## Install

{% <tabs id="install"> %}
=== Install script
Feel free to read the script before you run it:

```sh
curl -fsSL https://raw.githubusercontent.com/bushyice/slowshell/main/install.sh | sh
curl -fsSL .../install.sh | sh -s -- --version 0.0.1
curl -fsSL .../install.sh | sh -s -- --path /usr/local/bin
```

=== Nix
```sh
nix run github:bushyice/slowshell
nix profile install github:bushyice/slowshell
```

=== Nix Flake
You can add slowshell as a flake input:

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

=== Build from source
For a detailed guide on building, check [[Building from source]].

```sh
cargo build --release
```
{% </tabs> %}

## Running

```sh
slowshell daemon     # Runs on foreground
slowshell start/stop # Managing on background
```

## I have it, where to start?

After you install it, you can start [[Setup|setting up]] your shell.
