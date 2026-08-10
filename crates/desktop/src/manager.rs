use std::collections::{HashMap, HashSet};

use iced::{Element, Task};
use iced_layershell::reexport::IcedId;
use slowshell_config::Config;
use slowshell_core::listeners::ListenerAction;
use slowshell_core::message::Message;
use slowshell_core::types::ToUstr;
use slowshell_core::{Store, types::Ustr};

use crate::{
  DeployDesktopItemAction, DeployableDesktopItem, DesktopItem, EventFilter, ItemEffect,
  ItemMessage, MonitorScope, Visibility,
};

pub struct DesktopItems {
  items: Vec<Box<dyn DesktopItem>>,
  index: HashMap<Ustr, usize>,
  active: HashSet<usize>,
  windows: HashMap<IcedId, usize>,
  window_monitors: HashMap<IcedId, Ustr>,
  reverse_windows: HashMap<usize, HashSet<IcedId>>,
  subscriptions: HashMap<EventFilter, HashSet<usize>>,
  monitors: Vec<Ustr>,
  deployable: Vec<Box<dyn DeployableDesktopItem>>,
  deployable_index: HashMap<Ustr, usize>,
  deployable_subs: HashMap<EventFilter, HashSet<usize>>,
  uninitialized: HashSet<usize>,
}

impl Default for DesktopItems {
  fn default() -> Self {
    Self {
      items: Vec::new(),
      index: HashMap::new(),
      active: HashSet::new(),
      windows: HashMap::new(),
      window_monitors: HashMap::new(),
      reverse_windows: HashMap::new(),
      subscriptions: HashMap::new(),
      monitors: Vec::new(),
      deployable: Vec::new(),
      deployable_index: HashMap::new(),
      deployable_subs: HashMap::new(),
      uninitialized: HashSet::new(),
    }
  }
}

impl DesktopItems {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn sync_monitors(&mut self, config: &Config, monitors: Vec<Ustr>) -> Task<Message> {
    let old_set: HashSet<Ustr> = self.monitors.iter().cloned().collect();
    let new_set: HashSet<Ustr> = monitors.iter().cloned().collect();

    let removed: Vec<Ustr> = self
      .monitors
      .iter()
      .filter(|m| !new_set.contains(*m))
      .cloned()
      .collect();
    let mut close_tasks = Vec::new();
    for monitor in &removed {
      close_tasks.push(self.destroy_monitor(monitor));
    }

    self.monitors = monitors;

    let first_monitor = self.monitors.first().cloned();
    let launches: Vec<(usize, Ustr)> = {
      let mut result = Vec::new();
      for idx in 0..self.items.len() {
        if !matches!(self.items[idx].visibility(), Visibility::Visible) {
          continue;
        }
        let scope = self.items[idx].monitor(config);
        match scope {
          MonitorScope::PerMonitor => {
            for monitor in &self.monitors {
              if !old_set.contains(monitor) {
                result.push((idx, monitor.clone()));
              }
            }
          }
          MonitorScope::OnMonitors(names) => {
            for name in &names {
              if new_set.contains(name) && !old_set.contains(name) {
                result.push((idx, name.clone()));
              }
            }
          }
          MonitorScope::Single(Some(name)) => {
            if new_set.contains(&name)
              && !old_set.contains(&name)
              && !self.reverse_windows.contains_key(&idx)
            {
              result.push((idx, name));
            }
          }
          MonitorScope::Single(None) => {
            if !self.reverse_windows.contains_key(&idx) {
              if let Some(monitor) = &first_monitor {
                result.push((idx, monitor.clone()));
              }
            }
          }
        }
      }
      result
    };

    let mut tasks = close_tasks;
    for (idx, monitor) in &launches {
      tasks.push(self.launch(config, *idx, monitor, None));
    }

    Task::batch(tasks)
  }

  fn destroy_monitor(&mut self, monitor: &Ustr) -> Task<Message> {
    let mut tasks = Vec::new();
    let wids: Vec<IcedId> = self
      .window_monitors
      .iter()
      .filter(|(_, m)| *m == monitor)
      .map(|(&wid, _)| wid)
      .collect();

    for wid in wids {
      self.window_monitors.remove(&wid);
      if let Some(idx) = self.windows.remove(&wid) {
        if let Some(set) = self.reverse_windows.get_mut(&idx) {
          set.remove(&wid);
        }
      }
      tasks.push(Task::done(Message::RemoveWindow(wid)));
    }
    Task::batch(tasks)
  }

