use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{Datelike, Local, NaiveDate, Weekday};
use iced::{
  Alignment, Element, Length,
  widget::{Space, column, container, row},
};
use iced_exwlshell::reexport::IcedId;
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::{FdHandle, ListenerAction},
  message::{EventFilter, ItemEffect, ItemMessage},
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{Backdrop, Icon, Renderable, SizedPopup, clickable};

const CLOCK_TICK: &str = "menus/clock.tick";

pub struct ClockRenderable {
  offset: Arc<Mutex<i32>>,
  timer_fd: Arc<Mutex<Option<i32>>>,
}

impl Default for ClockRenderable {
  fn default() -> Self {
    Self {
      offset: Arc::new(Mutex::new(0)),
      timer_fd: Arc::new(Mutex::new(None)),
    }
  }
}

impl ClockRenderable {
  const PREVIOUS: usize = 0;
  const NEXT: usize = 1;
  const TODAY: usize = 2;

  fn current_offset(&self) -> i32 {
    self.offset.lock().map(|offset| *offset).unwrap_or(0)
  }
}

impl Renderable for ClockRenderable {
  fn events(&self, _store: &Store) -> Vec<EventFilter> {
    vec![EventFilter::Named(CLOCK_TICK.into())]
  }

  fn initialize(&self, _config: &Config, store: &mut Store) -> Box<dyn std::any::Any + Send> {
    let Ok(mut timer) = self.timer_fd.lock() else {
      return Box::new(());
    };

    if timer.is_none()
      && let Some(handle) = store.borrow::<FdHandle>()
      && let Ok(fd) = handle.set_named_interval(Duration::from_secs(1), CLOCK_TICK)
    {
      *timer = Some(fd);
    }

    Box::new(())
  }

  fn close(&mut self, store: &mut Store) {
    let Ok(mut timer) = self.timer_fd.lock() else {
      return;
    };

    if let (Some(fd), Some(handle)) = (timer.take(), store.borrow::<FdHandle>()) {
      handle.stop_timer(fd);
    }
  }

  fn on_event(
    &mut self,
    _config: &Config,
    _store: &mut Store,
    event: &ListenerAction,
  ) -> Option<ItemEffect> {
    match event {
      ListenerAction::Timer { name, .. } if name.as_ref() == CLOCK_TICK => Some(ItemEffect::Redraw),
      _ => None,
    }
  }

  fn handle_message(&mut self, message: &ItemMessage) -> Option<ItemEffect> {
    let ItemMessage::Effect(_, ItemEffect::Custom([40, kind, 0, 0])) = message else {
      return None;
    };

    let Ok(mut offset) = self.offset.lock() else {
      return None;
    };

    *offset = match *kind {
      Self::PREVIOUS => *offset - 1,
      Self::NEXT => *offset + 1,
      Self::TODAY => 0,
      _ => return None,
    };

    Some(ItemEffect::Redraw)
  }

  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    data: &'a dyn std::any::Any,
  ) -> Element<'a, ItemMessage> {
    let data = data.downcast_ref::<PopupSettings>().unwrap();

    let now = Local::now();
    let month_offset = self.current_offset();
    let month = shift_month(now.year(), now.month(), month_offset);

    let mut children: Vec<Element<'a, ItemMessage>> = Vec::new();
    children.push(render_clock_face(now, config));
    children.push(render_calendar_nav(month, month_offset, id, config));
    children.push(render_weekday_header(config));
    children.push(render_month_grid(month, now.date_naive(), config));

    let list_style = config.computed(&["menu", "list"]);
    let list_spacing = config.sizing(&["menu", "list"]).gap;
    let list_width = list_style.length("list-width").unwrap_or(220.0);
    let list = column(children)
      .spacing(list_spacing)
      .width(Length::Fixed(list_width));

    let inner_padding = config.sizing(&["menu", "inner"]).padding();
    let popup_style = config.container_cached(&["menu"]);

    SizedPopup::new(container(
      container(list).padding(inner_padding).style(popup_style),
    ))
    .with_width(120.)
    .with_backdrop(Backdrop::transparent().with_close_on_click(true))
    .with_position(data.position.0.get(store), data.position.1.get(store))
    .with_panel_edge(data.panel_edge)
    .with_panel_insets(store, data.monitor.as_deref())
    .with_on_close(ItemMessage::Effect(id, ItemEffect::Hide))
    .into()
  }
}

fn navigation_message(id: IcedId, kind: usize) -> ItemMessage {
  ItemMessage::Effect(id, ItemEffect::Custom([40, kind, 0, 0]))
}

fn render_clock_face<'a>(
  now: chrono::DateTime<Local>,
  config: &'a Config,
) -> Element<'a, ItemMessage> {
  let time_size = config
    .computed(&["menu", "clock", "time"])
    .length("clock-time-size")
    .unwrap_or(38.0);
  let seconds_size = config
    .computed(&["menu", "clock", "seconds"])
    .length("clock-seconds-size")
    .unwrap_or(15.0);

  let time = now.format("%H:%M").to_string();
  let seconds = now.format("%S").to_string();
  let date = now.format("%A, %-d %B %Y").to_string();
  let zone = now.format("%Z").to_string();

  let big_time = row![
    config.text(time, &["menu"]).size(time_size),
    config
      .text(seconds, &["menu", "text-faded"])
      .size(seconds_size),
  ]
  .align_y(Alignment::End)
  .spacing(6);

  column![
    big_time,
    config.text(date, &["menu", "text-name"]),
    config.text(zone, &["menu", "text-status", "text-faded"]),
  ]
  .spacing(2)
  .into()
}

