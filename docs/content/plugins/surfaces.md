+++
title = "Surfaces"
description = "The UI tree."
weight = 3
+++

Time to introduce you to [[Slowshell|slowshell's]] UI system. Plugins
have [[#What are Nodes|nodes]] that are the ui tree, however, those nodes
are drawn in one of three surfaces. 

Let's start with a simple example:

```rust
fn prefixed_text(ctx: &Context<'_>, text: &str) -> Node {
  Node::text(format!("some ", text))
    .size(12.0)
    .color(ctx.theme().text) // manual styling
    .classes(&["example", "text"]) // css styling
}
```

## What are Nodes

Nodes, the actual UI components views are built from. All nodes are variants
of one main `Node` enum, which is the description of an element tree that the
host turns into widgets.

**Nice, what nodes exist?**

- `Node::row`
- `Node::column`
- `Node::container`
- `Node::scrollable`
- `Node::clickable`
- `Node::icon`
- `Node::text`
- `Node::text_input`
- `Node::event_wrapper`
- `Node::progress`
- `Node::image`
- `Node::slider`
- `Node::stack`
- `Node::canvas`
- `Node::canvas_cached`

## The three surfaces

UI (or nodes) can be rendered in one of three surfaces:


### Components

The small single-purpose areas on top of [[Panels]]. These
surfaces are meant to display small stats, and mostly text
or single icons.

**Example**:
```rust
use slowshell_plugin_abi::prelude::*;

// The tick tag
const TICK: u32 = 1;

pub struct Cpu {
  usage: f32,
  show_label: bool,
}

impl Component for Cpu {
  // This is called when the panel initiates
  // or when the panel has this component
  // added
  fn new(ctx: &Context<'_>) -> Self {
    Self {
      usage: 0.0,
      show_label: ctx.options().bool("label").unwrap_or(true),
    }
  }

  // Unless events are provided,
  // this component will, infact, NOT, update
  fn events(&self) -> RVec<EventFilter> {
    vec![EventFilter::Tick].into()
  }

  // This is the *arming* station. Basically,
  // this is where you would initiate things
  // like services or timers in order to 
  // keep your component live.
  fn watch(&mut self, ctx: &Context<'_>) {
    ctx.set_interval(1000, true, TICK);
  }

  // Event fired => slowshell redirects it to
  // here. Where you requested any "Tick" event
  // to have a meeting with this component.
  fn update(&mut self, ctx: &Context<'_>, event: &Event) -> ItemEffect {
    match event {
      // The tag is basically a filter
      Event::Tick { tag, .. } if *tag == TICK => {
        self.usage = ctx.system_state().map(|s| s.cpu_usage).unwrap_or(0.0);
        ItemEffect::Redraw
      }
      _ => ItemEffect::None,
    }
  }

  // Finally, the actual "surface"
  // 
  // Here, you can use the primitive nodes
  // to build actual ui. Most nodes are
  // nestable, so it won't read as backwards
  // as iced would (where the root element is on the top of the code).
  fn view(&self, ctx: &Context<'_>) -> Node {
    Node::row([
      Node::icon("utilities-system-monitor-symbolic").size(14.0),
      Node::text(format!("cpu {:.0}%", self.usage)).color(ctx.theme().text),
    ])
    .spacing(5.0)
  }
}

fn register(host: HostRef<'_>) {
  // Register as a component (duh)
  host.component::<Cpu>("example/cpu");
}
```

### Renderables

General-purpose unowned surfaces. Basically, instead the UI itself
owning the surface, it is dragged wherever and displayed in surfaces
that allow it to be socketed.

Renderable could have various application (currently only menus 
and desktop widgets, but that will expand in the future)..

```rust
pub struct Counter { count: u32 }

impl Renderable for Counter {
  // init
  fn new(_ctx: &Context<'_>) -> Self { Self { count: 0 } }

  // This helps situate your surface
  // better. If you're making a menu
  // for example, you might wanna wrap
  // it on a popup.
  fn settings(&self) -> RenderableSettings {
    RenderableSettings { wrap_popup: false, width: 220.0, height: 120.0 }
  }

  // Yepp, here it is... The actual UI.
  fn view(&self, ctx: &Context<'_>) -> Node {
    Node::column([
      Node::text(format!("{}", self.count)).size(32.0).color(ctx.theme().primary),
      Node::clickable(Node::text("reset")).action("example.counter.reset"),
    ])
    .spacing(8.0)
    .align(Align::Center)
  }

  // Now, depending on where the
  // renderable is situated. It could
  // listen to events, initiate a service,
  // etc etc.
  
  fn events(&self) -> RVec<EventFilter> {
    RVec::new()
  }

  fn initialize(&mut self, _ctx: &Context<'_>) {}

    
  fn update(&mut self, _ctx: &Context<'_>) -> ItemEffect {
    ItemEffect::None
  }
  // direct event access
  fn on_event(&mut self, _ctx: &Context<'_>, _event: &Event) -> ItemEffect {
    ItemEffect::None
  }

  // more about this on desktop items
  fn handle_message(&mut self, _ctx: &Context<'_>, _message: &Message) -> ItemEffect {
    ItemEffect::None
  }
}

fn register(host: HostRef<'_>) {
  host.renderable::<Counter>("example/counter");
}
```

### Desktop items

These are the actual real surfaces, where the ui can actually
own the surface, but also have more control over its event system
as well as the control flow.

```rust
pub struct Wallpaper { path: String }

impl DesktopItem for Wallpaper {
  // initialize
  fn new(_ctx: &Context<'_>) -> Self { Self { path: "/path/to/something".into() } }

  // The wayland layershell rules.
  fn settings(&self) -> DesktopSettings {
    DesktopSettings {
      layer: Layer::Background,
      anchor: Anchor::TOP | Anchor::RIGHT | Anchor::BOTTOM | Anchor::LEFT,
      ..DesktopSettings::default()
    }
  }

  // Aaaand, the UI
  fn view(&self, ctx: &Context<'_>) -> Node {
    Node::image(self.path)
  }
}

fn register(host: HostRef<'_>) {
  host.desktop_item::<Wallpaper>("example-wallpaper");
}
```


### Deployers

Deployers aren't really surfaces, but they do deploy desktop items on events. They're
useful for cases where your desktop item is not supposed to be initiated at startup, but
can be initated per events.

```rust
impl DeployableDesktopItem for DemoDeployer {
  fn new(_ctx: &Context<'_>) -> Self { Self }

  fn events(&self) -> RVec<EventFilter> {
    vec![EventFilter::Payload("some.deploy.nowplaying".into())].into()
  }

  fn deploy(&mut self, ctx: &Context<'_>, event: &Event) -> DeployResult {
    if let Event::Payload { payload, .. } = event {
      if let Some(args) = payload.as_this::<DeployArgs>() {
        // Deploy here
        return DeployResult::Deploy(vec![
          DeployedItem::new::<NowPlayingWidget>(ctx, args.id.as_str()),
        ]);
      }
    }
    
    DeployResult::None
  }
}

fn register(host: HostRef<'_>) {
  host.payload_parser(&["some.deploy."], |command, args| {
    Some(DeployArgs {
      kind: command.trim_start_matches("some.deploy.").to_owned(),
      id: args.get("id").unwrap_or("default").to_owned(),
    })
  });
  
  host.desktop_item_deployer::<DemoDeployer>("demo-deployer");
}
```

---
