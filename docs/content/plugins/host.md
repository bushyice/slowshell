+++
title = "Host and Context"
description = "The object every callback gets."
weight = 2
+++

It is guaranteed that [[Plugins]] would have a context upon which to act. And that
context in slowshell is where almost all capabilities are. 

A lot of the context's functions are situational, meaning that `ctx.dispatch()` could
work in one context and not work in another.

## Logging and errors

Let's start simple, with the unconditional little utilities.

```rust
ctx.log(0, "started");                 // "[plugin:<id>] started" on stderr
ctx.error(1, "something went wrong");  // recorded as the plugin's last error
```

## Options

So, imagine this. You have your `core/clock` settings, and you want it
on your bar. So you put it in the config. But it's 12-hr format, and you'd
like it to be 24-hr format. So you go to the config, and give that specific
[[Components Overview|component]] the configs required to make it 24hrs.

Now think about your own component, and how it would "find" that config
from the hay stack.

That's what `ctx.options()` is. Very situational, only works within
[[Components Overview|components]].

```rust
let label = ctx.options().str("label").unwrap_or("audio");
let show = ctx.options().bool("show-percent").unwrap_or(false);
```

```kdl
component "example/hello" {
  label "Hi"
  show-percent #true
}
```

**where**: `ctx.options()` has `str`, `f64`, `i64`, `bool` for type lookups.

## Config

There are two config mechanisms, and they're independent.

### The raw scalar block

A top-level KDL node named after your plugin id(the cargo package name):

```kdl
hello-plugin {
  interval 500
  show-label #true
}
```

```rust
let interval = ctx.config_i64("interval").unwrap_or(1000);
let show = ctx.config_bool("show-label").unwrap_or(false);
```

**where**: the values are STRICTLY scalar, and accessed via
  - `config_f64`
  - `config_i64` 
  - `config_bool`

### Typed parsers

Configs have to be parsed, and if you need more than just scalars,
you might have to parse the config manually yourself.

I'm not personally a huge "Typed shit" advocate, but typed configs
are among the best solutions for this application, so enjoy!

<!-- i honestly have no clue what i am writing at this point -->

```rust
use slowshell_plugin_abi::{prelude::*, kdl::KdlNode};

#[derive(Clone)]
struct WidgetConfig {
  interval: u64,
  label: String,
}

fn register(host: HostRef<'_>) {
  // or config_parser_try if you want a failing variant
  host.config_parser::<WidgetConfig>("example-widget", |node| {
    let interval = node
      .and_then(|n| n.children())
      .and_then(|c| c.get("interval"))
      .and_then(|n| n.entries().first())
      .and_then(|e| e.value().as_integer())
      .unwrap_or(250)
      .max(1) as u64;
    WidgetConfig { interval, label: "CPU".into() }
  });
}
```

Then read it back wherever you like:

```rust
// keep in mind, this clones. 
if let Some(config) = ctx.config::<WidgetConfig>("example-widget") {
  let _ = config.interval;
}

// without cloning:
let the_result = ctx.with_config::<WidgetConfig, _>("example-widget", |c| {
  Node::text(format!("interval: {}", c.interval))
});
```

## Service state

Slowshell, as a shell, needs a few services to keep the stats accurate. So we 
have services within. Naturally, it would make sense for the internal state
to be shared across to plugins. So yeah, here's a list of all stable shared
states:

| Method | Snapshot |
|---|---|
| `ctx.audio_state()` | Volume, mute, sinks, current MPRIS player |
| `ctx.bluetooth_state()` | Adapter power, devices, discovery |
| `ctx.network_state()` | Wi-Fi/ethernet, access points, connection info |
| `ctx.system_state()` | CPU, memory, temperature, load, network counters, processes |
| `ctx.tray_state()` | Tray items and icons |
| `ctx.power_state()` | Battery, brightness, power profiles |
| `ctx.compositor_state()` | Monitors, workspaces, active window, overview |

Most of these services only start once a component that owns them shows up in a panel.

## The shared store

The store is a global typed map, which is how plugins (and shell code) pass values around
without sharing a dependency.

