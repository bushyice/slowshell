use std::{
  any::TypeId,
  collections::HashMap,
  sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
  },
};

use iced::{
  Element, Length,
  widget::{container, image},
};
use iced_exwlshell::reexport::{
  Anchor, BlurOption, KeyboardInteractivity, Layer, LayerSize, NewLayerShellSettings, OutputOption,
};
use slowshell_config::{Config, ConfigParser};
use slowshell_core::{
  Store,
  listeners::{FdHandle, ListenerAction},
};
use slowshell_desktop::{
  DesktopItem, EventFilter, ItemEffect, ItemMessage, MonitorScope, UpdateWhen, Visibility,
};

type SharedCache = Arc<Mutex<HashMap<String, image::Handle>>>;

struct PendingLoad(Arc<AtomicUsize>);

impl Drop for PendingLoad {
  fn drop(&mut self) {
    self.0.fetch_sub(1, Ordering::SeqCst);
  }
}

pub struct Wallpaper {
  file: Option<String>,
  paths: HashMap<String, String>,
  cache: SharedCache,
  pending: Arc<AtomicUsize>,
}

impl Wallpaper {
  pub fn new(config: &WallpaperConfig) -> Self {
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

    let cache: SharedCache = Arc::new(Mutex::new(HashMap::new()));
    let pending = Arc::new(AtomicUsize::new(unique_paths.len()));

    for path in unique_paths {
      let thread_cache = Arc::clone(&cache);
      let thread_pending = Arc::clone(&pending);
      let spawned = std::thread::Builder::new()
        .name("slowshell-wallpaper".into())
        .spawn(move || {
          let _pending = PendingLoad(thread_pending);
          if let Some(handle) = load_optimized_handle(&path, max_w, max_h) {
            thread_cache.lock().unwrap().insert(path, handle);
          }
        });

      if spawned.is_err() {
        pending.fetch_sub(1, Ordering::SeqCst);
      }
    }

    Self {
      file: config.file.clone(),
      paths: config.paths.clone().unwrap_or_default(),
      cache,
      pending,
    }
  }

  fn is_loading(&self) -> bool {
    self.pending.load(Ordering::SeqCst) > 0
  }

  fn events(&self) -> Vec<EventFilter> {
    let mut events = vec![EventFilter::Named("config.reload".into())];
    if self.is_loading() {
      events.push(EventFilter::Frame);
    }
    events
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
    "swaybg" => spawn_swaybg(conf),
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

fn spawn_signature(conf: &WallpaperConfig) -> String {
  let mut signature = String::new();
  signature.push_str(conf.backend.as_deref().unwrap_or("slowshell"));

  if let Some(file) = &conf.file {
    signature.push('\0');
    signature.push_str(file);
  }
  if let Some(paths) = &conf.paths {
    let mut entries: Vec<(&String, &String)> = paths.iter().collect();
    entries.sort();
    for (output, path) in entries {
      signature.push('\0');
      signature.push_str(output);
      signature.push('=');
      signature.push_str(path);
    }
  }
  if let Some(args) = &conf.spawn_args {
    signature.push('\0');
    signature.push_str(args);
  }

  signature.replace('\n', " ")
}

fn wallpaper_pid_path() -> std::path::PathBuf {
  std::env::var("XDG_RUNTIME_DIR")
    .map(std::path::PathBuf::from)
    .map(|dir| dir.join("slowshell-wallpaper.pid"))
    .unwrap_or_else(|_| std::path::PathBuf::from("/tmp/slowshell-wallpaper.pid"))
}

fn read_wallpaper_pidfile(path: &std::path::Path) -> Option<(String, u32)> {
  let content = std::fs::read_to_string(path).ok()?;
  let mut lines = content.lines();
  let signature = lines.next()?.to_string();
  let pid = lines.next()?.trim().parse().ok()?;
  Some((signature, pid))
}

fn process_alive(pid: u32) -> bool {
  unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

fn process_name(pid: u32) -> Option<String> {
  std::fs::read_to_string(format!("/proc/{pid}/comm"))
    .ok()
    .map(|name| name.trim().to_string())
}

fn spawn_swaybg(conf: &WallpaperConfig) {
  let signature = spawn_signature(conf);
  let pid_path = wallpaper_pid_path();

  if let Some((old_signature, pid)) = read_wallpaper_pidfile(&pid_path)
    && process_alive(pid)
    && process_name(pid).as_deref() == Some("swaybg")
  {
    if old_signature == signature {
      return;
    }

    unsafe {
      libc::kill(pid as libc::pid_t, libc::SIGTERM);
    }
  }

  let mut cmd = std::process::Command::new("swaybg");
  if let Some(paths) = &conf.paths {
    for (out, path) in paths {
      cmd.args(["-o", out, "-i", path, "-m", "fill"]);
    }
  }
  if let Some(file) = &conf.file {
    cmd.args(["-i", file, "-m", "fill"]);
  }

  match cmd.spawn() {
    Ok(child) => {
      let _ = std::fs::write(&pid_path, format!("{signature}\n{}\n", child.id()));
    }
    Err(e) => eprintln!("[wallpaper] failed to spawn swaybg: {e}"),
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
      size: LayerSize::FILL,
      margin: Some((0, 0, 0, 0)),
      keyboard_interactivity: KeyboardInteractivity::None,
      events_transparent: true,
      namespace: Some("slowshell-wallpaper".into()),
      output_option: OutputOption::OutputName(monitor.to_string()),
      blur_option: BlurOption::None,
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
    self.events()
  }

  fn update(
    &mut self,
    config: &Config,
    _store: &mut Store,
    event: &ListenerAction,
  ) -> miette::Result<ItemEffect> {
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
          return Ok(ItemEffect::Subscribe(self.events()));
        }
      }
      ListenerAction::Frame => {
        if self.is_loading() {
          return Ok(ItemEffect::None);
        }
        return Ok(ItemEffect::Subscribe(self.events()));
      }
      _ => {}
    }
    Ok(ItemEffect::None)
  }

  fn view<'a>(
    &'a self,
    config: &'a Config,
    _store: &'a Store,
    _id: iced_exwlshell::reexport::IcedId,
    monitor: &str,
  ) -> Element<'a, ItemMessage> {
    let fallback = config.palette().base;
    let path = self.paths.get(monitor).or(self.file.as_ref());
    let handle = path.and_then(|p| self.cache.lock().unwrap().get(p).cloned());

    if let Some(handle) = handle {
      container(
        image(handle)
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
        return Ok(None);
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
  app: Item(|config, store| {
    if let Some(conf) = config.typed::<WallpaperConfig>() {
      let backend = conf.backend.as_deref().unwrap_or("slowshell");
      if backend != "slowshell" {
        if store.borrow::<FdHandle>().is_some() {
          spawn_external_daemon(conf);
        }
        return Ok(vec![]);
      }

      if conf.file.is_some() || conf.paths.is_some() {
        return Ok(vec![Box::new(Wallpaper::new(conf)) as Box<dyn DesktopItem + Send>]);
      }
    }
    Ok(vec![])
  }),
);
