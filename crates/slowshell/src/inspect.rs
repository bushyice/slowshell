use slowshell_components::{ComponentRegistration, Components};
use slowshell_config::{
  Config,
  style::{ColorValue, StyleValue},
};
use slowshell_core::Store;
use slowshell_panels::PanelPositions;
use slowshell_registry::{GlobalRegistry, ResourceRegistration};
use slowshell_widgets::Renderables;

pub fn styles() -> miette::Result<()> {
  let loaded = crate::daemon::load();

  let mut names: Vec<String> = loaded
    .config
    .styles()
    .keys()
    .map(|key| key.to_string())
    .collect();
  names.sort();

  // TODO: List the styles themselves
  for name in names {
    println!("{name}");
  }

  Ok(())
}

pub fn plugins() -> miette::Result<()> {
  let loaded = crate::daemon::load();

  let mut infos = slowshell_plugin_host::loaded_plugins();
  infos.sort_by(|a, b| a.id.cmp(&b.id));

  for info in infos {
    println!("{} {} {}", info.id, info.version, info.path.display());
  }

  drop(loaded);

  Ok(())
}

pub fn renderables() -> miette::Result<()> {
  let mut loaded = crate::daemon::load();

  let mut names: Vec<String> = loaded
    .registry
    .inside("renderables")
    .into_iter()
    .filter_map(|resource| match resource {
      ResourceRegistration::Renderable { name, .. } => Some(name),
      _ => None,
    })
    .collect();

  names.extend(slowshell_plugin_host::renderable_names());
  names.sort();
  names.dedup();

  for name in names {
    println!("{name}");
  }

  Ok(())
}

pub fn components() -> miette::Result<()> {
  let mut loaded = crate::daemon::load();

  let mut components = Components::default();
  slowshell_components::register_all(&mut components);

  let mut names: Vec<String> = components.keys().map(|key| key.to_string()).collect();

  for resource in loaded.registry.inside("components") {
    if let ResourceRegistration::Unknown(registration) = resource
      && let Ok(registration) = registration.downcast::<ComponentRegistration>()
    {
      names.push(registration.name.to_string());
    }
  }

  names.sort();
  names.dedup();

  for name in names {
    println!("{name}");
  }

  Ok(())
}

pub fn items() -> miette::Result<()> {
  let mut loaded = crate::daemon::load();
  let store = context_store(&mut loaded.registry, &loaded.config);

  for resource in loaded.registry.inside("app") {
    if let ResourceRegistration::Item(create) = resource {
      for item in (create)(&loaded.config, &store)? {
        println!("{}", item.id());
      }
    }
  }

  Ok(())
}

pub fn spotlights() -> miette::Result<()> {
  let loaded = crate::daemon::load();

  let mut names: Vec<String> = slowshell_spotlight::SpotlightModes::default()
    .modes
    .keys()
    .map(|key| key.to_string())
    .collect();

  names.extend(slowshell_plugin_host::spotlight_names());
  names.sort();
  names.dedup();

  for name in names {
    println!("{name}");
  }

  drop(loaded);

  Ok(())
}

pub fn config_current() -> miette::Result<()> {
  let loaded = crate::daemon::load();

  match loaded.config.current_path() {
    Some(path) => println!("{}", path.display()),
    None => println!("no configuration file in use"),
  }

  Ok(())
}

pub fn config_validate() -> miette::Result<()> {
  let loaded = crate::daemon::load();

  loaded.config.validate()?;

  println!("Configuration is valid.");
  Ok(())
}

pub fn config_show() -> miette::Result<()> {
  let loaded = crate::daemon::load();
  let config = &loaded.config;

  println!("font = {}", config.font_name());
  println!("tick-interval = {}", config.tick_interval);
  println!(
    "wgpu-backend = {}",
    config.wgpu_backend.as_deref().unwrap_or("")
  );
  println!(
    "icon-theme = {}",
    config.icon_theme.as_deref().unwrap_or("")
  );
  println!();

  println!("theme {{");
  for (name, color) in [
    ("base", config.theme.base),
    ("crust", config.theme.crust),
    ("mantle", config.theme.mantle),
    ("primary", config.theme.primary),
    ("secondary", config.theme.secondary),
    ("green", config.theme.green),
    ("red", config.theme.red),
    ("blue", config.theme.blue),
    ("yellow", config.theme.yellow),
    ("orange", config.theme.orange),
    ("text", config.theme.text),
    ("subtext", config.theme.subtext),
    ("overlay", config.theme.overlay),
  ] {
    println!("  {name} = {}", hex(color));
  }
  println!("}}");

  let mut styles: Vec<(String, &slowshell_config::style::Style)> = config
    .styles()
    .iter()
    .map(|(name, style)| (name.to_string(), style))
    .collect();
  styles.sort_by(|a, b| a.0.cmp(&b.0));

  for (name, style) in styles {
    let mut entries: Vec<(&str, &StyleValue)> = style.entries().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));

    println!();
    println!("style {name} {{");
    for (key, value) in entries {
      println!("  {key} = {}", style_value(value));
    }
    println!("}}");
  }

  Ok(())
}

fn context_store(registry: &mut GlobalRegistry, config: &Config) -> Store {
  let mut store = Store::new();
  store.insert(PanelPositions::default());

  let mut renderables = Renderables::default();
  for resource in registry.inside("renderables") {
    if let Some((name, renderable)) = resource.as_renderable(config, &mut store) {
      renderables.insert(name.into(), renderable);
    }
  }
  store.insert(renderables);

  let mut components = Components::default();
  slowshell_components::register_all(&mut components);
  crate::app::register_plugin_components(registry, &mut components);
  store.insert(components);

  store
}

fn style_value(value: &StyleValue) -> String {
  match value {
    StyleValue::String(text) => text.clone(),
    StyleValue::Integer(int) => int.to_string(),
    StyleValue::Float(float) => float.to_string(),
    StyleValue::Boolean(boolean) => boolean.to_string(),
    StyleValue::Color(ColorValue::Transparent) => "transparent".into(),
    StyleValue::Color(ColorValue::Theme(name)) => name.clone(),
    StyleValue::Color(ColorValue::Color(hex)) => hex.clone(),
  }
}

fn hex(color: iced::Color) -> String {
  let [r, g, b, a] = color.into_rgba8();

  if a == 255 {
    format!("#{r:02x}{g:02x}{b:02x}")
  } else {
    format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
  }
}