```rust
ctx.store().insert(MyValue { hits: 0 });       // false if already present
ctx.store().contains::<MyValue>();
ctx.store().with::<MyValue, _>(|v| v.hits += 1);
ctx.store().borrow::<MyValue>();
ctx.store().touch::<MyValue>();                // notify subscribers
ctx.store().remove::<MyValue>();
ctx.store().subscribe::<MyValue, _>(|ctx| {
  ctx.log(0, "MyValue changed");
});
```

## Persistence

Yeah this one is pretty self-explanatory.

```rust
ctx.persistence_set_raw("last-id", "abc");
let last = ctx.persistence_get_raw("last-id");       // Option<RString>

// Serde
#[derive(serde::Serialize, serde::Deserialize)]
struct State { count: u32 }

ctx.persistence_set("state", &State { count: 3 });
let state: Option<State> = ctx.persistence_get("state");
let state = ctx.persistence_open_or_get("state", State { count: 0 });

ctx.persistence_has("state");
ctx.persistence_remove("last-id");
```

> **Note**: The saved typed data uses [ron](https://crates.io/crates/ron), which is the reason you need [serde](https://crates.io/crates/serde).

## Canvas and images

The [[Surfaces#What are Nodes|nodes]] that slowshell provides may not be
always capable of expressing what you want to render. Which is why canvases
exist:

```rust
ctx.canvas_node(200, 100, |pixels| {
  // Clear as such for every new frame draw 
  pixels.fill(0);
  // pixels.len() == 200 * 100 * 4, RGBA8, row-major, top-left origin
})
.width(Length::Fixed(200.0))
.height(Length::Fixed(100.0));
```

## Quick utils

Some of the utilities you saw in examples.

```rust
ctx.set_interval(1000, true, 7);   // repeating; ticks arrive as Event::Tick { tag: 7, fd }
ctx.set_interval(1000, false, 8);  // one-shot
ctx.register_fd(fd, 42);           // ready fds arrive as Event::Fd { action: 42, fd }
ctx.request_redraw(0);             // upgrades a None effect to Redraw
```

## Events and subscriptions

Your `events()` return value is what wakes your surface up:

| Filter | Delivers |
|---|---|
| `Tick` | `Event::Tick` for your timers and fds |
| `Fd` | `Event::Fd` for your fds |
| `Widget` | `Event::Widget` for text input, raw wrapper, and scroll events |
| `Named(name)` | Named shell signals and payload actions with no payload |
| `Payload(name)` | Commands, with or without a payload |
| `Ipc(name)` | Commands sent over the socket under that name |
| `Compositor(mask)` | Compositor changes matching the mask |
| `CompositorUpdate` | Any compositor change |
| `Frame` | Every rendered frame (use sparingly) |
| `ConfigReload` | Config reloads |
| `StartUp` | Once at startup |
| `All` | Everything |

If that list needs to change while you're running, return `ItemEffect::Subscribe` and
hand back a fresh list from `events()`.

The built-in services also publish named wakeups. The event carries no data, so re-read
the matching `ctx.*_state()` when one arrives:

| Name | Meaning |
|---|---|
| `component/audio.changed` | Volume, sinks, or MPRIS state changed |
| `component/bluetooth.changed` | Adapter/device state changed |
| `component/network.changed` | Network state changed |
| `component/notifications.changed` | Notification list changed |
| `component/power.changed` | Battery/profile/brightness changed |
| `desktop-entries.changed` | `.desktop` database loaded or updated |
| `config.reload` | Config reloaded (same as `ConfigReload`) |

Text inputs, event wrappers, and scrollables produce `Event::Widget`. The node's `id`
lands in `event.id` and the text in `event.value`, while event wrappers encode raw input
as space-separated strings like `keydown <code> <repeat> <mods> <text>` or `mousedown
<button>`.

## Dispatching commands

```rust
ctx.dispatch("panel.main.toggle");                          // named action
ctx.dispatch_with_string("audio.set_volume", "value=0.5");   // parsed payload
ctx.dispatch_payload("my.command", PayloadBox::new(args));   // typed payload
```

From a view, clicks are payloads too: `Node::clickable(child).action("my.command")`.

See [[Extensible modules#Payloads and commands]] for handlers and parsers.

## Compositor mutation

More situational functions. These *ONLY* work on a compositor implementation.

```rust
ctx.compositor_state();                        // read the current snapshot
ctx.set_compositor_state(state);               // publish a new snapshot
ctx.host().compositor_register_fd(fd, flags);  // low-level; does not take ownership
```

---
