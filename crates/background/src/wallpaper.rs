use std::{any::TypeId, collections::HashMap};

use iced::{
  Element, Length,
  widget::{container, image},
};
use iced_layershell::reexport::{
  Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use slowshell_config::{Config, ConfigParser};
use slowshell_core::{Store, listeners::ListenerAction};
use slowshell_desktop::{
  DesktopItem, EventFilter, ItemEffect, ItemMessage, MonitorScope, UpdateWhen, Visibility,
};

pub struct Wallpaper {
  file: Option<String>,
  paths: HashMap<String, String>,
  cache: HashMap<String, image::Handle>,
}

impl Wallpaper {
  pub fn new(config: &WallpaperConfig) -> Self {
    let mut cache = HashMap::new();
    let max_w = config.max_width.unwrap_or(1920);
    let max_h = config.max_height.unwrap_or(1080);

    let mut unique_paths = Vec::new();
    if let Some(file) = &config.file {
      unique_paths.push(file.clone());
    }
    if let Some(paths) = &config.paths {
      for path in paths.values() {
        if !unique_paths.contains(path) {
          unique_paths.push(path.clone());
        }
      }
    }

    for path in unique_paths {
      if let Some(handle) = load_optimized_handle(&path, max_w, max_h) {
        cache.insert(path, handle);
      }
    }

    Self {
      file: config.file.clone(),
      paths: config.paths.clone().unwrap_or_default(),
      cache,
    }
  }
}

fn load_optimized_handle(path: &str, max_width: u32, max_height: u32) -> Option<image::Handle> {
  let img = match ::image::open(path) {
    Ok(i) => i,
    Err(e) => {
      eprintln!("[wallpaper] failed to open image '{path}': {e}");
      return None;
    }
  };

  let (w, h) = (img.width(), img.height());
  let final_img = if w > max_width || h > max_height {
    img.resize(
      max_width,
      max_height,
      ::image::imageops::FilterType::Triangle,
    )
  } else {
    img
  };

  let rgba = final_img.to_rgba8();
  Some(image::Handle::from_rgba(
    rgba.width(),
    rgba.height(),
    rgba.into_raw(),
  ))
}

fn spawn_external_daemon(conf: &WallpaperConfig) {
  let backend = conf.backend.as_deref().unwrap_or("slowshell");
  match backend {
    "swaybg" => {
      let mut cmd = std::process::Command::new("swaybg");
      if let Some(paths) = &conf.paths {
        for (out, path) in paths {
          cmd.args(["-o", out, "-i", path, "-m", "fill"]);
        }
      }
      if let Some(file) = &conf.file {
        cmd.args(["-i", file, "-m", "fill"]);
      }
      if let Err(e) = cmd.spawn() {
        eprintln!("[wallpaper] failed to spawn swaybg: {e}");
      }
    }
    "swww" | "sww" => {
      if let Some(paths) = &conf.paths {
        for (out, path) in paths {
          if let Err(e) = std::process::Command::new("swww")
            .args(["img", "-o", out, path])
            .spawn()
          {
            eprintln!("[wallpaper] failed to run swww: {e}");
          }
        }
      } else if let Some(file) = &conf.file {
        if let Err(e) = std::process::Command::new("swww")
          .args(["img", file])
          .spawn()
        {
          eprintln!("[wallpaper] failed to run swww: {e}");
        }
      }
    }
    "hyprpaper" => {
      if let Some(file) = &conf.file {
        let _ = std::process::Command::new("hyprctl")
          .args(["hyprpaper", "preload", file])
          .spawn();
        let _ = std::process::Command::new("hyprctl")
          .args(["hyprpaper", "wallpaper", &format!(",{file}")])
          .spawn();
      }
      if let Some(paths) = &conf.paths {
        for (out, path) in paths {
          let _ = std::process::Command::new("hyprctl")
            .args(["hyprpaper", "preload", path])
            .spawn();
          let _ = std::process::Command::new("hyprctl")
            .args(["hyprpaper", "wallpaper", &format!("{out},{path}")])
            .spawn();
        }
      }
    }
    "custom" => {
      if let Some(template) = &conf.spawn_args {
        if let Some(paths) = &conf.paths {
          for (out, path) in paths {
            let cmd_str = template
              .replace("$PATH", path)
              .replace("$FILE", path)
              .replace("$OUTPUT", out)
              .replace("$MONITOR", out);
            spawn_shell_command(&cmd_str);
          }
        } else if let Some(file) = &conf.file {
          let cmd_str = template
            .replace("$PATH", file)
            .replace("$FILE", file)
            .replace("$OUTPUT", "")
            .replace("$MONITOR", "");
          spawn_shell_command(&cmd_str);
        }
      }
    }
    _ => {}
  }
}

fn spawn_shell_command(cmd_str: &str) {
  let parts: Vec<&str> = cmd_str.split_whitespace().collect();
  if let Some((prog, args)) = parts.split_first() {
    if let Err(e) = std::process::Command::new(prog).args(args).spawn() {
      eprintln!("[wallpaper] failed to spawn custom command '{prog}': {e}");
    }
  }
}

impl DesktopItem for Wallpaper {
  fn id(&self) -> &str {
    "wallpaper"
  }

  fn layer(&self, _: &Config, monitor: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
      layer: Layer::Background,
      anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
      exclusive_zone: Some(-1),
      size: None,
      margin: Some((0, 0, 0, 0)),
      keyboard_interactivity: KeyboardInteractivity::None,
      events_transparent: true,
      namespace: Some("slowshell-wallpaper".into()),
      output_option: OutputOption::OutputName(monitor.to_string()),
    }
  }

  fn visibility(&self) -> Visibility {
    Visibility::Visible
  }

  fn update_strategy(&self) -> UpdateWhen {
    UpdateWhen::OnDemand
  }

  fn monitor(&self, _: &Config) -> MonitorScope {
    MonitorScope::PerMonitor
  }

  fn init_events(&self) -> Vec<EventFilter> {
    vec![EventFilter::Named("config.reload".into())]
  }

  fn update(
    &mut self,
    config: &Config,
    _store: &mut Store,
    event: &ListenerAction,
  ) -> anyhow::Result<ItemEffect> {
    match event {
      ListenerAction::Named(n)
      | ListenerAction::Signal { name: n, .. }
      | ListenerAction::Timer { name: n, .. }
        if n.as_ref() == "config.reload" =>
      {
        if let Some(conf) = config.typed::<WallpaperConfig>() {
          let backend = conf.backend.as_deref().unwrap_or("slowshell");
          if backend != "slowshell" {
            spawn_external_daemon(conf);
          } else {
            *self = Wallpaper::new(conf);
          }
          return Ok(ItemEffect::Redraw);
        }
      }
      _ => {}
    }
    Ok(ItemEffect::None)
  }

  fn view<'a>(
    &'a self,
    config: &'a Config,
    _store: &'a Store,
    _id: iced_layershell::reexport::IcedId,
    monitor: &str,
  ) -> Element<'a, ItemMessage> {
    let fallback = config.theme.base;
    let path = self.paths.get(monitor).or(self.file.as_ref());
    let handle = path.and_then(|p| self.cache.get(p));

    if let Some(handle) = handle {
      container(
        image(handle.clone())
          .content_fit(iced::ContentFit::Cover)
          .width(Length::Fill)
          .height(Length::Fill),
      )
      .width(Length::Fill)
      .height(Length::Fill)
      .style(move |_theme: &iced::Theme| container::Style {
        background: Some(fallback.into()),
        ..Default::default()
      })
      .into()
    } else {
      container(iced::widget::Space::new())
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme: &iced::Theme| container::Style {
          background: Some(fallback.into()),
          ..Default::default()
        })
        .into()
    }
  }
}

