+++
title = "Plugins"
description = "Yeah."
sort_by = "weight"
+++

*A little bit about me*:

I always liked extensibility, programs that don't lock their internal machinery and allow
external programs to use and build upon the existing systems... That's what makes a program
actually alive. 

In [[Slowshell]], plugins are what I thought would cover the areas that others want to explore.
You could make your own shell past the primitives provided using actual programs- not just configs. 
That's the whole point.

## What do plugins do?

Well, they can introduce new functionality besides expanding on what already exists. For example,
imagine a plugin that adds completely new bar machinery, or a plugin that extends the current bars
and adds integrations to an existing application you use. Those [[Modules|modules]] you read about
are very extensible, and additive with plugins.

**Things plugins can provide**:
- [[Components Overview|Panel Components]]
- [[Architecture#Renderables|renderable widget]]
- [[Architecture#Desktop Items|Desktop Items]]
- [[Services|Background services]]
- [[Spotlight#Modes|Spotlight modes]]
- [[IPC#Commands|IPC Parsers and Commands]]
- [[Quick settings]]
- [[Configuration|Configs]]
- [[CSS]]

## Hello, plugin

Here's what a simple plugin could look like:

```rust
use slowshell_plugin_abi::prelude::*;

struct Hello;

impl Component for Hello {
  fn new(_ctx: &Context<'_>) -> Self {
    Hello
  }

  fn view(&self, _ctx: &Context<'_>) -> Node {
    Node::text("hello from a plugin")
  }
}

fn register(host: HostRef<'_>) {
  host.component::<Hello>("example/hello");
}

export_plugin!(register);
```

To actually get this to work, you need to put the plugin where slowshell expects them
(you can override this with `SLOWSHELL_PLUGIN_PATH`):
{% <tree> %}
~/.local/share/
  slowshell/
    plugins/
/use/share/
  slowshell/
    plugins/
{% </tree> %}

And to check your new component, you should add it to a panel:
```kdl
panels {
  panel "Main" {
    components {
      right { component "example/hello" }
    }
  }
}
```

## Where to next

- [[Writing a Plugin]]
- [[Host and Context]]
- [[Surfaces]]
- [[Styling]]

---
