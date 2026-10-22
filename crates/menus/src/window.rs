use std::{
  sync::{Arc, Mutex, OnceLock},
  time::{Duration, Instant},
};

use iced::{
  Alignment, Element, Length,
  widget::{Space, column, container, row, text::Wrapping},
};
use iced_exwlshell::reexport::IcedId;
use slowshell_commons::desktop::DesktopEntries;
use slowshell_compositor::CompositorStore;
use slowshell_config::Config;
use slowshell_core::{
  Store, Window,
  listeners::{CompositorEvent, FdHandle, ListenerAction},
  message::{EventFilter, ItemEffect, ItemMessage},
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{
  Backdrop, ConfirmDialogProps, Icon, Renderable, SizedPopup, clickable, hover_row,
  render_confirm_dialog, separator,
};

const SAMPLE_TICK: &str = "menus/window.tick";
const WIDTH: f32 = 280.0;
const PROMPT_KILL: usize = 0;
const CANCEL: usize = 1;

pub struct WindowRenderable {
  target: Arc<Mutex<Option<u64>>>,
  confirming: Arc<Mutex<bool>>,
  timer_fd: Arc<Mutex<Option<i32>>>,
  sample: Arc<Mutex<Option<ProcSample>>>,
}

impl Default for WindowRenderable {
  fn default() -> Self {
    Self {
      target: Arc::new(Mutex::new(None)),
      confirming: Arc::new(Mutex::new(false)),
      timer_fd: Arc::new(Mutex::new(None)),
      sample: Arc::new(Mutex::new(None)),
    }
  }
}

#[derive(Debug, Clone, Copy)]
struct ProcSample {
  ticks: u64,
  at: Instant,
  memory: u64,
  cpu: f32,
}

fn ticks_per_second() -> f64 {
  static TICKS: OnceLock<f64> = OnceLock::new();

  *TICKS.get_or_init(|| {
    let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };

    if ticks > 0 { ticks as f64 } else { 100.0 }
  })
}

fn read_process(pid: i32) -> Option<(u64, u64)> {
  let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
  let fields: Vec<&str> = stat.rsplit_once(')')?.1.split_whitespace().collect();
  let utime: u64 = fields.get(11)?.parse().ok()?;
  let stime: u64 = fields.get(12)?.parse().ok()?;

  let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
  let resident_kib: u64 = status
    .lines()
    .find_map(|line| line.strip_prefix("VmRSS:"))?
    .trim()
    .strip_suffix("kB")?
    .trim()
    .parse()
    .ok()?;

  Some((resident_kib * 1024, utime + stime))
}

fn sample_process(sample: &mut Option<ProcSample>, pid: i32) {
  let Some((memory, ticks)) = read_process(pid) else {
    *sample = None;
    return;
  };

  let now = Instant::now();

  let cpu = sample
    .map(|previous| {
      let elapsed = now.duration_since(previous.at).as_secs_f64();
      let delta = ticks.saturating_sub(previous.ticks) as f64;

      (delta / ticks_per_second() / elapsed) as f32
    })
    .unwrap_or(0.0);

  *sample = Some(ProcSample {
    ticks,
    at: now,
    memory,
    cpu: cpu.clamp(0.0, 1.0),
  });
}

fn format_memory(bytes: u64) -> String {
  const KB: f64 = 1024.0;
  const MB: f64 = KB * 1024.0;
  const GB: f64 = MB * 1024.0;

  let bytes = bytes as f64;

  if bytes >= GB {
    format!("{:.1}gb", bytes / GB)
  } else if bytes >= MB {
    format!("{:.0}mb", bytes / MB)
  } else if bytes >= KB {
    format!("{:.0}kb", bytes / KB)
  } else {
    format!("{bytes:.0}b")
  }
}

