#[cfg(feature = "panels")]
use slowshell_components::{ComponentRegistration, Components};
use slowshell_config::Config;
use slowshell_core::Store;
#[cfg(feature = "panels")]
use slowshell_panels::PanelPositions;
use slowshell_registry::{GlobalRegistry, ResourceRegistration};
use slowshell_widgets::Renderables;

pub fn styles() -> miette::Result<()> {
  let loaded = crate::daemon::load();

  let css = loaded.config.stylesheet();
  print!("{css}");
  if !css.ends_with('\n') {
    println!();
  }

  Ok(())
}

#[cfg(feature = "plugins")]
pub fn plugins() -> miette::Result<()> {
  let loaded = crate::daemon::load();

  let mut infos = slowshell_plugin_host::loaded_plugins();
  infos.sort_by(|a, b| a.id.cmp(&b.id));

  for info in infos {
    if info.requires.is_empty() {
      println!("{} {} {}", info.id, info.version, info.path.display());
    } else {
      println!(
        "{} {} {} requires {}",
        info.id,
        info.version,
        info.path.display(),
        info.requires.join(", ")
      );
    }
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

  #[cfg(feature = "plugins")]
  names.extend(slowshell_plugin_host::renderable_names());
  names.sort();
  names.dedup();

  for name in names {
    println!("{name}");
  }

  Ok(())
}

#[cfg(feature = "panels")]
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

#[cfg(feature = "spotlight")]
pub fn spotlights() -> miette::Result<()> {
  let loaded = crate::daemon::load();

  let mut names: Vec<String> = slowshell_spotlight::SpotlightModes::default()
    .modes
    .keys()
    .map(|key| key.to_string())
    .collect();

  #[cfg(feature = "plugins")]
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
    ("base", config.palette().base),
    ("crust", config.palette().crust),
    ("mantle", config.palette().mantle),
    ("primary", config.palette().primary),
    ("secondary", config.palette().secondary),
    ("green", config.palette().green),
    ("red", config.palette().red),
    ("blue", config.palette().blue),
    ("yellow", config.palette().yellow),
    ("orange", config.palette().orange),
    ("text", config.palette().text),
    ("subtext", config.palette().subtext),
    ("overlay", config.palette().overlay),
  ] {
    println!("  {name} = {}", hex(color));
  }
  println!("}}");

  println!();
  let css = config.stylesheet();
  print!("{css}");
  if !css.ends_with('\n') {
    println!();
  }

  Ok(())
}

fn context_store(registry: &mut GlobalRegistry, config: &Config) -> Store {
  let mut store = Store::new();

  #[cfg(feature = "panels")]
  store.insert(PanelPositions::default());

  let mut renderables = Renderables::default();
  for resource in registry.inside("renderables") {
    if let Some((name, renderable)) = resource.as_renderable(config, &mut store) {
      renderables.insert(name.into(), renderable);
    }
  }
  store.insert(renderables);

  #[cfg(feature = "panels")]
  {
    let mut components = Components::default();
    slowshell_components::register_all(&mut components);
    crate::app::register_plugin_components(registry, &mut components);
    store.insert(components);
  }

  store
}

fn hex(color: iced::Color) -> String {
  let [r, g, b, a] = color.into_rgba8();

  if a == 255 {
    format!("#{r:02x}{g:02x}{b:02x}")
  } else {
    format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
  }
}
