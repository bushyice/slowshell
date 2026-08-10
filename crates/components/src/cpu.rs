use std::{fs, time::Duration};

use iced::{
  Color, Element,
  widget::{container, text},
};
use slowshell_core::{
  Store,
  listeners::{FdHandle, ListenerAction},
  message::{EventFilter, ItemEffect, ItemMessage},
};

use crate::{Component, ComponentContext, MenuConfig, menu_trigger};

const TICK: &str = "component/cpu.tick";

#[derive(Default, Clone, Copy)]
struct CpuSample {
  total: u64,
  idle: u64,
}

pub struct CpuMem {
  timer_fd: Option<i32>,
  prev: Option<CpuSample>,
  cpu_usage: f32,
  mem_usage: f32,
}

fn read_cpu() -> Option<CpuSample> {
  let line = fs::read_to_string("/proc/stat")
    .ok()?
    .lines()
    .next()?
    .to_string();
  let mut parts = line.split_whitespace();
  let _ = parts.next();
  let nums: Vec<u64> = parts.filter_map(|p| p.parse().ok()).collect();
  if nums.len() < 4 {
    return None;
  }
  let total: u64 = nums.iter().sum();
  let idle = nums[3] + nums.get(4).copied().unwrap_or(0);
  Some(CpuSample { total, idle })
}

fn read_mem() -> Option<f32> {
  let data = fs::read_to_string("/proc/meminfo").ok()?;
  let mut total = None;
  let mut avail = None;
  for line in data.lines() {
    let mut parts = line.split_whitespace();
    let Some(key) = parts.next() else { continue };
    let Some(val) = parts.next().and_then(|s| s.parse::<f64>().ok()) else {
      continue;
    };
    match key {
      "MemTotal:" => total = Some(val),
      "MemAvailable:" => avail = Some(val),
      _ => {}
    }
  }
  let (total, avail) = (total?, avail?);
  if total == 0.0 {
    return None;
  }
  Some(((total - avail) / total * 100.0) as f32)
}

impl CpuMem {
  pub fn new() -> Self {
    let mut cpu = Self {
      timer_fd: None,
      prev: None,
      cpu_usage: 0.0,
      mem_usage: 0.0,
    };
    cpu.refresh();
    cpu
  }

  fn refresh(&mut self) {
    if let Some(sample) = read_cpu() {
      if let Some(prev) = self.prev {
        let dt = sample.total.saturating_sub(prev.total);
        let di = sample.idle.saturating_sub(prev.idle);
        self.cpu_usage = if dt > 0 {
          (dt.saturating_sub(di) as f32 / dt as f32 * 100.0).clamp(0.0, 100.0)
        } else {
          0.0
        };
      }
      self.prev = Some(sample);
    }
    self.mem_usage = read_mem().unwrap_or(0.0);
  }
}

impl Default for CpuMem {
  fn default() -> Self {
    Self::new()
  }
}

impl Component for CpuMem {
  fn events(&self) -> Vec<EventFilter> {
    vec![EventFilter::Named(TICK.into())]
  }

  fn watch(&mut self, store: &Store) {
    if self.timer_fd.is_some() {
      return;
    }
    let Some(handle) = store.borrow::<FdHandle>() else {
      return;
    };
    if let Ok(fd) = handle.set_interval(Duration::from_secs(2), ListenerAction::Named(TICK.into()))
    {
      self.timer_fd = Some(fd);
    }
  }

  fn stop(&mut self, store: &Store) {
    if let (Some(fd), Some(handle)) = (self.timer_fd.take(), store.borrow::<FdHandle>()) {
      handle.stop_timer(fd);
    }
  }

  fn update(&mut self, _store: &mut Store, event: &ListenerAction) -> anyhow::Result<ItemEffect> {
    if matches!(event, ListenerAction::Named(n) if &**n == TICK) {
      self.refresh();
      Ok(ItemEffect::Redraw)
    } else {
      Ok(ItemEffect::None)
    }
  }

  fn view<'a>(&self, _store: &Store, ctx: &ComponentContext) -> Element<'a, ItemMessage> {
    let label = text(format!(
      "CPU {:.0}% MEM {:.0}%",
      self.cpu_usage, self.mem_usage
    ))
    .size(12)
    .color(Color::from_rgba(1.0, 1.0, 1.0, 0.9));
    let content = container(label).padding([4, 8]);
    menu_trigger(
      content.into(),
      MenuConfig {
        content: "cpu".into(),
        panel_name: None,
        position: ctx.position,
      },
    )
  }
}