fn format_cpu(cpu: f32) -> String {
  format!("{}%", (cpu.clamp(0.0, 1.0) * 100.0).round() as u32)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowMessage {
  PromptKill,
  Cancel,
}

impl WindowMessage {
  fn code(self) -> usize {
    match self {
      WindowMessage::PromptKill => PROMPT_KILL,
      WindowMessage::Cancel => CANCEL,
    }
  }

  fn message(self, id: IcedId) -> ItemMessage {
    ItemMessage::Effect(id, ItemEffect::Custom([40, self.code(), 0, 0]))
  }
}

fn active_window_id(store: &Store) -> Option<u64> {
  store
    .borrow::<CompositorStore>()
    .and_then(|c| c.state().ok())
    .and_then(|s| s.active_window.as_ref().map(|window| window.id))
    .or_else(|| slowshell_commons::window::last_active_window().get().map(|window| window.id))
}

fn window_by_id(store: &Store, window_id: u64) -> Option<Window> {
  let state = store.borrow::<CompositorStore>()?.state().ok()?;

  state
    .active_windows
    .iter()
    .find(|window| window.id == window_id)
    .cloned()
}

impl Renderable for WindowRenderable {
  fn events(&self, _store: &Store) -> Vec<EventFilter> {
    vec![
      EventFilter::Named(SAMPLE_TICK.into()),
      EventFilter::Compositor(CompositorEvent::WINDOWS | CompositorEvent::FOCUS),
    ]
  }

  fn initialize(&self, _config: &Config, store: &mut Store) -> Box<dyn std::any::Any + Send> {
    if let Ok(mut confirming) = self.confirming.lock() {
      *confirming = false;
    }
    if let Ok(mut target) = self.target.lock() {
      *target = active_window_id(store);
    }

    if let Ok(mut sample) = self.sample.lock() {
      *sample = None;

      let pid = self
        .target
        .lock()
        .ok()
        .and_then(|target| *target)
        .and_then(|window_id| window_by_id(store, window_id))
        .and_then(|window| window.pid());

      if let Some(pid) = pid {
        sample_process(&mut sample, pid);
      }
    }

    if let Ok(mut timer) = self.timer_fd.lock()
      && timer.is_none()
      && let Some(handle) = store.borrow::<FdHandle>()
      && let Ok(fd) = handle.set_named_interval(Duration::from_secs(1), SAMPLE_TICK)
    {
      *timer = Some(fd);
    }

    Box::new(())
  }

  fn close(&mut self, store: &mut Store) {
    if let Ok(mut confirming) = self.confirming.lock() {
      *confirming = false;
    }
    if let Ok(mut target) = self.target.lock() {
      *target = None;
    }
    if let Ok(mut sample) = self.sample.lock() {
      *sample = None;
    }
    if let (Ok(Some(fd)), Some(handle)) = (
      self.timer_fd.lock().map(|mut fd| fd.take()),
      store.borrow::<FdHandle>(),
    ) {
      handle.stop_timer(fd);
    }
  }

  fn on_event(
    &mut self,
    _config: &Config,
    store: &mut Store,
    event: &ListenerAction,
  ) -> Option<ItemEffect> {
    match event {
      ListenerAction::Timer { name, .. } if name.as_ref() == SAMPLE_TICK => {
        let pid = self
          .target
          .lock()
          .ok()
          .and_then(|target| *target)
          .and_then(|window_id| window_by_id(store, window_id))
          .and_then(|window| window.pid());

        if let (Ok(mut sample), Some(pid)) = (self.sample.lock(), pid) {
          sample_process(&mut sample, pid);
        }

        Some(ItemEffect::Redraw)
      }
      ListenerAction::UpdateCompositor(_) => {
        let window_id = self.target.lock().ok().and_then(|target| *target)?;

        match window_by_id(store, window_id) {
          Some(_) => Some(ItemEffect::Redraw),
          None => Some(ItemEffect::Hide),
        }
      }
      _ => None,
    }
  }

  fn handle_message(&mut self, message: &ItemMessage) -> Option<ItemEffect> {
    let ItemMessage::Effect(_, ItemEffect::Custom([40, kind, 0, 0])) = message else {
      return None;
    };

    let Ok(mut confirming) = self.confirming.lock() else {
      return None;
    };

    match *kind {
      kind if kind == WindowMessage::PromptKill.code() => {
        *confirming = true;
        Some(ItemEffect::Redraw)
      }
      kind if kind == WindowMessage::Cancel.code() => {
        *confirming = false;
        Some(ItemEffect::Redraw)
      }
      _ => None,
    }
  }

  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    data: &'a dyn std::any::Any,
  ) -> Element<'a, ItemMessage> {
    let settings = data.downcast_ref::<PopupSettings>().unwrap();

    let window = {
      let target = self.target.lock().ok().and_then(|target| *target);
      target.and_then(|window_id| window_by_id(store, window_id))
    };

    let Some(window) = window else {
      return Space::new().into();
    };

    let confirming = self.confirming.lock().map(|flag| *flag).unwrap_or(false);
    let sample = self.sample.lock().ok().and_then(|sample| *sample);

    let body: Element<'a, ItemMessage> = if confirming {
      render_confirm_dialog(
        ConfirmDialogProps {
          title: "Force kill",
          message: "Kill the application? Unsaved work will be lost and the \
                    whole process is terminated.",
          icon: Some("process-stop-symbolic"),
          confirm_label: "Force Kill",
          cancel_label: "Cancel",
          is_danger: true,
          on_confirm: ItemMessage::EffectAction(
            id,
            ItemEffect::Hide,
            ListenerAction::KillWindow(window.id),
          ),
          on_cancel: WindowMessage::Cancel.message(id),
        },
        config,
      )
      .into()
    } else {
      let app_name = store
        .borrow::<DesktopEntries>()
        .map(|entries| entries.resolve_name(&window.class).to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| {
          if window.class.trim().is_empty() {
            "Unknown".to_string()
          } else {
            window.class.clone()
          }
        });

      let mut details = column![
        info_row(config, "application-default-symbolic", "App", app_name),
        info_row(
          config,
          "utilities-terminal-symbolic",
          "PID",
          window
            .pid()
            .map(|pid| pid.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        ),
      ]
      .spacing(4)
      .width(Length::Fill);

      if let Some((width, height)) = window.size() {
        details = details.push(info_row(
          config,
          "view-fullscreen-symbolic",
          "Size",
          format!("{width} × {height}"),
        ));
      }

      let info = column![
        config
          .text(window.title.clone(), &["menu", "text-header"])
          .wrapping(Wrapping::Word),
        details,
        process_usage(config, sample),
      ]
      .spacing(config.sizing(&["menu"]).gap)
      .width(Length::Fill);

      let actions = column![
        action_row(
          config,
          "window-close-symbolic",
          "Close",
          false,
          ItemMessage::EffectAction(id, ItemEffect::Hide, ListenerAction::CloseWindow(window.id),),
        ),
        action_row(
          config,
          "process-stop-symbolic",
          "Force Kill",
          true,
          WindowMessage::PromptKill.message(id),
        ),
      ]
      .spacing(config.sizing(&["menu", "list"]).gap)
      .width(Length::Fill);

      let divider: Element<'a, ItemMessage> = separator()
        .color(
          config
            .color(&["menu", "separator"], "color")
            .unwrap_or(config.palette().base),
        )
        .into();

      column![info, divider, actions]
        .spacing(config.sizing(&["menu"]).gap)
        .width(Length::Fill)
        .into()
    };

    let inner_padding = config.sizing(&["menu", "inner"]).padding();
    let popup_style = config.container_cached(&["menu"]);

    SizedPopup::new(container(
      container(body)
        .padding(inner_padding)
        .style(popup_style)
        .width(Length::Fixed(WIDTH)),
    ))
    .with_width(WIDTH)
    .with_backdrop(Backdrop::transparent().with_close_on_click(true))
    .with_position(
      settings.position.0.get(store),
      settings.position.1.get(store),
    )
    .with_panel_edge(settings.panel_edge)
    .with_panel_insets(store, settings.monitor.as_deref())
    .with_on_close(ItemMessage::Effect(id, ItemEffect::Hide))
    .into()
  }
}

