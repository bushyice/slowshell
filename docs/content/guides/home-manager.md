+++
title = "Home Manager"
weight = 4
description = "Install and configure Slowshell declaratively with Home Manager."
+++

The flake ships a Home Manager module that installs [[Slowshell]], writes its
KDL config, and optionally runs it as a systemd user service. It works with
standalone Home Manager and with Home Manager used as a NixOS module.

## Add the flake

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    slowshell.url = "github:bushyice/slowshell";
  };
}
```

The module is exported as `homeManagerModules.default` (and
`homeManagerModules.slowshell`).

## Enable it

```nix
{ inputs, pkgs, ... }:
{
  imports = [ inputs.slowshell.homeManagerModules.default ];

  programs.slowshell = {
    enable = true;
    package = inputs.slowshell.packages.${pkgs.system}.default;

    systemd.enable = true;

    # Put helper programs the shell calls on the service's PATH.
    systemd.extraPackages = with pkgs; [
      wireplumber
      brightnessctl
      wl-clipboard
      cliphist
    ];

    font = "Lexend";
    tickInterval = 3;
    notifications.enabled = true;

    theme = {
      base = "#1e1e2e";
      text = "#cdd6f4";
      primary = "#eba0ac";
    };

    panels.Main = {
      position = "top";
      height = 34;
      transparent = true;
      sections = {
        left = [ "core/workspaces" "core/window" ];
        center = [ { component = "core/clock"; settings.format = "%I:%M %p"; } ];
        right = [
          {
            label = "System";
            menu = "menus/system";
            items = [
              { component = "core/audio"; settings."show-percent" = true; }
              "core/network"
            ];
          }
        ];
      };
    };
  };
}
```

`systemd.enable` is off by default. Turn it on to have Home Manager start the
shell with `wayland.systemd.target`; leave it off if your compositor starts
`slowshell` itself.

## What the module writes

The module renders the options into a single file,
`$XDG_CONFIG_HOME/slowshell/config.kdl`, and validates it at build time with
`slowshell config --validate`, so a broken config fails `home-manager switch`
instead of the shell.

Slowshell resolves a few things *relative to the config file*: `theme "name"`
looks for `themes/<name>.kdl` and `include-css` looks for the CSS file next to
it. Home Manager links the config into the Nix store, so those paths would point
into the store rather than at your dotfiles. The module therefore inlines the
palette and CSS instead:

```nix
programs.slowshell = {
  theme = { base = "#1e1e2e"; text = "#cdd6f4"; };  # → theme { … } block
  css = ''
    .component { font-size: 13px; }
  '';                                               # → css "…" node
};
```

See [[Configuration]] and [[Themes]] for the full KDL syntax.

## Options

Only a subset of the config is modelled as typed options. Everything else goes
through `extraConfig`, which is appended verbatim to the generated file:

```nix
programs.slowshell.extraConfig = ''
  plugins {
    enabled "core/"
    disabled "core/tray"
  }

  spotlight {
    cliphist #true
  }
'';
```

`widgets` and `panels` accept arbitrary `settings` per component, so component
options do not need to wait for the module to model them:

```nix
programs.slowshell.panels.Main.sections.left = [
  {
    component = "core/icon";
    label = "Slowshell";
    settings = {
      action = "panel.main.toggle";
      icon = "slowshell-icon-shell.svg";
    };
  }
];
```

Setting `package = null` keeps the config but installs nothing, which is handy
when the shell comes from somewhere else (a NixOS module, `nix profile`, or a
development build).

## Without systemd

If you launch the shell from your compositor, drop the `systemd` block:

```nix
programs.slowshell.systemd.enable = false;
```

and start it from the compositor config:

```sh
slowshell daemon
```

`~/.config/slowshell/config.kdl` is written either way.
