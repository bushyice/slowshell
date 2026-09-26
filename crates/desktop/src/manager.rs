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
  ItemMessage, MonitorScope, UpdateWhen, Visibility,
};

pub struct DesktopItems {
  items: Vec<Option<Box<dyn DesktopItem>>>,
  free: Vec<usize>,
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
      free: Vec::new(),
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

    if old_set == new_set {
      return Task::none();
    }

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
        let Some(item) = self.items[idx].as_ref() else {
          continue;
        };
        if !matches!(item.visibility(), Visibility::Visible) {
          continue;
        }
        let scope = item.monitor(config);
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
      match self.items.get(idx).and_then(|slot| slot.as_ref()) {
        Some(item) => item.layer(config, monitor),
        None => return Task::none(),
      }
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
    let mut events = item.init_events();

    if item.update_strategy() == UpdateWhen::EveryFrame && !events.contains(&EventFilter::Frame) {
      events.push(EventFilter::Frame);
    }

    let index = match self.free.pop() {
      Some(index) => index,
      None => {
        self.items.push(None);
        self.items.len() - 1
      }
    };

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
    self.items[index] = Some(item);

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

  pub fn check_deployables(
    &mut self,
    config: &Config,
    store: &mut Store,
    event: &ListenerAction,
  ) -> Task<Message> {
    let mut matching: Vec<usize> = self
      .deployable_subs
      .get(&event.into())
      .map(|indices| indices.iter().copied().collect())
      .unwrap_or_default();

    if let Some(indices) = self.deployable_subs.get(&EventFilter::All) {
      matching.extend(indices.iter().copied());
    }

    if let ListenerAction::Payload {
      name,
      payload: None,
    } = event
      && let Some(indices) = self.deployable_subs.get(&EventFilter::Named(name.clone()))
    {
      matching.extend(indices.iter().copied());
    }

    matching.sort_unstable();
    matching.dedup();

    if matching.is_empty() {
      return Task::none();
    }

    let mut tasks = Vec::new();

    for idx in matching {
      let item = &mut self.deployable[idx];
      match item.deploy(config, store, event) {
        Ok(Some(action)) => {
          for action in action.as_many() {
            match action {
              DeployDesktopItemAction::Destroy(id) => {
                tasks.push(self.destroy(id));
              }
              DeployDesktopItemAction::Deploy(item) => {
                tasks.push(self.register(config, item));
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

  pub fn destroy_idx(&mut self, idx: usize) -> Task<Message> {
    let task = self.close_item_windows(idx);
    self.remove_subscriptions(idx);
    self.active.remove(&idx);
    self.uninitialized.remove(&idx);

    if let Some(item) = self.items[idx].take() {
      self.index.remove(&item.id().to_ustr());
      self.free.push(idx);
    }

    task
  }

  pub fn destroy(&mut self, id: Ustr) -> Task<Message> {
    let Some(idx) = self.index.remove(&id) else {
      return Task::none();
    };

    self.destroy_idx(idx)
  }

  fn launch_all(&mut self, config: &Config, idx: usize) -> Task<Message> {
    let Some(scope) = self
      .items
      .get(idx)
      .and_then(|slot| slot.as_ref())
      .map(|item| item.monitor(config))
    else {
      return Task::none();
    };
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
      if let Some(item) = self.items[idx].as_mut() {
        let _ = item.initialize(store);
      }
    }

    self.uninitialized.clear();
  }

  fn matching_items(&self, event: &ListenerAction) -> Vec<usize> {
    let mut matching: Vec<usize> = self
      .subscriptions
      .get(&event.into())
      .map(|indices| indices.iter().copied().collect())
      .unwrap_or_default();

    if let Some(indices) = self.subscriptions.get(&EventFilter::All) {
      matching.extend(indices.iter().copied());
    }

    if let ListenerAction::Payload {
      name,
      payload: None,
    } = event
      && let Some(indices) = self.subscriptions.get(&EventFilter::Named(name.clone()))
    {
      matching.extend(indices.iter().copied());
    }

    matching
  }

  pub fn wants_frames(&self) -> bool {
    self
      .subscriptions
      .get(&EventFilter::Frame)
      .is_some_and(|indices| !indices.is_empty())
  }

  pub fn update(
    &mut self,
    config: &Config,
    store: &mut Store,
    event: &ListenerAction,
  ) -> Task<Message> {
    let mut matching = self.matching_items(event);

    if let ListenerAction::UpdateCompositor(changed) = event
      && !changed.is_empty()
    {
      for (filter, indices) in &self.subscriptions {
        if let EventFilter::Compositor(mask) = filter
          && mask.intersects(*changed)
        {
          matching.extend(indices.iter().copied());
        }
      }
    }

    matching.sort_unstable();
    matching.dedup();

    if matching.is_empty() {
      return Task::none();
    }

    let mut tasks = Vec::new();
    let mut pending_destroy: Vec<usize> = Vec::new();

    for idx in matching {
      // if !self.active.contains(&idx) {
      //   continue;
      // }
      let Some(item) = self.items.get_mut(idx).and_then(|slot| slot.as_mut()) else {
        continue;
      };
      match item.update(config, store, event) {
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
    config: &'a Config,
    store: &'a Store,
  ) -> Option<Element<'a, ItemMessage>> {
    let &idx = self.windows.get(&window_id)?;
    if !self.active.contains(&idx) {
      return None;
    }
    let monitor = self
      .window_monitors
      .get(&window_id)
      .map(|m| &**m)
      .unwrap_or("");
    let item = self.items.get(idx).and_then(|slot| slot.as_ref())?;
    Some(item.view(config, store, window_id, monitor))
  }

  pub fn handle_message(
    &mut self,
    config: &Config,
    message: &ItemMessage,
    store: Option<&mut Store>,
  ) -> Task<Message> {
    if let ItemMessage::Effect(id, _) = message {
      if let Some(&idx) = self.windows.get(id) {
        let (effect, item_id) = {
          let Some(item) = self.items.get_mut(idx).and_then(|slot| slot.as_mut()) else {
            return Task::none();
          };
          let effect = item.handle_message(store, message);
          (effect, item.id().to_ustr())
        };
        if effect == ItemEffect::ReallyDestroy {
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
      ItemEffect::ShowAndSubscribe(new_filters) => {
        let task = if !self.reverse_windows.contains_key(&idx) {
          Some(self.launch_all(config, idx))
        } else {
          None
        };
        self.active.insert(idx);

        self.remove_subscriptions(idx);
        for filter in new_filters {
          self.subscriptions.entry(filter).or_default().insert(idx);
        }

        task
      }
      ItemEffect::Hide => {
        self.active.remove(&idx);
        let transient = self
          .items
          .get(idx)
          .and_then(|slot| slot.as_ref())
          .is_some_and(|item| matches!(item.visibility(), Visibility::Transient));
        if transient {
          Some(self.close_item_windows(idx))
        } else {
          None
        }
      }
      ItemEffect::ReallyHide => {
        self.active.remove(&idx);
        Some(self.close_item_windows(idx))
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
      ItemEffect::UpdateWindow(ws) => {
        let mut tasks = Vec::new();
        if let Some(wids) = self.reverse_windows.get(&idx) {
          for &wid in wids {
            if let Some(margin) = ws.margin {
              tasks.push(Task::done(Message::MarginChange { id: wid, margin }));
            }
            if let Some(size) = ws.size {
              tasks.push(Task::done(Message::SizeChange { id: wid, size }));
            }
            if let Some(zone) = ws.exclusive_zone {
              tasks.push(Task::done(Message::ExclusiveZoneChange {
                id: wid,
                zone_size: zone,
              }));
            }
          }
        }
        Some(Task::batch(tasks))
      }
      ItemEffect::Custom([1, y, 0, 0]) => Some(iced::widget::operation::scroll_to(
        iced::widget::Id::new("spotlight-list"),
        iced::widget::scrollable::AbsoluteOffset {
          x: None,
          y: Some(y as f32),
        },
      )),
      ItemEffect::Custom(_) => None,
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
    self.subscriptions.retain(|_, indices| {
      indices.remove(&idx);
      !indices.is_empty()
    });
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::{Arc, Mutex};

  use iced_layershell::reexport::NewLayerShellSettings;
  use slowshell_core::listeners::CompositorEvent;

  struct MockItem {
    calls: Arc<Mutex<Vec<CompositorEvent>>>,
  }

  impl DesktopItem for MockItem {
    fn id(&self) -> &str {
      "mock"
    }

    fn layer(&self, _config: &Config, _monitor: &str) -> NewLayerShellSettings {
      unimplemented!("hidden items are never launched")
    }

    fn visibility(&self) -> Visibility {
      Visibility::Toggleable(false)
    }

    fn update_strategy(&self) -> UpdateWhen {
      UpdateWhen::OnEvent
    }

    fn init_events(&self) -> Vec<EventFilter> {
      vec![EventFilter::Compositor(CompositorEvent::WINDOWS)]
    }

    fn update(
      &mut self,
      _config: &Config,
      _store: &mut Store,
      event: &ListenerAction,
    ) -> miette::Result<ItemEffect> {
      if let ListenerAction::UpdateCompositor(changed) = event {
        self.calls.lock().unwrap().push(*changed);
      }
      Ok(ItemEffect::None)
    }

    fn view<'a>(
      &'a self,
      _config: &'a Config,
      _store: &'a Store,
      _id: IcedId,
      _monitor: &str,
    ) -> Element<'a, ItemMessage> {
      iced::widget::Space::new().into()
    }
  }

  struct MockEvents {
    id: &'static str,
    filters: Vec<EventFilter>,
    calls: Arc<Mutex<Vec<String>>>,
  }

  impl DesktopItem for MockEvents {
    fn id(&self) -> &str {
      self.id
    }

    fn layer(&self, _config: &Config, _monitor: &str) -> NewLayerShellSettings {
      unimplemented!("hidden items are never launched")
    }

    fn visibility(&self) -> Visibility {
      Visibility::Toggleable(false)
    }

    fn update_strategy(&self) -> UpdateWhen {
      UpdateWhen::OnEvent
    }

    fn init_events(&self) -> Vec<EventFilter> {
      self.filters.clone()
    }

    fn update(
      &mut self,
      _config: &Config,
      _store: &mut Store,
      event: &ListenerAction,
    ) -> miette::Result<ItemEffect> {
      self.calls.lock().unwrap().push(format!("{event:?}"));
      Ok(ItemEffect::None)
    }

    fn view<'a>(
      &'a self,
      _config: &'a Config,
      _store: &'a Store,
      _id: IcedId,
      _monitor: &str,
    ) -> Element<'a, ItemMessage> {
      iced::widget::Space::new().into()
    }
  }

  #[test]
  fn all_and_named_subscribers_receive_bare_commands() {
    let config = Config::default();
    let mut store = Store::new();
    let mut items = DesktopItems::new();
    let all_calls = Arc::new(Mutex::new(Vec::new()));
    let named_calls = Arc::new(Mutex::new(Vec::new()));

    let _ = items.register(
      &config,
      Box::new(MockEvents {
        id: "mock-all",
        filters: vec![EventFilter::All],
        calls: Arc::clone(&all_calls),
      }),
    );
    let _ = items.register(
      &config,
      Box::new(MockEvents {
        id: "mock-named",
        filters: vec![EventFilter::Named("fixture.toggle".into())],
        calls: Arc::clone(&named_calls),
      }),
    );

    let bare = ListenerAction::Payload {
      name: "fixture.toggle".into(),
      payload: None,
    };
    let _ = items.update(&config, &mut store, &bare);

    assert_eq!(all_calls.lock().unwrap().len(), 1);
    assert_eq!(named_calls.lock().unwrap().len(), 1);

    let _ = items.update(
      &config,
      &mut store,
      &ListenerAction::Named("fixture.desktop".into()),
    );
    assert_eq!(all_calls.lock().unwrap().len(), 2);
    assert_eq!(named_calls.lock().unwrap().len(), 1);
  }

  #[test]
  fn compositor_subscribers_are_dispatched_by_area() {
    let config = Config::default();
    let mut store = Store::new();
    let mut items = DesktopItems::new();
    let calls = Arc::new(Mutex::new(Vec::new()));

    let _ = items.register(
      &config,
      Box::new(MockItem {
        calls: Arc::clone(&calls),
      }),
    );

    let _ = items.update(
      &config,
      &mut store,
      &ListenerAction::UpdateCompositor(CompositorEvent::WORKSPACES | CompositorEvent::MONITORS),
    );
    assert!(calls.lock().unwrap().is_empty());

    let changed = CompositorEvent::WINDOWS | CompositorEvent::FOCUS;
    let _ = items.update(
      &config,
      &mut store,
      &ListenerAction::UpdateCompositor(changed),
    );
    assert_eq!(*calls.lock().unwrap(), vec![changed]);
  }

  #[test]
  fn emptied_frame_subscription_stops_requesting_frames() {
    let mut items = DesktopItems::default();

    items
      .subscriptions
      .entry(EventFilter::Frame)
      .or_default()
      .insert(0);
    assert!(items.wants_frames());

    items.remove_subscriptions(0);

    assert!(
      !items.subscriptions.contains_key(&EventFilter::Frame),
      "an empty frame subscription set must be dropped"
    );
    assert!(
      !items.wants_frames(),
      "the frame loop must stop once the last frame subscriber unsubscribes"
    );
  }
}
