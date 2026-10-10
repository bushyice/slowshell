+++
title = "Theme and palette"
description = "Slowshell theme palettes."
weight = 3
+++

The theme in [[Slowshell|slowshell]] is a simple 13-key color list,
where each color is a hex string and will be generated into css variables to 
be used in [[#CSS|CSS]].

> **Note**: All 13 keys are always required to define a theme. (I think?)

## Inline Themes

You can write your theme inline in your [[Configuration|`config.kdl`]].

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

## External Themes

You can write your theme in a separate file and use it your [[Configuration|`config.kdl`]].
To use themes from external files, the theme needs to be under a `themes` folder in the same
path as your `config.kdl`.

{% <tree> %}
~/.config/
  slowshell/
    themes/
      catppuccin-mocha.kdl
    config.kdl
{% </tree> %}

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

## CSS

The css variables generated to be used with [[CSS|css stylesheets]] from your
pallette follow a pattern where the variable name contains `--colorkey`, with
`--colorkey-a05` up to `--colorkey-a95` for different levels of color opacity.
  - where `--colorkey` can be any of the 13 color keys used to define the pallette.

```css
:root {
  --base: #1e1e2e;
  --base-a05: rgba(30, 30, 46, 0.05);
  --base-a10: rgba(30, 30, 46, 0.1);
  --base-a15: rgba(30, 30, 46, 0.15);
  --base-a18: rgba(30, 30, 46, 0.18);
  --base-a20: rgba(30, 30, 46, 0.2);
  --base-a25: rgba(30, 30, 46, 0.25);
  --base-a30: rgba(30, 30, 46, 0.3);
  --base-a35: rgba(30, 30, 46, 0.35);
  --base-a40: rgba(30, 30, 46, 0.4);
  --base-a45: rgba(30, 30, 46, 0.45);
  --base-a50: rgba(30, 30, 46, 0.5);
  --base-a55: rgba(30, 30, 46, 0.55);
  --base-a60: rgba(30, 30, 46, 0.6);
  --base-a65: rgba(30, 30, 46, 0.65);
  --base-a70: rgba(30, 30, 46, 0.7);
  --base-a75: rgba(30, 30, 46, 0.75);
  --base-a80: rgba(30, 30, 46, 0.8);
  --base-a85: rgba(30, 30, 46, 0.85);
  --base-a90: rgba(30, 30, 46, 0.9);
  --base-a95: rgba(30, 30, 46, 0.95);
  /* ... */
}
```

And using it in your stylesheet can be as simple as:

```css
.component {
  color: var(--text); /* text is one of the 13 color keys */
}
```

---
