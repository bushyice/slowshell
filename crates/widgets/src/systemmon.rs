use crate::Renderable;
use iced::{
  Alignment, Color, Element, Length,
  widget::{Space, column, container, progress_bar, row, text},
};
use slowshell_commons::system::SharedSystemState;
use slowshell_config::{Config, style::Style, style::Theme};
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
  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    _id: iced_layershell::reexport::IcedId,
    _data: &'a dyn std::any::Any,
  ) -> Element<'a, ItemMessage> {
    render_system_mon_widget(config, store, SystemMonOptions::default())
  }
}

pub fn render_system_mon_widget<'a>(
  config: &'a Config,
  store: &Store,
  options: SystemMonOptions,
) -> Element<'a, ItemMessage> {
  let style = config.style("menu");
  let theme = &config.theme;

  let Some(shared) = store.borrow::<SharedSystemState>() else {
    return container(
      text("System monitor unavailable")
        .size(style.number("status.font.size").unwrap_or(11.0))
        .color(style.color(theme, "color.faded", theme.subtext)),
    )
    .padding(12)
    .into();
  };

  if options.show_processes {
    shared
      .sample_processes
      .store(true, std::sync::atomic::Ordering::Relaxed);
  }

  let snap = {
    let guard = shared.snapshot.lock().unwrap();
    guard.clone()
  };

  let mut children: Vec<Element<'a, ItemMessage>> = Vec::new();

  children.push(render_stat_bar(
    "CPU",
    snap.cpu_usage,
    format!("{:.0}%", snap.cpu_usage),
    style.color(theme, "color.primary", theme.primary),
    &style,
    theme,
  ));

  let mem_gb_used = snap.mem_used as f64 / 1024.0 / 1024.0 / 1024.0;
  let mem_gb_total = snap.mem_total as f64 / 1024.0 / 1024.0 / 1024.0;
  children.push(render_stat_bar(
    "Memory",
    snap.mem_usage,
    format!("{:.1} / {:.1} GB", mem_gb_used, mem_gb_total),
    style.color(theme, "color.secondary", theme.yellow),
    &style,
    theme,
  ));

  let mut sub_info: Vec<Element<'a, ItemMessage>> = Vec::new();
  if options.show_temp
    && let Some(temp) = snap.temperature
  {
    sub_info.push(
      text(format!("Temp: {:.0}°C", temp))
        .size(style.number("status.font.size").unwrap_or(10.0))
        .color(style.color(theme, "color.faded", theme.subtext))
        .into(),
    );
  }

  sub_info.push(
    text(format!(
      "Load: {:.2}  {:.2}  {:.2}",
      snap.load[0], snap.load[1], snap.load[2]
    ))
    .size(style.number("status.font.size").unwrap_or(11.0))
    .color(style.color(theme, "color.faded", theme.subtext))
    .into(),
  );

  if snap.network_rx > 0 || snap.network_tx > 0 {
    sub_info.push(
      text(format!(
        "Net: ↓{}  ↑{}",
        format_bytes(snap.network_rx),
        format_bytes(snap.network_tx)
      ))
      .size(style.number("status.font.size").unwrap_or(11.0))
      .color(style.color(theme, "color.faded", theme.subtext))
      .into(),
    );
  }

  children.push(row(sub_info).spacing(12).align_y(Alignment::Center).into());

  if options.show_processes && !snap.top_processes.is_empty() {
    let proc_header = row![
      text("Process")
        .size(style.number("status.font.size").unwrap_or(10.0))
        .color(style.color(theme, "color.faded", theme.subtext)),
      Space::new().width(Length::Fill),
      text("Memory")
        .size(style.number("status.font.size").unwrap_or(10.0))
        .color(style.color(theme, "color.faded", theme.subtext)),
      Space::new().width(12),
      text("CPU%")
        .size(style.number("status.font.size").unwrap_or(10.0))
        .color(style.color(theme, "color.faded", theme.subtext)),
    ]
    .align_y(Alignment::Center);

    children.push(proc_header.into());

    let mut proc_rows = Vec::new();
    for p in snap.top_processes.iter().take(options.max_processes) {
      let name = if p.name.chars().count() > 14 {
        format!("{}…", p.name.chars().take(13).collect::<String>())
      } else {
        p.name.clone()
      };

      let mem_mb = p.memory as f64 / 1024.0 / 1024.0;
      let mem_text = if mem_mb >= 1024.0 {
        format!("{:.1}G", mem_mb / 1024.0)
      } else {
        format!("{:.0}M", mem_mb)
      };

      let r = row![
        text(name)
          .size(style.number("name.font.size").unwrap_or(11.0))
          .color(style.color(theme, "color", theme.text)),
        Space::new().width(Length::Fill),
        text(mem_text)
          .size(style.number("status.font.size").unwrap_or(10.0))
          .color(style.color(theme, "color.faded", theme.subtext)),
        Space::new().width(12),
        text(format!("{:.0}%", p.cpu_usage))
          .size(style.number("status.font.size").unwrap_or(10.0))
          .color(style.color(theme, "color.primary", theme.primary)),
      ]
      .align_y(Alignment::Center);

      let bg = style.color(theme, "row.background", Color::TRANSPARENT);
      let row_radius = style.number("row.radius").unwrap_or(4.0);

      proc_rows.push(
        container(r)
          .padding(style.padding([2.0, 6.0]))
          .width(Length::Fill)
          .style(move |_| container::Style {
            background: Some(bg.into()),
            border: iced::Border {
              radius: row_radius.into(),
              ..Default::default()
            },
            ..Default::default()
          })
          .into(),
      );
    }

    children.push(column(proc_rows).spacing(3).width(Length::Fill).into());
  }

  column(children)
    .spacing(style.number("spacing").unwrap_or(8.0))
    .width(Length::Fill)
    .into()
}

fn render_stat_bar<'a>(
  label: &'static str,
  percent: f32,
  val_text: String,
  accent_color: Color,
  style: &Style,
  theme: &'a Theme,
) -> Element<'a, ItemMessage> {
  let header = row![
    text(label)
      .size(style.number("name.font.size").unwrap_or(13.0))
      .color(theme.text),
    Space::new().width(Length::Fill),
    text(val_text)
      .size(style.number("status.font.size").unwrap_or(11.0))
      .color(theme.subtext),
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