fn render_calendar_nav<'a>(
  month: (i32, u32),
  offset: i32,
  id: IcedId,
  config: &'a Config,
) -> Element<'a, ItemMessage> {
  let faded = config.palette().subtext;

  let arrow_size = config
    .computed(&["menu", "arrow-size"])
    .length("arrow-size")
    .unwrap_or(12.0) as u16;

  let prev_icon: Element<'a, ItemMessage> = Icon::new("go-previous-symbolic")
    .size(arrow_size)
    .color(faded)
    .into();
  let prev = clickable(prev_icon, move |_, _, _| {
    Some(navigation_message(id, ClockRenderable::PREVIOUS))
  });

  let next_icon: Element<'a, ItemMessage> = Icon::new("go-next-symbolic")
    .size(arrow_size)
    .color(faded)
    .into();
  let next = clickable(next_icon, move |_, _, _| {
    Some(navigation_message(id, ClockRenderable::NEXT))
  });

  let label = month_label(month);

  let title: Element<'a, ItemMessage> = if offset == 0 {
    config.text(label, &["menu", "text-header"]).into()
  } else {
    clickable(
      config
        .text(label, &["menu", "text-header", "text-primary"])
        .into(),
      move |_, _, _| Some(navigation_message(id, ClockRenderable::TODAY)),
    )
  };

  row![
    prev,
    Space::new().width(Length::Fill),
    title,
    Space::new().width(Length::Fill),
    next,
  ]
  .align_y(Alignment::Center)
  .into()
}

fn render_weekday_header<'a>(config: &'a Config) -> Element<'a, ItemMessage> {
  let labels = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

  let cells: Vec<Element<'a, ItemMessage>> = labels
    .iter()
    .map(|label| {
      container(config.text(*label, &["menu", "text-status", "text-faded"]))
        .width(Length::FillPortion(1))
        .align_x(Alignment::Center)
        .into()
    })
    .collect();

  row(cells).into()
}

fn render_month_grid<'a>(
  month: (i32, u32),
  today: NaiveDate,
  config: &'a Config,
) -> Element<'a, ItemMessage> {
  let calendar_spacing = config
    .computed(&["menu", "calendar"])
    .length("calendar-spacing")
    .unwrap_or(2.0);

  let (year, month_num) = month;
  let first = NaiveDate::from_ymd_opt(year, month_num, 1).unwrap_or(today);
  let days_in_month = days_in_month(year, month_num);
  let leading = monday_index(first.weekday());

  let mut rows: Vec<Element<'a, ItemMessage>> = Vec::new();
  let mut cells: Vec<Element<'a, ItemMessage>> = Vec::new();

  for _ in 0..leading {
    cells.push(empty_cell());
  }

  for day in 1..=days_in_month {
    let date = NaiveDate::from_ymd_opt(year, month_num, day).unwrap_or(first);
    cells.push(day_cell(date, today, config));

    if cells.len() == 7 {
      rows.push(row(std::mem::take(&mut cells)).into());
    }
  }

  if !cells.is_empty() {
    while cells.len() < 7 {
      cells.push(empty_cell());
    }
    rows.push(row(cells).into());
  }

  container(column(rows).spacing(calendar_spacing))
    .width(Length::Fill)
    .into()
}

fn day_cell<'a>(date: NaiveDate, today: NaiveDate, config: &'a Config) -> Element<'a, ItemMessage> {
  let is_today = date == today;
  let is_weekend = matches!(date.weekday(), Weekday::Sat | Weekday::Sun);

  let classes: &[&str] = if is_today {
    &["menu", "text-invert"]
  } else if is_weekend {
    &["menu", "text-faded"]
  } else {
    &["menu"]
  };

  let height = Length::Fixed(
    config
      .computed(&["menu", "calendar", "cell"])
      .length("calendar-cell-height")
      .unwrap_or(28.0),
  );
  let label = config.text(date.day().to_string(), classes);

  let cell: Element<'a, ItemMessage> = if is_today {
    container(label)
      .width(Length::FillPortion(1))
      .height(height)
      .align_x(Alignment::Center)
      .align_y(Alignment::Center)
      .style(config.container_cached(&["menu", "calendar", "today", "fill"]))
      .into()
  } else {
    container(label)
      .width(Length::FillPortion(1))
      .height(height)
      .align_x(Alignment::Center)
      .align_y(Alignment::Center)
      .into()
  };

  cell
}

fn empty_cell<'a>() -> Element<'a, ItemMessage> {
  Space::new().width(Length::FillPortion(1)).into()
}

fn shift_month(year: i32, month: u32, offset: i32) -> (i32, u32) {
  let zero_based = year as i64 * 12 + (month as i64 - 1) + offset as i64;
  let new_year = zero_based.div_euclid(12) as i32;
  let new_month = zero_based.rem_euclid(12) as u32 + 1;
  (new_year, new_month)
}

fn days_in_month(year: i32, month: u32) -> u32 {
  let (next_year, next_month) = shift_month(year, month, 1);
  let first_next = NaiveDate::from_ymd_opt(next_year, next_month, 1);
  let first = NaiveDate::from_ymd_opt(year, month, 1);
  match (first, first_next) {
    (Some(first), Some(next)) => (next - first).num_days() as u32,
    _ => 30,
  }
}

fn monday_index(weekday: Weekday) -> u32 {
  weekday.num_days_from_monday()
}

fn month_label((year, month): (i32, u32)) -> String {
  NaiveDate::from_ymd_opt(year, month, 1)
    .map(|date| date.format("%B %Y").to_string())
    .unwrap_or_default()
}
