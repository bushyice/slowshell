+++
title = "CSS"
description = "Restyling your shell with CSS."
weight = 4
+++

[[Slowshell]] uses [CSS](https://developer.mozilla.org/en-US/docs/Web/CSS) for styling,
which can be part of your [[Configuration|`config.kdl`]].

## Overriding built-in styles

Within your [[Configuration|`config.kdl`]] you can either include a css file or
actually just write css inline:

```kdl
// inline
css """
.component { color: var(--subtext); }
"""

// external files:
include-css "style.css"
```

**Note**: If you do use external files, I recommend to put the css files within the `config.kdl`'s parent.
{% <tree> %}
~/.config/
  slowshell/
    config.kdl
    style.css
{% </tree> %}

## Using theme palette colors

There are CSS variables generated from theme colors out of your [[Theme and palette|palette]]. And you can just
address them as below:

```css
.component {
  color: var(--subtext);
}
```

## Available Classes

There are more than just a few classes and a few properties to list here as a lot of components and ui widgets
have custom properties. As well as that [[Plugins|plugins]] can extend the stylesheet with their own
classes and properties.

However, Whenever you need to view classes and properties, you can run the below command to show your whole
active stylesheet.
```sh
slowshell list styles
```

You can copy the output and get started with building your own style for your shell.

<details>

<summary>Show built-in classes for current version</summary>

```css
.component {
  color: var(--text);
  color-faded: var(--subtext);
  color-invert: var(--base);
  color-primary: var(--primary);
  font-size: 12px;
  icon-size: 14px;
  gap: 5px;
  padding-v: 5px;
  padding-h: 9px;
  hover-background: var(--primary-a18);
  hover-radius: 4px;
}

.component.vertical {
  font-size: 15px;
  icon-size: 18px;
  gap: 8px;
  padding-v: 4px;
  padding-h: 5px;
}

.component.faded { color: var(--subtext); }
.component.invert { color: var(--base); }
.component.primary { color: var(--primary); }
.component.secondary { color: var(--yellow); }
.component.danger { color: var(--red); }

.workspaces {
  font-size: 12px;
  gap: 4px;
  padding-v: 0px;
  padding-h: 10px;
  border-radius: 999px;
  border-width: 0px;
  border-color: var(--overlay);
  width-active: 32px;
  width-inactive: 24px;
}

.workspaces.active { background-color: var(--primary); color: var(--base); }
.workspaces.inactive { background-color: var(--text); color: var(--base); }

.windows {
  color: var(--text);
  color-faded: var(--subtext);
  color-active: var(--primary);
  background-active: var(--primary-a18);
  font-size: 11px;
  icon-size: 22px;
  gap: 3px;
  padding-v: 2px;
  padding-h: 4px;
  border-radius: 6px;
}

.windows.item { border-radius: 6px; }
.windows.item.active { background-color: var(--primary-a18); }
.windows.item.open { background-color: var(--crust); }

.windows.active { color: var(--primary); }
.windows.open { color: var(--text); }
.windows.closed { color: var(--subtext); }
.menu {
  background-color: var(--crust-a95);
  border-radius: 12px;
  border-width: 1px;
  border-color: var(--overlay-a15);
  padding-v: 16px;
  padding-h: 16px;
  gap: 12px;
  color: var(--text);
  font-size: 13px;
}

.menu.row {
  background-color: var(--mantle-a50);
  hover-background: var(--mantle-a70);
  border-radius: 6px;
  border-width: 0px;
  border-color: var(--overlay-a15);
  padding-v: 16px;
  padding-h: 16px;
  gap: 8px;
}

.menu.row.compact {
  padding-v: 7px;
  padding-h: 12px;
}

.menu.row.solid { background-color: var(--mantle); }

.menu.hover { hover-background: var(--mantle-a70); }

.menu.text-header { font-size: 15px; }
.menu.text-name { font-size: 13px; }
.menu.text-body { font-size: 13px; }
.menu.text-status { font-size: 11px; }
.menu.text-faded { color: var(--subtext); }
.menu.text-primary { color: var(--primary); }
.menu.text-secondary { color: var(--yellow); }
.menu.text-danger { color: var(--red); }
.menu.text-invert { color: var(--base); }

.menu.button {
  background-color: var(--mantle);
  hover-background: var(--overlay-a15);
  border-radius: 8px;
  border-width: 1px;
  border-color: var(--overlay-a15);
  font-size: 12px;
  padding-v: 16px;
  padding-h: 16px;
}
.menu.button:hover { background-color: var(--overlay-a15); }

.menu.badge {
  background-color: var(--mantle-a50);
  border-radius: 8px;
  border-width: 0px;
  padding-v: 2px;
  padding-h: 8px;
}
.menu.chip {
  background-color: var(--mantle-a50);
  hover-background: var(--overlay-a15);
  border-radius: 10px;
  border-width: 0px;
  padding-v: 4px;
  padding-h: 10px;
}

.menu.pill {
  background-color: transparent;
  hover-background: var(--mantle-a70);
  border-radius: 6px;
  border-width: 0.5px;
  border-color: var(--overlay);
  padding-v: 16px;
  padding-h: 16px;
}
.menu.pill.active {
  background-color: var(--mantle);
  border-width: 1px;
  border-color: var(--primary);
}

.menu.arrow { color: var(--overlay); }
.menu.connected { color: var(--green); }
.menu.separator { color: var(--base); }
.menu.border { color: var(--overlay); }

.menu.qs { width: 340px; }
.menu.qs.avatar { avatar-size: 42px; }
.menu.qs.tile {
  background-color: var(--mantle);
  hover-background: var(--overlay-a15);
  border-radius: 12px;
  border-width: 1px;
  border-color: var(--overlay-a15);
  padding-v: 16px;
  padding-h: 16px;
  font-size: 13px;
}
.menu.qs.tile.active { border-color: var(--primary); }

.menu.qs.tile.status { font-size: 11px; }
.menu.qs.tile { icon-size: 18px; }

.menu.clock.time { clock-time-size: 38px; }
.menu.clock.seconds { clock-seconds-size: 15px; }
.menu.calendar { calendar-spacing: 2px; }
.menu.calendar.cell { calendar-cell-height: 28px; }
.menu.calendar.today { calendar-today-radius: 999px; }
.menu.calendar.today.fill {
  background-color: var(--primary);
  border-radius: 999px;
  border-width: 0px;
}

.menu.icon { icon-size: 16px; }
.menu.checkbox { checkbox-size: 14px; }
.menu.arrow-size { arrow-size: 12px; }
.menu.list { list-width: 220px; gap: 8px; }
.menu.list.large { list-large-width: 320px; }
.menu.list.indent { list-indent: true; }
.menu.inner { padding-v: 12px; padding-h: 12px; }

.slider {
  background-color: var(--primary);
  background-rail: var(--mantle);
  background-rail-filled: var(--primary);
  border-color: transparent;
  border-width: 0px;
  rail-width: 4px;
  rail-border-width: 0px;
  rail-border-color: transparent;
  rail-border-radius: 2px;
  rect: false;
  slider-size: 7px;
}
.notifications {
  background-color: var(--crust-a90);
  border-radius: 10px;
  border-width: 1px;
  border-color: var(--overlay-a15);
  padding-v: 12px;
  padding-h: 12px;
  gap: 4px;
}

.notifications.list { gap: 8px; }

.notifications.icon { icon-size: 20px; }
.notifications.app { color: var(--overlay); }
.notifications.app.icon-color { icon-size: 18px; }

.notifications.text-app { color: var(--overlay); font-size: 10px; }
.notifications.text-summary { font-size: 13px; }
.notifications.text-body { font-size: 11px; }

.notifications.input { font-size: 11px; padding: 6px; }

.notifications.action {
  background-color: var(--mantle);
  color: var(--text);
  border-radius: 8px;
  border-width: 0px;
  border-color: var(--overlay-a15);
  font-size: 11px;
  font-weight: 400;
  padding-v: 7px;
  padding-h: 10px;
}
.notifications.action:hover { background-color: var(--overlay-a15); }
.notifications.action:active { background-color: var(--overlay-a25); }
.panel {
  background-color: var(--base-a45);
  border-width: 0px;
  border-color: var(--overlay-a15);
  padding-v: 8px;
  padding-h: 8px;
  gap: 4px;
}

.panel.items { gap: 4px; }

.item {
  background-color: var(--mantle-a60);
  border-radius: 4px;
  border-width: 0px;
  border-color: var(--overlay-a15);
  padding-v: 4px;
  padding-h: 8px;
  margin: 4px;
  font-size: 13px;
  color: var(--text);
  gap: 8px;
}

.item.prefix { border-radius: 4px; }
.spotlight {
  padding-v: 14px;
  padding-h: 14px;
  gap: 8px;
}

.spotlight.search {
  background-color: var(--mantle);
  border-radius: 12px;
  border-width: 1px;
  border-color: var(--overlay-a30);
}

.spotlight.result {
  background-color: var(--mantle);
  border-radius: 12px;
  border-width: 0px;
  border-color: var(--overlay-a15);
}

.spotlight.result.selected {
  background-color: var(--primary);
}

.spotlight.row {
  padding-v: 10px;
  padding-h: 10px;
}

.spotlight.tag {
  background-color: var(--crust-a60);
  border-radius: 4px;
  border-width: 0px;
  border-color: var(--overlay-a15);
}

.spotlight.action-chip {
  background-color: var(--crust-a60);
  border-radius: 4px;
  border-width: 0px;
  border-color: var(--overlay-a15);
}

.spotlight.single {
  padding-v: 18px;
  padding-h: 18px;
  background-color: var(--mantle);
  border-radius: 12px;
  border-width: 1px;
  border-color: var(--overlay-a30);
}

.spotlight.backdrop {
  background-color: var(--crust-a60);
}

.spotlight.text-search { color: var(--text); font-size: 22px; }
.spotlight.text-placeholder { color: var(--overlay); font-size: 22px; }
.spotlight.text-result { color: var(--text); font-size: 15px; }
.spotlight.text-result-selected { color: var(--base); font-size: 15px; }
.spotlight.text-subtitle { color: var(--subtext); font-size: 11px; }
.spotlight.text-subtitle-selected { color: var(--mantle); font-size: 11px; }
.spotlight.text-tile { color: var(--text); font-size: 12px; }
.spotlight.text-tile-selected { color: var(--base); font-size: 12px; }
.spotlight.text-tag { color: var(--subtext); font-size: 10px; }
.spotlight.text-action { color: var(--primary); font-size: 10px; }
.spotlight.text-tile-action { color: var(--base); font-size: 10px; }
.spotlight.text-hint { color: var(--overlay); font-size: 11px; }
.spotlight.text-single { color: var(--text); font-size: 32px; }
.spotlight.text-single-subtitle { color: var(--subtext); font-size: 14px; }
.spotlight.text-single-hint { color: var(--overlay); font-size: 11px; }

.spotlight.accent { color: var(--primary); }
.clock.analog.face {
  background-color: var(--mantle);
  border-width: 3px;
  border-color: var(--overlay);
}

.clock-hand.hour {
  width: 6px;
  height: 26px;
  border-radius: 3px;
}

.clock-hand.minute {
  width: 4px;
  height: 40px;
  border-radius: 2px;
}
```
</details>

---
