# Evaluates the Home Manager module with a minimal stand-in for the parts of
# Home Manager it touches, so `nix flake check` can build and validate the
# generated config without pulling home-manager in as a flake input.
#
# The stub only provides the options the module reads; everything else about
# Home Manager (linking files, the systemd unit rendering) is out of scope.
{
  lib,
  pkgs,
  slowshell,
}:

let
  stub =
    { lib, ... }:
    {
      options = {
        home.homeDirectory = lib.mkOption {
          type = lib.types.str;
          default = "/home/tester";
        };

        xdg.configHome = lib.mkOption {
          type = lib.types.str;
          default = "/home/tester/.config";
        };

        wayland.systemd.target = lib.mkOption {
          type = lib.types.str;
          default = "graphical-session.target";
        };

        home.packages = lib.mkOption {
          type = lib.types.listOf lib.types.package;
          default = [ ];
        };

        xdg.configFile = lib.mkOption {
          type = lib.types.attrsOf (
            lib.types.submodule {
              options.source = lib.mkOption { type = lib.types.path; };
            }
          );
          default = { };
        };

        systemd.user.services = lib.mkOption {
          type = lib.types.attrsOf lib.types.attrs;
          default = { };
        };

        assertions = lib.mkOption {
          type = lib.types.listOf lib.types.anything;
          default = [ ];
        };

        warnings = lib.mkOption {
          type = lib.types.listOf lib.types.str;
          default = [ ];
        };
      };
    };

  # A configuration that exercises every block the module knows how to emit.
  example = {
    programs.slowshell = {
      enable = true;
      package = slowshell;
      checkConfig = true;

      font = "Lexend";
      tickInterval = 3;
      wgpuBackend = "vulkan";
      iconTheme = "Adwaita";
      animations.enabled = true;
      notifications.enabled = false;
      spotlight = {
        cache = true;
        cliphist = false;
      };

      theme = {
        base = "#1e1e2e";
        text = "#cdd6f4";
        primary = "#eba0ac";
      };

      css = ''
        .component { font-size: 13px; }
        .panel .item { margin: 0 2px; }
      '';

      wallpaper = {
        enable = true;
        backend = "slowshell";
        file = "/home/tester/wallpaper.png";
        paths."eDP-1" = "/home/tester/laptop.png";
        maxWidth = 3840;
        maxHeight = 2160;
      };

      widgets.Monitor = {
        inner = "widgets/systemmon";
        width = 240;
        height = 180;
        x = 10;
        y = -10;
        anchor = "top-right";
        layer = "overlay";
        monitor = "*";
        root = false;
        settings = {
          format = "%H:%M";
          opacity = 0.85;
          "show-seconds" = false;
        };
      };

      panels.Main = {
        position = "top";
        monitor = "*";
        height = 34;
        transparent = true;
        overlay = false;
        autohide = false;
        autohideTrigger = 4;
        sections = {
          left = [
            "core/workspaces"
            {
              component = "core/window";
              label = "Window";
            }
          ];
          center = [
            {
              component = "core/clock";
              settings.format = "%I:%M %p";
            }
          ];
          right = [
            {
              label = "System";
              menu = "menus/system";
              prefix = {
                component = "core/icon";
                settings.icon = "audio-volume-high";
              };
              items = [
                {
                  component = "core/audio";
                  label = "Audio";
                  settings."show-percent" = true;
                }
                { component = "core/network"; }
              ];
            }
          ];
        };
      };

      systemd.enable = true;
      systemd.extraPackages = [ pkgs.coreutils ];

      extraConfig = ''
        plugins {
          enabled "core/"
        }
      '';
    };
  };

  evaluated = lib.evalModules {
    modules = [
      stub
      ./hm-module.nix
      example
      { _module.args.pkgs = pkgs; }
    ];
  };

  failedAssertions = builtins.filter (entry: !entry.assertion) evaluated.config.assertions;
in
assert lib.assertMsg (failedAssertions == [ ])
  "programs.slowshell: ${lib.concatMapStringsSep "; " (entry: entry.message) failedAssertions}";
evaluated.config.xdg.configFile."slowshell/config.kdl".source
