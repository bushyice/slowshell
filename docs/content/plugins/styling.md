+++
title = "Styling"
description = "CSS and themes for plugins"
weight = 5
+++

Once your surface can draw, you probably want it to feel like the rest of the shell. The
host hands you CSS and the theme.

## Style

Classes you attach to nodes get resolved against the live config (including user CSS),
and `ctx.style(...)` turns those classes into typed values:
```rust
let style = ctx.style("example cpu");
let radius = style.length("border-radius");    // Option<f32>
let bg = style.color("background-color");      // Option<Color>
let muted = style.color_or("color-faded", ctx.theme().subtext);
let flag = style.bool("fill-width");
let pad = style.padding([0.0, 4.0]);           // [y, x]
```
## The palette

`ctx.theme()` gives you the 13-color [[Theme and palette|palette]]:

```rust
let accent = ctx.theme().primary;
let muted = ctx.theme().subtext;
```

## Register your own CSS

```rust
fn register(host: HostRef<'_>) {
  host.style(r#"
    .example-cpu { font-size: 12px; icon-size: 14px; gap: 4px; }
    .example-cpu.warn { color: #f2b96a; }
  "#);
}
```

---
