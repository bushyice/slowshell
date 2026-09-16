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

## Plugins (overview and docs)

The decision for plugins was pretty messy- Wasm has an overhead and is sandboxed,
and i was trying to avoid scripting languages. I thought about luajit but sounded
like just as much work as C-ABI native plugins.

And so, native `.so` plugins it is! **They are supposed to be able to provide modules
and components that the core doesn't, and are able to use most features that the core
has**.

Currently, plugins are designed with C-ABI, but my target is to support rust plugins
initially with all the wrappers and wiring required to make it rusty. 

- **Note**:
  - Plugins' `.so` files are discovered from `$SLOWSHELL_PLUGIN_PATH`,
      `~/.local/share/slowshell/plugins`, `$XDG_DATA_DIRS/slowshell/plugins`. 
  - They can be enabled/disabled with:
      ```kdl
      // config.kdl
      plugins {
        enabled "example-hello" "git-status"
        disabled "broken-thing"
      }
      ```

## Versioning

The ABI version is a single `u32` checked once at load time. A plugin with a different
version than the host willn't be loaded.

```symbol
crates/plugin/include/slowshell-plugin.h#SL_PLUGIN_ABI_VERSION
```

The symbols the host looks up are:

```symbol
crates/plugin/include/slowshell-plugin.h#slowshell_plugin_init
crates/plugin/include/slowshell-plugin.h#SlPluginMeta
```

The host exports the API to the plugin through a single `SlHostApi` table of
function pointers:

```symbol
crates/plugin/include/slowshell-plugin.h#SlHostApi
```

## Ownership and lifetimes

The rules from the header:

- **plugin -> host** strings and arrays are borrowed for that specific call.
- **host -> plugin** handles are valid for the call except the
  `SlHostApi` pointer and the plugin's `ctx` pointer, which are `'static`.
- Canvas buffers returned are host-owned and reused and the pointer is
  valid only until the next view begins.
- Vtables must be `'static` and the SDK leaks them for that reason.

The core string type is `SlStr`: a borrowed pointer + length, NOT
NUL-terminated, and allowed to be null.

```symbol
crates/plugin/include/slowshell-plugin.h#SlStr
```

## Registration surfaces

A plugin registers things inside `slowshell_plugin_init`. 

With the rust sdk, you get these from the registerar:

- `component::<T>("name")`  
- `renderable::<T>("name")`
- `payload::<T>("command")`
- `desktop_item::<T>("name")`
- `compositor::<T>("name")`
- `spotlight::<T>("name")`
- `style("name", &sheet)`
- `config_parser::<T>(…)`



The C-level vtables are declared in the header, for example a component:

```symbol
crates/plugin/include/slowshell-plugin.h#SlComponentVtable
```

## Writing a rust plugin

Add the SDK and build a `cdylib`:

```toml
[package]
name = "example-hello"        # this will be the plugin id
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
slowshell-plugin-sdk = { git = "https://tangled.org/bushyice.com/slowshell" }
```

And then implement a trait and export it (requires `Send + 'static`).

```rust
use slowshell_plugin_sdk::{
  export_plugin, Component, Context, Event, EventMask, ItemEffect, Node, Registrar,
};

struct Hello {
  count: u32,
}

impl Component for Hello {
  fn new(_ctx: &Context) -> Self {
    Hello { count: 0 }
  }

  fn events(&self) -> EventMask {
    EventMask::TICK
  }

  fn watch(&mut self, ctx: &Context) {
    ctx.set_interval(1000, true, 7); // 1s repeating, tag 7
  }

  fn update(&mut self, _ctx: &Context, event: &Event) -> ItemEffect {
    if matches!(event, Event::Tick { tag: 7, .. }) {
      self.count += 1;
      return ItemEffect::Redraw;
    }
    ItemEffect::None
  }

  fn view(&self, _ctx: &Context) -> Node {
    Node::text(format!("hello {}", self.count))
  }
}

fn register(reg: &mut Registrar) {
  reg.component::<Hello>("example/hello");
}

export_plugin!(register);
```

`export_plugin!` emits `slowshell_plugin_meta`, `slowshell_plugin_init` and
`slowshell_plugin_shutdown`, and calls your function with `Registrar`.

```symbol
crates/plugin-sdk/src/lib.rs#Component
crates/plugin-sdk/src/lib.rs#Registrar
crates/plugin-sdk/src/lib.rs#export_plugin
```

The `Context` passed to callbacks is the plugin's view of the host: options,
logging, timers and fds, canvas allocation, config access, styles and themes,
compositor state, service snapshots, notifications, the shared registry and
`dispatch`.