#[derive(Clone, Default)]
pub struct WallpaperConfig {
  pub backend: Option<String>,
  pub file: Option<String>,
  pub paths: Option<HashMap<String, String>>,
  pub spawn_args: Option<String>,
  pub max_width: Option<u32>,
  pub max_height: Option<u32>,
}

slowshell_registry::register_resources!(
  config: Unknown(Box::new(ConfigParser {
    type_id: TypeId::of::<WallpaperConfig>(),
    de: |nodes| {
      let Some(node) = slowshell_config::find_node(nodes, "wallpaper") else {
        return Err(anyhow::anyhow!("missing \"wallpaper\""));
      };
      let paths = slowshell_config::child(node, "paths").map(|paths_node| {
        slowshell_config::node_children(paths_node)
          .into_iter()
          .map(|p| {
            (
              slowshell_config::node_name(p).to_string(),
              slowshell_config::str_arg(p, 0)
                .unwrap_or_default()
                .to_string(),
            )
          })
          .collect::<HashMap<_, _>>()
      });
      Ok(Some(Box::new(WallpaperConfig {
        backend: slowshell_config::child_str_owned(node, "backend"),
        file: slowshell_config::child_str_owned(node, "file"),
        paths,
        spawn_args: slowshell_config::child_str_owned(node, "spawn-args"),
        max_width: slowshell_config::child_i64(node, "max-width").map(|width| width as u32),
        max_height: slowshell_config::child_i64(node, "max-height").map(|height| height as u32),
      })))
    }
  })),
  app: Item(|config, _| {
    if let Some(conf) = config.typed::<WallpaperConfig>() {
      let backend = conf.backend.as_deref().unwrap_or("slowshell");
      if backend != "slowshell" {
        spawn_external_daemon(conf);
        return Ok(vec![]);
      }

      if conf.file.is_some() || conf.paths.is_some() {
        return Ok(vec![Box::new(Wallpaper::new(conf)) as Box<dyn DesktopItem + Send>]);
      }
    }
    Ok(vec![])
  }),
);