  fn launch(
    &mut self,
    config: &Config,
    idx: usize,
    monitor: &Ustr,
    item: Option<&dyn DesktopItem>,
  ) -> Task<Message> {
    let settings = if let Some(item) = item {
      item.layer(config, monitor)
    } else {
      self.items[idx].layer(config, monitor)
    };
    let (id, task) = Message::open_layer(settings);
    self.windows.insert(id, idx);
    self.window_monitors.insert(id, monitor.clone());
    self.reverse_windows.entry(idx).or_default().insert(id);
    task
  }

  pub fn register(&mut self, config: &Config, item: Box<dyn DesktopItem>) -> Task<Message> {
    let scope = item.monitor(config);
    let vis = item.visibility();
    let events = item.init_events();
    let index = self.items.len();

    let task = if matches!(vis, Visibility::Visible) {
      match scope {
        MonitorScope::Single(monitor) => {
          let monitor =
            monitor.unwrap_or_else(|| self.monitors.first().cloned().unwrap_or_default());
          self.launch(config, index, &monitor, Some(&*item))
        }
        MonitorScope::PerMonitor => {
          let monitors = self.monitors.clone();
          let tasks: Vec<_> = monitors
            .iter()
            .map(|m| self.launch(config, index, m, Some(&*item)))
            .collect();
          Task::batch(tasks)
        }
        MonitorScope::OnMonitors(names) => {
          let tasks: Vec<_> = names
            .iter()
            .map(|m| self.launch(config, index, m, Some(&*item)))
            .collect();
          Task::batch(tasks)
        }
      }
    } else {
      Task::none()
    };

    for event in events {
      self.subscriptions.entry(event).or_default().insert(index);
    }

    if matches!(vis, Visibility::Visible) {
      self.active.insert(index);
    }

    self.index.insert(item.id().to_ustr(), index);
    self.uninitialized.insert(index);
    self.items.push(item);

    task
  }

  pub fn deployable(&mut self, deployable: Box<dyn DeployableDesktopItem>) {
    let idx = self.deployable.len();
    self.deployable_index.insert(deployable.id().to_ustr(), idx);

    for event in deployable.deploys_on() {
      self.deployable_subs.entry(event).or_default().insert(idx);
    }

    self.deployable.push(deployable);
  }

  pub fn check_deployables(&mut self, store: &mut Store, event: &ListenerAction) -> Task<Message> {
    let Some(matching) = self.deployable_subs.get(&event.into()).cloned() else {
      return Task::none();
    };

    let mut tasks = Vec::new();

    for idx in matching {
      let item = &mut self.deployable[idx];
      match item.deploy(store, event) {
        Ok(Some(action)) => {
          for action in action.as_many() {
            match action {
              DeployDesktopItemAction::Destroy(id) => {
                tasks.push(self.destroy(id));
              }
              DeployDesktopItemAction::Deploy(item) => {
                tasks.push(self.register(store.borrow::<Config>().unwrap(), item));
              }
              _ => {}
            }
          }
        }
        Ok(None) => {}
        Err(_) => {}
      }
    }

    Task::batch(tasks)
  }

  fn rebuild_index(&mut self, removed_idx: usize) {
    for val in self.index.values_mut() {
      if *val > removed_idx {
        *val -= 1;
      }
    }

    self.uninitialized = self
      .uninitialized
      .iter()
      .map(|&i| if i > removed_idx { i - 1 } else { i })
      .collect();

    self.active = self
      .active
      .iter()
      .map(|&i| if i > removed_idx { i - 1 } else { i })
      .collect();

    for val in self.windows.values_mut() {
      if *val > removed_idx {
        *val -= 1;
      }
    }

    self.reverse_windows = self
      .reverse_windows
      .drain()
      .map(|(i, v)| {
        let new_i = if i > removed_idx { i - 1 } else { i };
        (new_i, v)
      })
      .collect();

    for indices in self.subscriptions.values_mut() {
      *indices = indices
        .iter()
        .map(|&i| if i > removed_idx { i - 1 } else { i })
        .collect();
    }
  }

  pub fn destroy_idx(&mut self, idx: usize) -> Task<Message> {
    let task = self.close_item_windows(idx);
    self.remove_subscriptions(idx);
    self.active.remove(&idx);
    self.items.remove(idx);
    self.rebuild_index(idx);

    task
  }

  pub fn destroy(&mut self, id: Ustr) -> Task<Message> {
    let Some(idx) = self.index.remove(&id) else {
      return Task::none();
    };

    self.destroy_idx(idx)
  }

