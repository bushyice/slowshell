+++
title = "Extensible modules"
description = "The parts of slowshell extensible with plugins."
weight = 4
+++

[[Surfaces]] are the obvious extensible parts, but they aren't the only ones. [[Slowshell]]
has a handful of deeper sockets you can plug into... its command bus, compositor
backends, the launcher, etc etc.

## Payloads and commands

Payloads are slowshell's command bus: a named action, optionally carrying a typed value.

A single fixed command is just a type:
```rust
struct Greet;

impl Payload for Greet {
  fn new(_ctx: &Context<'_>) -> Self { Self }

  fn invoke(&mut self, ctx: &Context<'_>, args: &Args, _payload: ROption<&PayloadBox>) {
    let name = args.get("name").unwrap_or("world");
    ctx.notify(&Notification::new("Hello", format!("hello, {name}!")));
  }
}

fn register(host: HostRef<'_>) {
  host.payload::<Greet>("example.greet");
}
```

Which you can then fire from IPC(or dispatch) as such:
```sh
slowshell ipc exec example.greet name=you
```

Noe let's say you need a dynamic command such as `a.b` where `.c` is handled
within the same handler, you could do this:
```rust
host.payload_parser(&["demo.deploy."], |command, args| {
  Some(DeployArgs {
    kind: command.trim_start_matches("demo.deploy.").to_owned(),
    id: args.get("id").unwrap_or("default").to_owned(),
  })
});
```

And ofcourse, if that command should be triggerable from the [[Spotlight#Modes|spotlight command palette]],
You can register it as such:
```rust
host.command_with("example.greet", "Greet Someone", "Send a friendly notification");
```

## Compositors

The Compositor state is basically integration of slowshell with the window compositor
that it's running in. This is one of the integral slowshell states. 

```rust
impl Compositor for MyCompositor {
  fn new(_ctx: &Context<'_>) -> Self { Self { state: CompositorState::default() } }

  fn is_active(&self) -> bool {
    // Pick me only when my session is around.
    std::env::var("MY_COMPOSITOR_SOCKET").is_ok()
  }

  fn initialize(&mut self, ctx: &Context<'_>) {
    let fd = connect_to_my_compositor();
    ctx.host().compositor_register_fd(fd, 0);
  }

  fn update_state(&mut self, ctx: &Context<'_>) {
    drain_socket(&mut self.state);
    ctx.set_compositor_state(self.state.clone());
  }

  fn send_command(&mut self, _ctx: &Context<'_>, command: u32, arg: i32) {
    match command {
      0 => focus_workspace(arg as u8),
      1 => focus_window(arg as u64),
      _ => {}
    }
  }
}

fn register(host: HostRef<'_>) {
  host.compositor::<MyCompositor>("my-compositor");
}
```

## Spotlight modes

```rust
impl Spotlight for Notes {
  fn new(_ctx: &Context<'_>) -> Self { Self }

  fn generate(&mut self, _ctx: &Context<'_>, query: &str) -> Vec<SpotlightResult> {
    find_notes(query)
      .map(|note| {
        SpotlightResult::default()
          .icon("note-symbolic")
          .title(note.title)
          .action_label("Open")
      })
      .collect()
  }

  fn activate(&mut self, _ctx: &Context<'_>, index: usize) { open_note(index); }

  fn display_styles() -> &'static [DisplayStyle] {
    &[DisplayStyle::List, DisplayStyle::Grid]
  }
}

fn register(host: HostRef<'_>) {
  host.spotlight::<Notes>("notes");
}
```

```sh
slowshell ipc spotlight.toggle notes
slowshell list spotlights
```

## Quick settings

```rust
impl QuickSettings for Vpn {
  fn new(_ctx: &Context<'_>) -> Self { Self }

  fn tiles(&self, _ctx: &Context<'_>) -> Vec<QuickSettingsTile> {
    let connected = vpn_is_up();
    vec![
      QuickSettingsTile::new("example-vpn", "network-vpn-symbolic", "VPN")
        .status(if connected { "Connected" } else { "Off" })
        .active(connected)
        .order(40)
        .action(if connected { "example.vpn.disconnect" } else { "example.vpn.connect" }),
    ]
  }

  // tiles can expand into more tiles.
  fn children(&self, _ctx: &Context<'_>, id: &str) -> Vec<QuickSettingsTile> {
    if id == "example-vpn" {
      vec![QuickSettingsTile::new("example-vpn-server", "network-server-symbolic", "Server")
        .action("example.vpn.pick")]
    } else {
      Vec::new()
    }
  }
}

fn register(host: HostRef<'_>) {
  host.quick_settings::<Vpn>("example-vpn");
}
```

---
