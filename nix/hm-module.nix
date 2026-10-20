# Home Manager module for slowshell.
#
# Renders the Nix options into slowshell's KDL config and installs it at
# `$XDG_CONFIG_HOME/slowshell/config.kdl`, the first location slowshell looks
# in. The file is generated as a whole, so a switch never leaves it partially
# written.
#
# Because Home Manager links that file into the Nix store, slowshell's
# `canonicalize()` puts the config's parent directory inside the store. Any
# path slowshell resolves *relative* to the config therefore cannot reach
# `$XDG_CONFIG_HOME`, which is why the palette and CSS are inlined here
# instead of being written as `themes/*.kdl` and `include-css` files.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  inherit (lib)
    concatStringsSep
    filterAttrs
    isAttrs
    isString
    literalExpression
    mapAttrsToList
    mkIf
    mkOption
    mkPackageOption
    optional
    optionalAttrs
    optionalString
    types
    ;

  cfg = config.programs.slowshell;

  kdl = import ./kdl.nix { inherit lib; };

  node = kdl.render;

  leaf =
    name: value:
    node {
      inherit name;
      args = [ value ];
    };

  # A KDL value tree: scalars become node arguments, attribute sets become
  # child nodes, which slowshell's `node_options_map`/`node_to_value` read as
  # tables.
  kdlValueType =
    let
      value = types.oneOf [
        types.bool
        types.int
        types.float
        types.str
        (types.attrsOf value)
      ];
    in
    value;

  valueNode =
    name: value:
    if isAttrs value then
      node {
        inherit name;
        children = mapAttrsToList valueNode value;
      }
    else
      leaf name value;

  # Panel entries are either a bare component name or an attrset. Within an
  # attrset, `items`, `menu` or `prefix` turn the entry into a group; a group
  # is rendered as `group "Label" { … }` and needs a label.
  componentOptions = {
    component = mkOption {
      type = types.str;
      example = "core/clock";
      description = "Component id, as listed by `slowshell list components`.";
    };

    label = mkOption {
      type = types.nullOr types.str;
      default = null;
      description = "Text shown by the component; slowshell falls back to the component id.";
    };

    settings = mkOption {
      type = kdlValueType;
      default = { };
      example = literalExpression ''
        { format = "%H:%M"; }
      '';
      description = "Options passed to the component, one KDL node per attribute.";
    };
  };

  componentType = types.submodule { options = componentOptions; };

  entryType = types.submodule {
    options = {
      component = mkOption {
        type = types.nullOr types.str;
        default = null;
        example = "core/clock";
        description = "Component id; required unless the entry is a group.";
      };

      label = mkOption {
        type = types.nullOr types.str;
        default = null;
        description = "Text shown by the component, or the group's caption.";
      };

      settings = mkOption {
        type = kdlValueType;
        default = { };
        description = "Options passed to the component, one KDL node per attribute.";
      };

      items = mkOption {
        type = types.nullOr (types.listOf (types.either types.str componentType));
        default = null;
        example = literalExpression ''
          [
            "core/clock"
            { component = "core/audio"; label = "Audio"; }
          ]
        '';
        description = ''
          Components inside this group, each either a bare component id or an
          attrset with options. Setting it turns the entry into a group.
        '';
      };

      menu = mkOption {
        type = types.nullOr types.str;
        default = null;
        example = "menus/system";
        description = ''
          Renderable opened when the group's prefix is clicked. Slowshell ships
          menus such as `menus/system`, `menus/power`, `menus/clock`,
          `menus/sound`, `menus/network`, `menus/bluetooth`, `menus/tray` and
          `menus/window`.
        '';
      };

      prefix = mkOption {
        type = types.nullOr componentType;
        default = null;
        description = "Component shown before the group's items, typically an icon.";
      };
    };
  };

  panelEntryType = types.either types.str entryType;

  # --- general settings ---------------------------------------------------

  generalBlocks =
    optional (cfg.font != null) (leaf "font" cfg.font)
    ++ optional (cfg.tickInterval != null) (leaf "tick-interval" cfg.tickInterval)
    ++ optional (cfg.wgpuBackend != null) (leaf "wgpu-backend" cfg.wgpuBackend)
    ++ optional (cfg.iconTheme != null) (node {
      name = "icons";
      children = [ (leaf "theme" cfg.iconTheme) ];
    })
    ++ optional (cfg.animations.enabled != null) (node {
      name = "animations";
      children = [ (leaf "enabled" cfg.animations.enabled) ];
    })
    ++ optional (cfg.notifications.enabled != null) (node {
      name = "notifications";
      children = [ (leaf "enabled" cfg.notifications.enabled) ];
    })
    ++ optional (cfg.spotlight.cache != null || cfg.spotlight.cliphist != null) (node {
      name = "spotlight";
      children =
        optional (cfg.spotlight.cache != null) (leaf "cache" cfg.spotlight.cache)
        ++ optional (cfg.spotlight.cliphist != null) (leaf "cliphist" cfg.spotlight.cliphist);
    });

  # --- palette ------------------------------------------------------------

  paletteNames = [
    "base"
    "crust"
    "mantle"
    "primary"
    "secondary"
    "green"
    "red"
    "blue"
    "yellow"
    "orange"
    "text"
    "subtext"
    "overlay"
  ];

  themeBlock = node {
    name = "theme";
    children = mapAttrsToList (name: value: leaf name value) cfg.theme;
  };

  # --- wallpaper ----------------------------------------------------------

  wallpaperBlock = node {
    name = "wallpaper";
    children =
      optional (cfg.wallpaper.backend != null) (leaf "backend" cfg.wallpaper.backend)
      ++ optional (cfg.wallpaper.file != null) (leaf "file" cfg.wallpaper.file)
      ++ optional (cfg.wallpaper.paths != { }) (node {
        name = "paths";
        children = mapAttrsToList (output: path: leaf output path) cfg.wallpaper.paths;
      })
      ++ optional (cfg.wallpaper.spawnArgs != null) (leaf "spawn-args" cfg.wallpaper.spawnArgs)
      ++ optional (cfg.wallpaper.maxWidth != null) (leaf "max-width" cfg.wallpaper.maxWidth)
      ++ optional (cfg.wallpaper.maxHeight != null) (leaf "max-height" cfg.wallpaper.maxHeight);
  };

  # --- widgets ------------------------------------------------------------

  widgetBlock =
    name: widget:
    node {
      name = "widget";
      args = [ name ];
      children =
        optional (widget.inner != null) (leaf "inner" widget.inner)
        ++ optional (widget.width != null) (leaf "width" widget.width)
        ++ optional (widget.height != null) (leaf "height" widget.height)
        ++ optional (widget.x != null) (leaf "x" widget.x)
        ++ optional (widget.y != null) (leaf "y" widget.y)
        ++ optional (widget.anchor != null) (leaf "anchor" widget.anchor)
        ++ optional (widget.layer != null) (leaf "layer" widget.layer)
        ++ optional (widget.monitor != null) (leaf "monitor" widget.monitor)
        ++ optional widget.root (leaf "root" true)
        ++ mapAttrsToList valueNode widget.settings;
    };

  # --- panels -------------------------------------------------------------

  componentBlock =
    tag: item:
    node {
      name = tag;
      args = [ item.component ];
      children =
        optional (item.label != null) (leaf "label" item.label) ++ mapAttrsToList valueNode item.settings;
    };

  itemBlock = item: if isString item then leaf "item" item else componentBlock "item" item;

  groupBlock =
    group:
    node {
      name = "group";
      args = [ group.label ];
      children =
        optional (group.menu != null) (leaf "menu" group.menu)
        ++ optional (group.prefix != null) (componentBlock "prefix" group.prefix)
        ++ map itemBlock (if group.items == null then [ ] else group.items);
    };

  # `group` requires a label argument and `component` a component id; without
  # them slowshell silently drops the entry, so report the mistake instead.
  fail = where: message: throw "programs.slowshell.panels.${where}: ${message}";

  entryIsGroup =
    entry: isAttrs entry && (entry.items != null || entry.menu != null || entry.prefix != null);

  entryBlock =
    where: entry:
    if isString entry then
      leaf "component" entry
    else if entryIsGroup entry then
      let
        label = if entry.label != null then entry.label else entry.component;
      in
      if label == null then
        fail where "groups need a `label`."
      else
        groupBlock (entry // { inherit label; })
    else if entry.component == null then
      fail where "components need a `component` id."
    else
      componentBlock "component" entry;

  sectionBlock =
    where: section: entries:
    node {
      name = section;
      children = lib.imap0 (
        index: entry: entryBlock "${where}.sections.${section}[${toString index}]" entry
      ) entries;
    };

  panelBlock =
    name: panel:
    node {
      name = "panel";
      args = [ name ];
      # Panel attributes are KDL properties, which is what `prop_str` and
      # friends read.
      props =
        optionalAttrs (panel.position != null) { position = panel.position; }
        // optionalAttrs (panel.monitor != null) { monitor = panel.monitor; }
        // optionalAttrs (panel.height != null) { height = panel.height; }
        // optionalAttrs (panel.transparent != null) { transparent = panel.transparent; }
        // optionalAttrs (panel.overlay != null) { overlay = panel.overlay; }
        // optionalAttrs (panel.autohide != null) { autohide = panel.autohide; }
        // optionalAttrs (panel.autohideTrigger != null) {
          autohide-trigger = panel.autohideTrigger;
        };
      children =
        let
          sections = filterAttrs (_: entries: entries != [ ]) panel.sections;
        in
        optional (sections != { }) (node {
          name = "components";
          children = lib.mapAttrsToList (section: entries: sectionBlock name section entries) sections;
        });
    };

  # --- document -----------------------------------------------------------

  document = concatStringsSep "\n" (
    [
      "// Generated by Home Manager from `programs.slowshell`."
      "// This file is overwritten on the next `home-manager switch`."
      ""
    ]
    ++ generalBlocks
    ++ optional (cfg.theme != { }) themeBlock
    ++ optional (cfg.css != "") (leaf "css" cfg.css)
    ++ optional (cfg.wallpaper.enable) wallpaperBlock
    ++ optional (cfg.widgets != { }) (node {
      name = "widgets";
      children = mapAttrsToList widgetBlock cfg.widgets;
    })
    ++ optional (cfg.panels != { }) (node {
      name = "panels";
      children = mapAttrsToList panelBlock cfg.panels;
    })
    ++ optional (cfg.extraConfig != "") cfg.extraConfig
    ++ [ "" ]
  );

  configFile = pkgs.writeTextFile {
    name = "slowshell-config.kdl";
    text = document;
    checkPhase = optionalString cfg.checkConfig ''
      check_home="$(mktemp -d)"
      mkdir -p "$check_home/.config/slowshell"
      cp "$out" "$check_home/.config/slowshell/config.kdl"

      if ! HOME="$check_home" ${lib.getExe' cfg.package "slowshell"} config --validate \
        > "$check_home/validate.log" 2>&1
      then
        cat "$check_home/validate.log" >&2
        exit 1
      fi

      # `config --validate` reports invalid CSS as a diagnostic but still
      # exits successfully, so treat any diagnostic as a failure.
      if grep -q '^\[config\]' "$check_home/validate.log"; then
        cat "$check_home/validate.log" >&2
        exit 1
      fi
    '';
  };

  activeService = cfg.systemd.enable && cfg.package != null;

  # Slowshell shells out to helpers (wallpaper backends, `wpctl`, `brightnessctl`,
  # `cliphist`, `wl-paste`, …) that may live in packages of their own. Extra
  # packages are prepended to PATH here rather than assigned through
  # `Environment=PATH=`, which would drop the session's own PATH.
  serviceExec =
    if cfg.systemd.extraPackages == [ ] then
      "${lib.getExe cfg.package} daemon"
    else
      pkgs.writeShellScript "slowshell-service" ''
        export PATH=${lib.makeBinPath cfg.systemd.extraPackages}''${PATH:+:$PATH}
        exec ${lib.getExe cfg.package} daemon
      '';
in
{
  options.programs.slowshell = {
    enable = lib.mkEnableOption "slowshell, a wayland desktop shell";

    package = mkPackageOption pkgs "slowshell" {
      nullable = true;
      default = null;
      extraDescription = ''
        `package` has no default because slowshell is not in nixpkgs; set it to
        the package from this flake:

        ```nix
        programs.slowshell.package = inputs.slowshell.packages.''${pkgs.system}.default;
        ```

        Leaving it `null` installs nothing but still writes the config, which is
        useful when slowshell comes from somewhere else.
      '';
    };

    systemd.enable = lib.mkEnableOption "the slowshell systemd user service" // {
      description = ''
        Whether to run slowshell as a systemd user service that starts with
        {option}`programs.slowshell.systemd.target`.

        When this is disabled the module only installs the package and writes
        the config, leaving it to the compositor or the user to start the
        shell.
      '';
    };

    systemd.target = mkOption {
      type = types.str;
      default = config.wayland.systemd.target;
      defaultText = literalExpression "config.wayland.systemd.target";
      example = "sway-session.target";
      description = "Target the slowshell service is bound to.";
    };

    systemd.extraPackages = mkOption {
      type = types.listOf types.package;
      default = [ ];
      example = literalExpression "with pkgs; [ wireplumber brightnessctl wl-clipboard cliphist ]";
      description = ''
        Packages put on the service's {env}`PATH` in addition to the session's.
        Slowshell runs helper programs for wallpaper backends, volume and
        brightness components, and the `cliphist` spotlight mode.
      '';
    };

    font = mkOption {
      type = types.nullOr types.str;
      default = null;
      example = "Lexend";
      description = ''
        UI font family. `Lexend` is embedded in slowshell and used when this is
        `null`.
      '';
    };

    tickInterval = mkOption {
      type = types.nullOr types.ints.positive;
      default = null;
      example = 3;
      description = "Seconds between passive tick events; slowshell defaults to `3`.";
    };

    wgpuBackend = mkOption {
      type = types.nullOr (
        types.enum [
          "vulkan"
          "gl"
        ]
      );
      default = null;
      description = "Force a wgpu backend instead of letting slowshell choose one.";
    };

    iconTheme = mkOption {
      type = types.nullOr types.str;
      default = null;
      example = "Adwaita";
      description = ''
        Freedesktop icon theme. Slowshell looks for it in the icon theme
        directories of {env}`XDG_DATA_HOME`, {env}`XDG_DATA_DIRS` and
        {file}`/usr/share/icons`.
      '';
    };

    animations.enabled = mkOption {
      type = types.nullOr types.bool;
      default = null;
      description = "Global animation switch, for builds that include the `animations` feature.";
    };

    notifications.enabled = mkOption {
      type = types.nullOr types.bool;
      default = null;
      description = ''
        Whether slowshell's notification daemon claims
        `org.freedesktop.Notifications`.
      '';
    };

    spotlight = {
      cache = mkOption {
        type = types.nullOr types.bool;
        default = null;
        description = "Cache empty spotlight results to make repeat launches faster.";
      };

      cliphist = mkOption {
        type = types.nullOr types.bool;
        default = null;
        description = ''
          Enable the `cliphist` clipboard mode. `cliphist`, `wl-paste` and
          `wl-copy` then need to be reachable on {env}`PATH`; see
          {option}`programs.slowshell.systemd.extraPackages`.
        '';
      };
    };

    theme = mkOption {
      type = types.attrsOf types.str;
      default = { };
      example = literalExpression ''
        {
          base = "#1e1e2e";
          text = "#cdd6f4";
          primary = "#eba0ac";
        }
      '';
      description = ''
        Inline palette, applied as a `theme { … }` block. Valid keys are
        ${concatStringsSep ", " (map (key: "`${key}`") paletteNames)}; unset
        colors keep slowshell's built-in values. Colors are `#rrggbb` or
        `#rrggbbaa`.

        Slowshell's `theme "name"` form reads `themes/<name>.kdl` relative to
        the config file. That file would have to live in the Nix store next to
        the generated config, so this module inlines the palette instead.
      '';
    };

    css = mkOption {
      type = types.lines;
      default = "";
      example = ''
        .component { font-size: 13px; }
      '';
      description = ''
        CSS appended to slowshell's built-in stylesheet, emitted as an
        `css "<…>"` node. Used instead of `include-css`, whose relative path
        would not resolve through the store symlink.
      '';
    };

    wallpaper = {
      enable = lib.mkEnableOption "a wallpaper block" // {
        description = ''
          Whether to configure wallpaper handling. Without
          {option}`programs.slowshell.wallpaper.file` or
          {option}`programs.slowshell.wallpaper.paths` no wallpaper is drawn.
        '';
      };

      backend = mkOption {
        type = types.nullOr types.str;
        default = null;
        example = "swaybg";
        description = ''
          Backend that draws the wallpaper. The default, `slowshell`, is the
          built-in renderer; for anything else slowshell also needs an
          {option}`programs.slowshell.wallpaper.spawnArgs` command line.
        '';
      };

      file = mkOption {
        type = types.nullOr types.str;
        default = null;
        example = "/home/you/Pictures/wallpaper.png";
        description = ''
          Image used for every output. Slowshell does not expand `~`, so this
          must be an absolute path.
        '';
      };

      paths = mkOption {
        type = types.attrsOf types.str;
        default = { };
        example = literalExpression ''
          {
            "HDMI-A-1" = "/home/you/Pictures/left.png";
            "eDP-1" = "/home/you/Pictures/laptop.png";
          }
        '';
        description = "Per-output images, keyed by output name; these take precedence over `file`.";
      };

      spawnArgs = mkOption {
        type = types.nullOr types.str;
        default = null;
        example = "-m fill";
        description = "Extra arguments passed to an external wallpaper backend.";
      };

      maxWidth = mkOption {
        type = types.nullOr types.ints.positive;
        default = null;
        description = "Downscale cap for the built-in renderer.";
      };

      maxHeight = mkOption {
        type = types.nullOr types.ints.positive;
        default = null;
        description = "Downscale cap for the built-in renderer.";
      };
    };

    widgets = mkOption {
      type = types.attrsOf (
        types.submodule {
          options = {
            inner = mkOption {
              type = types.nullOr types.str;
              default = null;
              example = "widgets/systemmon";
              description = "Renderable drawn inside the widget (`slowshell list renderables`).";
            };

            width = mkOption {
              type = types.nullOr types.ints.positive;
              default = null;
              description = "Width in pixels (slowshell default `200`).";
            };

            height = mkOption {
              type = types.nullOr types.ints.positive;
              default = null;
              description = "Height in pixels (slowshell default `200`).";
            };

            x = mkOption {
              type = types.nullOr types.int;
              default = null;
              description = "Horizontal offset from the anchor.";
            };

            y = mkOption {
              type = types.nullOr types.int;
              default = null;
              description = "Vertical offset from the anchor.";
            };

            anchor = mkOption {
              type = types.nullOr (
                types.enum [
                  "top-left"
                  "top-center"
                  "top-right"
                  "center-left"
                  "center"
                  "center-right"
                  "bottom-left"
                  "bottom-center"
                  "bottom-right"
                ]
              );
              default = null;
              description = "Corner the widget is anchored to (slowshell default `top-left`).";
            };

            layer = mkOption {
              type = types.nullOr (
                types.enum [
                  "bottom"
                  "top"
                  "overlay"
                ]
              );
              default = null;
              description = "Layer-shell layer the widget is drawn on.";
            };

            monitor = mkOption {
              type = types.nullOr types.str;
              default = null;
              example = "*";
              description = "Output name, or `\"*\"` for every monitor.";
            };

            root = mkOption {
              type = types.bool;
              default = false;
              description = "Whether this is a root (full-screen) widget.";
            };

            settings = mkOption {
              type = kdlValueType;
              default = { };
              example = literalExpression ''
                {
                  format = "%H:%M";
                  "show-seconds" = false;
                }
              '';
              description = ''
                Options passed through to the widget's renderable, one KDL node
                per attribute. See the renderable's documentation for the
                option names it understands.
              '';
            };
          };
        }
      );
      default = { };
      description = "Desktop widgets, keyed by widget name.";
    };

    panels = mkOption {
      type = types.attrsOf (
        types.submodule {
          options = {
            position = mkOption {
              type = types.nullOr (
                types.enum [
                  "top"
                  "bottom"
                  "left"
                  "right"
                ]
              );
              default = null;
              description = "Edge the panel is attached to (slowshell default `top`).";
            };

            monitor = mkOption {
              type = types.nullOr types.str;
              default = null;
              example = "*";
              description = "Output name, or `\"*\"` for one panel per monitor.";
            };

            height = mkOption {
              type = types.nullOr types.ints.positive;
              default = null;
              description = "Panel thickness in pixels (slowshell default `32`).";
            };

            transparent = mkOption {
              type = types.nullOr types.bool;
              default = null;
              description = "Whether the panel background is transparent.";
            };

            overlay = mkOption {
              type = types.nullOr types.bool;
              default = null;
              description = "Draw the panel on the overlay layer.";
            };

            autohide = mkOption {
              type = types.nullOr types.bool;
              default = null;
              description = "Hide the panel until the pointer approaches the edge.";
            };

            autohideTrigger = mkOption {
              type = types.nullOr types.ints.positive;
              default = null;
              description = "Distance in pixels from the edge that reveals an autohidden panel.";
            };

            sections = mkOption {
              type = types.attrsOf (types.listOf panelEntryType);
              default = { };
              example = literalExpression ''
                {
                  left = [ "core/workspaces" ];
                  center = [ { component = "core/clock"; settings.format = "%I:%M"; } ];
                  right = [
                    {
                      label = "System";
                      items = [ { component = "core/audio"; label = "Audio"; } ];
                    }
                  ];
                }
              '';
              description = ''
                Panel contents keyed by section (`left`, `center`, `right`).
                Each entry is a component name, a component attrset, or a group
                attrset — one that carries `items`. Empty sections are omitted.
              '';
            };
          };
        }
      );
      default = { };
      description = "Panels, keyed by panel name (for example `Main`).";
    };

    extraConfig = mkOption {
      type = types.lines;
      default = "";
      example = ''
        widgets {
          widget "Clock" {
            inner "core/clock"
          }
        }
      '';
      description = ''
        Raw KDL appended to the generated config, for anything this module does
        not model: plugin blocks, component options that need the raw syntax,
        components not covered by the typed options, and so on.

        Top-level blocks are looked up by their first occurrence in the file, so
        a second `theme` or `wallpaper` block here does not override the
        generated one.
      '';
    };

    checkConfig = mkOption {
      type = types.bool;
      default = cfg.package != null;
      defaultText = literalExpression "programs.slowshell.package != null";
      description = ''
        Whether to validate the generated config at build time with
        {command}`slowshell config --validate`. This needs
        {option}`programs.slowshell.package`.
      '';
    };
  };

  config = mkIf cfg.enable {
    assertions = [
      {
        assertion = !cfg.checkConfig || cfg.package != null;
        message = "programs.slowshell.checkConfig needs programs.slowshell.package to be set.";
      }
      {
        assertion = !cfg.systemd.enable || cfg.package != null;
        message = "programs.slowshell.systemd.enable needs programs.slowshell.package to be set.";
      }
      {
        # Slowshell builds the config path from $HOME and ignores
        # XDG_CONFIG_HOME, while Home Manager links the file under
        # `xdg.configHome`. If a user moves configHome elsewhere the file would
        # be generated but never read, so fail loudly instead.
        assertion = config.xdg.configHome == "${config.home.homeDirectory}/.config";
        message = ''
          programs.slowshell expects `xdg.configHome` to be `$HOME/.config`
          (currently ${config.xdg.configHome}), because slowshell resolves its
          config path from $HOME and ignores XDG_CONFIG_HOME.
        '';
      }
    ];

    warnings =
      let
        unknownColors = lib.subtractLists paletteNames (lib.attrNames cfg.theme);
        unknownSections = lib.subtractLists [ "left" "center" "right" ] (
          lib.concatMap (panel: lib.attrNames panel.sections) (lib.attrValues cfg.panels)
        );
      in
      optional (unknownColors != [ ])
        "programs.slowshell.theme: slowshell has no ${
          concatStringsSep ", " (map (key: "`${key}`") unknownColors)
        }; valid colors are ${concatStringsSep ", " (map (key: "`${key}`") paletteNames)}."
      ++
        optional (unknownSections != [ ])
          "programs.slowshell.panels.*.sections: slowshell only reads `left`, `center` and `right`, ignoring ${
            concatStringsSep ", " (map (key: "`${key}`") unknownSections)
          }.";

    home.packages = optional (cfg.package != null) cfg.package;

    # Slowshell reads `$HOME/.config/slowshell/config.kdl` by joining `HOME`
    # with a literal path, ignoring XDG_CONFIG_HOME, so the file is only found
    # when the two agree.
    xdg.configFile."slowshell/config.kdl".source = configFile;

    systemd.user.services.slowshell = mkIf activeService {
      Unit = {
        Description = "slowshell wayland desktop shell";
        Documentation = [ "https://slowshell.bushyice.com" ];
        PartOf = [ cfg.systemd.target ];
        After = [ cfg.systemd.target ];
        ConditionEnvironment = "WAYLAND_DISPLAY";
        # Home Manager links the config into the store, so slowshell's file
        # watcher sees the store directory rather than `~/.config` and never
        # fires on a switch. Embedding the config path in the unit makes its
        # content change whenever the config does, which is what makes
        # Home Manager restart the running shell.
        X-Restart-Triggers = [ "${configFile}" ];
      };

      Service = {
        ExecStart = serviceExec;
        Restart = "on-failure";
        RestartSec = 2;
        # Helper processes (wallpaper backend, cliphist) should not keep the
        # unit alive after it stops.
        KillMode = "mixed";
      };

      Install.WantedBy = [ cfg.systemd.target ];
    };
  };
}
