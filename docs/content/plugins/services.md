+++
title = "Services"
description = "Background jobs."
weight = 6
+++

[[Slowshell]] uses small little services within itself to do background work, and what
those services do is: listen to some change -> wake the shell up for updates -> keep
listening silently.

```rust
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct Weather {
  temp: f32,
}

pub struct WeatherService {
  state: Arc<Mutex<Weather>>,
}

impl Service for WeatherService {
  fn new(ctx: &Context<'_>) -> Self {
    let state = Arc::new(Mutex::new(Weather::default()));
    ctx.store().insert(state.clone());   // consumers read this
    Self { state }
  }

  fn run(&mut self, ctx: &ServiceContext<'_>) {
    while !ctx.stopping() {
      if let Some(temp) = fetch_temperature() {
        self.state.lock().unwrap().temp = temp;
        ctx.changed();                    // wake subscribed surfaces
      }
      if !ctx.sleep(60_000) {
        break;                            // stop requested
      }
    }
  }
}

fn register(host: HostRef<'_>) {
  host.service::<WeatherService>("example/weather");          // lazy
  // host.service_autostart::<WeatherService>("example/weather"); // at startup
}
```

## Starting services

You can start services from the [[Host and Context#Services|host]] lazily when they are needed:
```rust
impl Component for WeatherComponent {
  fn watch(&self, ...) {
    if !ctx.service_running("example/weather") {
      ctx.service_start("example/weather");
    }
  }
}
```

## Built-in services

These built-in(unless you did opt-out while [[Building from source]]) are always
available; And they're what provide most global common shell-service states.

| Name | Service |
|---|---|
| `network` | NetworkManager watcher |
| `audio` | PipeWire/wpctl volume + MPRIS |
| `bluetooth` | BlueZ adapter and devices |
| `power` | Battery, brightness, power profiles |
| `system` | CPU/memory/temperature sampler |
| `tray` | Status notifier items |
| `notifications` | The notification daemon (unless disabled in config) |

---