fn info_row<'a>(
  config: &'a Config,
  icon_name: &'static str,
  label: &'static str,
  value: String,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();

  let icon = Icon::new(icon_name).size(12).color(theme.primary);

  row![
    row![
      icon,
      config.text(label, &["menu", "text-status", "text-faded"])
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .width(Length::Fixed(80.0)),
    config.text(value, &["menu", "text-name"]),
  ]
  .align_y(Alignment::Center)
  .spacing(8)
  .width(Length::Fill)
  .into()
}

fn process_usage<'a>(config: &'a Config, sample: Option<ProcSample>) -> Element<'a, ItemMessage> {
  let (memory, cpu) = match sample {
    Some(sample) => (format_memory(sample.memory), format_cpu(sample.cpu)),
    None => ("unknown".to_string(), "unknown".to_string()),
  };

  row![
    stat_cell(config, "mem", memory),
    stat_cell(config, "cpu", cpu),
  ]
  .spacing(8.)
  .align_y(Alignment::Center)
  .width(Length::Fill)
  .into()
}

fn stat_cell<'a>(
  config: &'a Config,
  label: &'static str,
  value: String,
) -> Element<'a, ItemMessage> {
  let content = column![
    config.text(label, &["menu", "text-status", "text-faded"]),
    config.text(value, &["menu", "text-name"]),
  ]
  .spacing(2)
  .align_x(Alignment::Center);

  container(content)
    .padding(config.sizing(&["menu", "badge"]).padding())
    .style(config.container_cached(&["menu", "badge"]))
    .width(Length::Fill)
    .align_x(Alignment::Center)
    .into()
}

fn action_row<'a>(
  config: &'a Config,
  icon_name: &'static str,
  label: &'static str,
  danger: bool,
  message: ItemMessage,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();

  let icon_size = config
    .computed(&["menu", "icon"])
    .length("icon-size")
    .unwrap_or(16.0) as u16;

  let icon: Element<'a, ItemMessage> = Icon::new(icon_name)
    .size(icon_size)
    .color(if danger { theme.red } else { theme.primary })
    .into();

  let label_classes: &[&str] = if danger {
    &["menu", "text-name", "text-danger"]
  } else {
    &["menu", "text-name"]
  };

  let classes: &'static [&'static str] = &["menu", "row", "compact"];

  let content = container(
    row![
      icon,
      Space::new().width(8),
      config.text(label, label_classes)
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill),
  )
  .padding(config.sizing(classes).padding())
  .width(Length::Fill);

  clickable(hover_row(config, classes, content), move |_, _, _| {
    Some(message.clone())
  })
}
