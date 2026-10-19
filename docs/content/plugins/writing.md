+++
title = "Writing a Plugin"
description = "A simple showcase."
weight = 1
+++

Alright, time to write a very simple showcase plugin, shall we? We'll
mainly look at dependencies and versioning while making a panel component. For details,
you can check [[Host and Context]], [[Surfaces]] and [[Extensible Modules]].

## The crate

```sh
cargo new --lib my-plugin
```

```toml
# add these to your Cargo.toml
[lib]
crate-type = ["cdylib"]

[dependencies]
slowshell-plugin-abi = { git = "https://tangled.org/bushyice.com/slowshell" }
```

> **Note**: The package name and package version will be the name and version of the plugin as well.

## Write it

A component that updates once per second:

```rust
use slowshell_plugin_abi::prelude::*;

// This is kind of the ID for the
// tick event.
const TICK: u32 = 0xA001;

struct TheComponent {
  count: u32,
}

impl Component for TheComponent {
  fn new(_ctx: &Context<'_>) -> Self {
    Self { count: 0 }
  }

  fn events(&self) -> RVec<EventFilter> {
    vec![EventFilter::Tick].into()
  }

  // This happens when the component is
  // first initiated.
  fn watch(&mut self, ctx: &Context<'_>) {
    
    ctx.set_interval(1000, true, TICK);
  }

  // This happens for every event
  // in the events() method above.
  fn update(&mut self, _ctx: &Context<'_>, event: &Event) -> ItemEffect {
    match event {
      // The TICK, by tag.
      Event::Tick { tag, .. } if *tag == TICK => {
        self.count += 1;
        ItemEffect::Redraw
      }
      _ => ItemEffect::None,
    }
  }

  // The rendering
  fn view(&self, _ctx: &Context<'_>) -> Node {
    Node::text(format!("counting: #{:03}", self.count))
      .size(12.0)
      .classes(&["the-component"])
  }
}

fn register(host: HostRef<'_>) {
  host.component::<TheComponent>("my-plugin/the-component"); // or whatever name
}

export_plugin!(register);
```

## Build and install

```sh
cargo build --release
mkdir -p ~/.local/share/slowshell/plugins
cp target/release/libmy_plugin.so ~/.local/share/slowshell/plugins/
```

You can check if it loaded with this:

```sh
slowshell list plugins
# my-plugin 0.1.0 /home/you/.local/share/slowshell/plugins/libmy_plugin.so
slowshell list components | grep my-plugin
```

## Enable and disable

```kdl
plugins {
  enabled "a" "b"
  disabled "c"
}
```

## Dependencies

```rust
export_plugin!(register, requires = [("other-plugin", "^0.2")]);
```
**where**:
- versions use [semver](https://semver.org/) versioning

---