```symbol
crates/plugin-sdk/src/lib.rs#Context
```

### Other traits

| Trait | Purpose | Required methods |
| --- | --- | --- |
| `Component` | Panel component. | `new`, `view` |
| `Renderable` | Standalone surface (popups, widgets). | `new`, `view` |
| `Payload` | Command handler. | `new`, `invoke` |
| `DesktopItem` | Layer-shell desktop item. | `new`, `settings`, `view` |
| `Compositor` | Compositor backend adapter. | `new` |
| `Spotlight` | Spotlight mode provider. | `new` |


### Nodes

The SDK also provides `Node` and its constructors (`Node::row`, `Node::column`, `Node::text`, `Node::icon`,
`Node::progress`, `Node::image`, `Node::canvas`, ...), `Style`, `Border`,
`Color`, `Theme`, `StyleSheet`, `Notification`, `Options`, `Args`, `Event`,
`EventMask` and `ItemEffect`.

## Writing a C plugin

Include the header and export the three symbols. A minimal component:

```c
#include "slowshell-plugin.h"
#include <stdlib.h>
#include <string.h>

typedef struct { unsigned ticks; } Hello;

static void *hello_create(void *ctx) { (void)ctx; return calloc(1, sizeof(Hello)); }

static void  hello_destroy(void *ctx, void *s) { (void)ctx; free(s); }

static uint32_t hello_events(void *ctx, void *s) { (void)ctx; (void)s; return SL_EVENT_MASK_TICK; }

static void  hello_watch(void *ctx, void *s, void *o) { (void)ctx; (void)s; (void)o; }

static SlEffect hello_update(void *ctx, void *s, const SlEvent *e) {
  (void)ctx;
  Hello *h = s;
  if (e->kind == SL_EVENT_TICK) h->ticks++;
  return (SlEffect){ .code = SL_EFFECT_NONE };
}

static bool hello_check_view(void *ctx, void *s, void *o) { (void)ctx; (void)s; (void)o; return true; }

static void hello_stop(void *ctx, void *s) { (void)ctx; (void)s; }

static void hello_view(void *ctx, void *s, void *o, SlNodeList *out) {
  (void)ctx; (void)o;

  static char buf[64];
  static SlNode node;

  memset(&node, 0, sizeof node);
  snprintf(buf, sizeof buf, "hello %u", ((Hello *)s)->ticks);

  node.kind = SL_NODE_TEXT;
  node.text = (SlStr){ (const uint8_t *)buf, strlen(buf) };
  node.size = 12.0f;

  out->nodes = &node;
  out->len = 1;
}

static const SlComponentVtable VT = {
  .size = sizeof(SlComponentVtable),
  .create = hello_create, .destroy = hello_destroy, .events = hello_events,
  .watch = hello_watch, .update = hello_update, .view = hello_view,
  .check_view = hello_check_view, .stop = hello_stop,
};

int32_t slowshell_plugin_init(const SlHostApi *api, void *host, void **out_userdata) {
  if (out_userdata) *out_userdata = NULL;
  api->register_component(host, (SlStr){ (const uint8_t *)"example/hello", 13 }, &VT, NULL);
  return 0;
}

void slowshell_plugin_shutdown(void *userdata) { (void)userdata; }

SlPluginMeta slowshell_plugin_meta(void) {
  return (SlPluginMeta){
    SL_PLUGIN_ABI_VERSION,
    (SlStr){ (const uint8_t *)"example-hello", 13 },
    (SlStr){ (const uint8_t *)"0.1.0", 5 },
  };
}
```

Build and install:

```sh
cc -shared -fPIC -I crates/plugin/include -o example_hello.so plugin.c
mkdir -p ~/.local/share/slowshell/plugins
cp example_hello.so ~/.local/share/slowshell/plugins/
```

## Config parsers

A plugin can add a root-level kdl block. The callback receives the raw block as
string (or a null pointer if the block is absent) and returns non-zero to
report that the entry was rejected.

```symbol
crates/plugin/include/slowshell-plugin.h#SlConfigParserFn
```

In rust:

```rust
reg.config_parser::<MyConfig>("my-plugin", |node| { /* parse */ });
// later
let config: Option<&MyConfig> = ctx.config::<MyConfig>("my-plugin");
```

If a plugin's config block is named after its id (in rust, the crate's name), the host exposes
its children through `ctx.config_str/f64/i64/bool` (long as they are just scalars).