  fn launch_all(&mut self, config: &Config, idx: usize) -> Task<Message> {
    let scope = self.items[idx].monitor(config);
    let targets = match scope {
      MonitorScope::Single(monitor) => {
        let target = match monitor {
          Some(m) => m,
          None => self.monitors.first().cloned().unwrap_or_default(),
        };
        vec![target]
      }
      MonitorScope::PerMonitor => self.monitors.clone(),
      MonitorScope::OnMonitors(names) => names,
    };

    let tasks: Vec<_> = targets
      .iter()
      .map(|m| self.launch(config, idx, m, None))
      .collect();
    Task::batch(tasks)
  }

  pub fn intialize(&mut self, store: &mut Store) {
    if self.uninitialized.len() < 1 {
      return;
    }

    for idx in self.uninitialized.clone() {
      let item = &mut self.items[idx];
      match item.initialize(store) {
        _ => {}
      }
    }

    self.uninitialized.clear();
  }

  pub fn update(
    &mut self,
    config: &Config,
    store: &mut Store,
    event: &ListenerAction,
  ) -> Task<Message> {
    let Some(matching) = self.subscriptions.get(&event.into()).cloned() else {
      return Task::none();
    };

    let mut tasks = Vec::new();
    let mut pending_destroy: Vec<usize> = Vec::new();

    for idx in matching {
      // if !self.active.contains(&idx) {
      //   continue;
      // }
      match self.items[idx].update(store, event) {
        Ok(effect) => {
          if effect == ItemEffect::ReallyDestroy {
            pending_destroy.push(idx);
          } else if let Some(task) = self.apply_effect(config, idx, effect) {
            tasks.push(task);
          }
        }
        Err(_) => {
          // TOOD: Actual error handling
        }
      }
    }

    pending_destroy.sort_unstable();
    for &idx in pending_destroy.iter().rev() {
      tasks.push(self.destroy_idx(idx));
    }

    Task::batch(tasks)
  }

  pub fn view<'a>(
    &'a self,
    window_id: IcedId,
    store: &'a Store,
  ) -> Option<Element<'a, ItemMessage>> {
    let &idx = self.windows.get(&window_id)?;
    if !self.active.contains(&idx) {
      return None;
    }
    Some(self.items[idx].view(store, window_id))
  }

  pub fn handle_message(&mut self, config: &Config, message: &ItemMessage) -> Task<Message> {
    if let ItemMessage::Effect(id, _) = message {
      if let Some(&idx) = self.windows.get(id) {
        let effect = self.items[idx].handle_message(message);
        if effect == ItemEffect::ReallyDestroy {
          let item_id = self.items[idx].id().to_ustr();
          return self.destroy(item_id);
        }
        if let Some(task) = self.apply_effect(config, idx, effect) {
          return task;
        }
      }
    }
    Task::none()
  }

  fn apply_effect(
    &mut self,
    config: &Config,
    idx: usize,
    effect: ItemEffect,
  ) -> Option<Task<Message>> {
    match effect {
      ItemEffect::Show => {
        let task = if !self.reverse_windows.contains_key(&idx) {
          Some(self.launch_all(config, idx))
        } else {
          None
        };
        self.active.insert(idx);
        task
      }
      ItemEffect::Hide => {
        self.active.remove(&idx);
        if matches!(self.items[idx].visibility(), Visibility::Transient) {
          Some(self.close_item_windows(idx))
        } else {
          None
        }
      }
      ItemEffect::Destroy => {
        self.remove_subscriptions(idx);
        self.active.remove(&idx);
        Some(self.close_item_windows(idx))
      }
      ItemEffect::ReallyDestroy => Some(self.destroy_idx(idx)),
      ItemEffect::Subscribe(new_filters) => {
        self.remove_subscriptions(idx);
        for filter in new_filters {
          self.subscriptions.entry(filter).or_default().insert(idx);
        }
        None
      }
      ItemEffect::Redraw | ItemEffect::None => None,
    }
  }

  fn close_item_windows(&mut self, idx: usize) -> Task<Message> {
    let mut tasks = Vec::new();
    if let Some(wids) = self.reverse_windows.remove(&idx) {
      for wid in wids {
        self.windows.remove(&wid);
        self.window_monitors.remove(&wid);
        tasks.push(Task::done(Message::RemoveWindow(wid)));
      }
    }
    self.active.remove(&idx);
    Task::batch(tasks)
  }

  fn remove_subscriptions(&mut self, idx: usize) {
    for indices in self.subscriptions.values_mut() {
      indices.remove(&idx);
    }
  }
}
