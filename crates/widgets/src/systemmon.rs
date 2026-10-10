use crate::Renderable;
use iced::{
  Alignment, Color, Element, Length,
  widget::{Space, column, container, progress_bar, row},
};
use slowshell_commons::system::SharedSystemState;
use slowshell_config::Config;
use slowshell_core::{Store, message::ItemMessage};

#[derive(Debug, Clone, Copy)]
pub struct SystemMonOptions {
  pub show_processes: bool,
  pub max_processes: usize,
  pub show_temp: bool,
}

impl Default for SystemMonOptions {
  fn default() -> Self {
    Self {
      show_processes: true,
      max_processes: 6,
      show_temp: true,
    }
  }
}

pub struct SystemMonWidget;

impl Renderable for SystemMonWidget {
  fn initialize(&self, _config: &Config, store: &mut Store) -> Box<dyn std::any::Any + Send> {
    if SystemMonOptions::default().show_processes {
      set_sample_processes(store, true);
    }
    Box::new(())
  }

  fn close(&mut self, store: &mut Store) {
    set_sample_processes(store, false);
  }

  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    _id: iced_exwlshell::reexport::IcedId,
    _data: &'a dyn std::any::Any,
  ) -> Element<'a, ItemMessage> {
    render_system_mon_widget(config, store, SystemMonOptions::default())
  }
}

pub fn set_sample_processes(store: &Store, enabled: bool) {
  if let Some(shared) = store.borrow::<SharedSystemState>() {
    shared
      .sample_processes
      .store(enabled, std::sync::atomic::Ordering::Relaxed);
  }
}

pub fn render_system_mon_widget<'a>(
  config: &'a Config,
  store: &Store,
  options: SystemMonOptions,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();

  let Some(shared) = store.borrow::<SharedSystemState>() else {
    return container(config.text(
      "System monitor unavailable",
      &["menu", "text-status", "text-faded"],
    ))
    .padding(12)
    .into();
  };

  let snap = {
    let guard = shared.snapshot.lock().unwrap();
    guard.clone()
  };

  let mut children: Vec<Element<'a, ItemMessage>> = Vec::new();

  children.push(render_stat_bar(
    config,
    "CPU",
    snap.cpu_usage,
    format!("{:.0}%", snap.cpu_usage),
    theme.primary,
  ));

  let mem_gb_used = snap.mem_used as f64 / 1024.0 / 1024.0 / 1024.0;
  let mem_gb_total = snap.mem_total as f64 / 1024.0 / 1024.0 / 1024.0;
  children.push(render_stat_bar(
    config,
    "Memory",
    snap.mem_usage,
    format!("{:.1} / {:.1} GB", mem_gb_used, mem_gb_total),
    theme.yellow,
  ));

  let mut sub_info: Vec<Element<'a, ItemMessage>> = Vec::new();
  if options.show_temp
    && let Some(temp) = snap.temperature
  {
    sub_info.push(
      config
        .text(
          format!("Temp: {:.0}°C", temp),
          &["menu", "text-status", "text-faded"],
        )
        .into(),
    );
  }

  sub_info.push(
    config
      .text(
        format!(
          "Load: {:.2}  {:.2}  {:.2}",
          snap.load[0], snap.load[1], snap.load[2]
        ),
        &["menu", "text-status", "text-faded"],
      )
      .into(),
  );

  if snap.network_rx > 0 || snap.network_tx > 0 {
    sub_info.push(
      config
        .text(
          format!(
            "Net: ↓{}  ↑{}",
            format_bytes(snap.network_rx),
            format_bytes(snap.network_tx)
          ),
          &["menu", "text-status", "text-faded"],
        )
        .into(),
    );
  }

  children.push(row(sub_info).spacing(12).align_y(Alignment::Center).into());

  if options.show_processes && !snap.top_processes.is_empty() {
    let proc_header = row![
      config.text("Process", &["menu", "text-status", "text-faded"]),
      Space::new().width(Length::Fill),
      config.text("Memory", &["menu", "text-status", "text-faded"]),
      Space::new().width(12),
      config.text("CPU%", &["menu", "text-status", "text-faded"]),
    ]
    .align_y(Alignment::Center);

    children.push(proc_header.into());

    let mut proc_rows = Vec::new();
    for p in snap.top_processes.iter().take(options.max_processes) {
      let r = row![
        config.text(p.name_display.clone(), &["menu", "text-name"]),
        Space::new().width(Length::Fill),
        config.text(
          p.memory_display.clone(),
          &["menu", "text-status", "text-faded"]
        ),
        Space::new().width(12),
        config.text(
          p.cpu_display.clone(),
          &["menu", "text-status", "text-primary"]
        ),
      ]
      .align_y(Alignment::Center);

      proc_rows.push(crate::hover_row(
        config,
        &["menu", "row"],
        container(r)
          .padding(config.sizing(&["menu", "row"]).padding())
          .width(Length::Fill),
      ));
    }

    children.push(column(proc_rows).spacing(3).width(Length::Fill).into());
  }

  column(children)
    .spacing(config.sizing(&["menu"]).gap)
    .width(Length::Fill)
    .into()
}

fn render_stat_bar<'a>(
  config: &'a Config,
  label: &'static str,
  percent: f32,
  val_text: String,
  accent_color: Color,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();
  let header = row![
    config.text(label, &["menu", "text-name"]),
    Space::new().width(Length::Fill),
    config.text(val_text, &["menu", "text-status", "text-faded"]),
  ]
  .align_y(Alignment::Center);

  let bg = theme.mantle;
  let bar = progress_bar(0.0..=100.0, percent)
    .length(Length::Fill)
    .girth(Length::Fixed(8.0))
    .style(move |_| progress_bar::Style {
      background: bg.into(),
      bar: accent_color.into(),
      border: iced::Border {
        radius: 4.0.into(),
        ..Default::default()
      },
    });

  column![header, bar].spacing(4).width(Length::Fill).into()
}

fn format_bytes(bytes: u64) -> String {
  if bytes >= 1024 * 1024 * 1024 {
    format!("{:.1}G", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
  } else if bytes >= 1024 * 1024 {
    format!("{:.1}M", bytes as f64 / 1024.0 / 1024.0)
  } else {
    format!("{}K", bytes / 1024)
  }
}
