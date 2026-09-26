use std::{
  borrow::Cow,
  cell::RefCell,
  collections::{HashMap, hash_map::Entry},
  hash::{Hash, Hasher},
  sync::Arc,
};

use iced::{
  Alignment, Element, mouse,
  widget::{column, container, row},
};
use slowshell_commons::{desktop::DesktopEntries, panels::PanelOrientation};
use slowshell_compositor::CompositorStore;
use slowshell_config::Config;
use slowshell_core::{
  Store, Window,
  listeners::{CompositorEvent, ListenerAction},
  message::{EventFilter, ItemEffect, ItemMessage},
  persistence,
  types::{PayloadBox, Ustr},
};
use slowshell_widgets::{Icon, clickable, separator, shift_down, track_modifiers};

use crate::{Component, ComponentContext, ComponentOptions, spaced_component};

const PIN_ACTION: &str = "windows.toggle-pin";
const LAUNCH_ACTION: &str = "windows.launch";
const PIN_FILE: &str = "windows-pinned";

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct PinState {
  pinned: Vec<String>,
  unpinned: Vec<String>,
}

struct DockApp {
  class: String,
  title: String,
  icon_names: Vec<Cow<'static, str>>,
  windows: Vec<(u64, bool)>,
  active: bool,
  has_windows: bool,
  pinned: bool,
}

struct AppsCache {
  key: u64,
  apps: Arc<Vec<DockApp>>,
}

pub struct Windows {
  icon_size: Option<f32>,
  show_titles: bool,
  title_length: usize,
  config_pinned: Vec<String>,
  pin_state: PinState,
  apps_cache: RefCell<Option<AppsCache>>,
}

impl Default for Windows {
  fn default() -> Self {
    Self {
      icon_size: None,
      show_titles: false,
      title_length: 8,
      config_pinned: Vec::new(),
      pin_state: PinState::default(),
      apps_cache: RefCell::new(None),
    }
  }
}

impl Component for Windows {
  fn events(&self) -> Vec<EventFilter> {
    vec![
      EventFilter::Compositor(CompositorEvent::WINDOWS | CompositorEvent::FOCUS),
      EventFilter::Payload(PIN_ACTION.into()),
      EventFilter::Payload(LAUNCH_ACTION.into()),
    ]
  }

  fn watch(&mut self, _store: &mut Store, options: Option<&ComponentOptions>) {
    self.icon_size = options.and_then(|o| o.number("icon-size"));
    self.show_titles = options.and_then(|o| o.bool("show-titles")).unwrap_or(false);
    self.title_length = options
      .and_then(|o| o.int("title-length"))
      .unwrap_or(8)
      .max(1) as usize;
    self.config_pinned = options
      .and_then(|o| o.strings("pinned"))
      .map(|pinned| pinned.into_iter().map(str::to_string).collect())
      .unwrap_or_default();

    self.pin_state = persistence::load(PIN_FILE).unwrap_or_default();
  }

  fn update(
    &mut self,
    _config: &Config,
    store: &mut Store,
    event: &ListenerAction,
    _options: Option<&ComponentOptions>,
  ) -> miette::Result<ItemEffect> {
    if let ListenerAction::Payload { name, payload } = event
      && let Some(class) = payload.as_ref().and_then(|p| p.as_this::<Ustr>())
    {
      if &**name == PIN_ACTION {
        self.toggle_pin(class);

        if let Err(e) = persistence::save(PIN_FILE, &self.pin_state) {
          eprintln!("[windows] failed to persist pinned applications: {e}");
        }

        return Ok(ItemEffect::Redraw);
      }

      if &**name == LAUNCH_ACTION
        && let Some(exec) = store
          .borrow::<DesktopEntries>()
          .and_then(|entries| entries.resolve_exec(class))
      {
        slowshell_commons::desktop::spawn_exec(&exec);
      }
    }

    Ok(ItemEffect::None)
  }

  fn check_view(&self, store: &Store, _options: Option<&ComponentOptions>) -> bool {
    let has_windows = store
      .borrow::<CompositorStore>()
      .and_then(|compositor| compositor.state().ok())
      .is_some_and(|state| !state.active_windows.is_empty());

    has_windows || self.has_pinned()
  }

  fn view<'a>(
    &'a self,
    config: &'a Config,
    store: &'a Store,
    ctx: &ComponentContext,
    _options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let Some(Ok(state)) = store.borrow::<CompositorStore>().map(|c| c.state()) else {
      return row![].into();
    };

    let style = config.computed(&["windows"]);
    let size = config.sizing(&["windows"]);

    let icon_size = self
      .icon_size
      .unwrap_or_else(|| style.length("icon-size").unwrap_or(22.0)) as u16;
    let spacing = size.gap;
    let padding = size.padding();

    let desktop = store.borrow::<DesktopEntries>();
    let apps = self.build_apps_cached(&state.active_windows, desktop);

    let horizontal = ctx.orientation == PanelOrientation::Horizontal;

    let separator_opacity = 0.25;

    let mut items: Vec<Element<'a, ItemMessage>> = Vec::with_capacity(apps.len());
    let mut previous_pinned: Option<bool> = None;

    for app in apps.iter() {
      if matches!(previous_pinned, Some(true)) && !app.pinned {
        let sep: Element<'static, ItemMessage> = if horizontal {
          container(
            separator()
              .vertical(true)
              .size(0.1)
              .opacity(separator_opacity)
              .padding([10, 0]),
          )
          .padding([10, 0])
          .into()
        } else {
          container(
            separator()
              .vertical(false)
              .size(0.1)
              .opacity(separator_opacity)
              .padding([0, 10]),
          )
          .padding([10, 0])
          .into()
        };
        items.push(sep);
      }
      previous_pinned = Some(app.pinned);

      let icon = Icon::any(app.icon_names.iter().map(Cow::as_ref)).size(icon_size);
      let symbolic = icon
        .resolved_name()
        .is_some_and(|name| name.contains("symbolic"));

      let icon_classes: &[&str] = if app.active {
        &["windows", "active"]
      } else if app.has_windows {
        &["windows", "open"]
      } else {
        &["windows", "closed"]
      };

      let icon_el: Element<'a, ItemMessage> = if symbolic {
        let color = config
          .computed(icon_classes)
          .color("color")
          .map(|c| c.to_iced());
        match color {
          Some(color) => icon.color(color).into_element(),
          None => icon.into_element(),
        }
      } else {
        icon.into_element()
      };

      let body: Element<'a, ItemMessage> = if self.show_titles {
        column![
          icon_el,
          config.text(truncate_label(&app.title, self.title_length), icon_classes),
        ]
        .spacing(1)
        .align_x(Alignment::Center)
        .into()
      } else {
        icon_el
      };

      let item_classes: &'static [&'static str] = if app.active {
        &["windows", "item", "active"]
      } else if app.has_windows {
        &["windows", "item", "open"]
      } else {
        &["windows", "item"]
      };

      let button = container(body)
        .padding(padding)
        .style(config.container_cached(item_classes));

      let ids: Vec<u64> = app.windows.iter().map(|(id, _)| *id).collect();
      let active_idx = app.windows.iter().position(|(_, active)| *active);
      let has_windows = app.has_windows;
      let class = Ustr::from(app.class.as_str());

      items.push(clickable(button.into(), move |event, _layout, _cursor| {
        let iced::Event::Mouse(mouse::Event::ButtonPressed(button)) = event else {
          return None;
        };

        match button {
          mouse::Button::Left => {
            if has_windows {
              let id = cycle_id(&ids, active_idx, !shift_down())?;
              Some(ItemMessage::Action(ListenerAction::FocusWindow(id)))
            } else {
              Some(ItemMessage::Action(ListenerAction::Payload {
                name: LAUNCH_ACTION.into(),
                payload: Some(PayloadBox::new(class.clone())),
              }))
            }
          }
          mouse::Button::Right => Some(ItemMessage::Action(ListenerAction::Payload {
            name: PIN_ACTION.into(),
            payload: Some(PayloadBox::new(class.clone())),
          })),
          _ => None,
        }
      }));
    }

    let content: Element<'a, ItemMessage> = if horizontal {
      row(items)
        .spacing(spacing)
        .align_y(Alignment::Center)
        .into()
    } else {
      column(items)
        .spacing(spacing)
        .align_x(Alignment::Center)
        .into()
    };

    track_modifiers(spaced_component(config, ctx, content, false))
  }
}

impl Windows {
  fn is_pinned(&self, class: &str) -> bool {
    let configured = self.config_pinned.iter().any(|pinned| pinned == class);
    let pinned = self.pin_state.pinned.iter().any(|pinned| pinned == class);
    let unpinned = self.pin_state.unpinned.iter().any(|pinned| pinned == class);

    (configured || pinned) && !unpinned
  }

  fn has_pinned(&self) -> bool {
    self
      .config_pinned
      .iter()
      .chain(self.pin_state.pinned.iter())
      .any(|class| self.is_pinned(class))
  }

  fn build_apps_cached(
    &self,
    windows: &[Window],
    desktop: Option<&DesktopEntries>,
  ) -> Arc<Vec<DockApp>> {
    let key = self.apps_key(windows, desktop);

    if let Some(cache) = self.apps_cache.borrow().as_ref()
      && cache.key == key
    {
      return Arc::clone(&cache.apps);
    }

    let apps = Arc::new(self.build_apps(windows, desktop));
    *self.apps_cache.borrow_mut() = Some(AppsCache {
      key,
      apps: Arc::clone(&apps),
    });
    apps
  }

  fn apps_key(&self, windows: &[Window], desktop: Option<&DesktopEntries>) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    windows.len().hash(&mut hasher);
    for window in windows {
      window.id.hash(&mut hasher);
      window.class.hash(&mut hasher);
      window.is_active.hash(&mut hasher);
    }
    self.config_pinned.hash(&mut hasher);
    self.pin_state.pinned.hash(&mut hasher);
    self.pin_state.unpinned.hash(&mut hasher);
    desktop.is_some().hash(&mut hasher);
    if desktop.is_some() {
      DesktopEntries::global_version().hash(&mut hasher);
    }
    hasher.finish()
  }

  fn pinned_classes(&self) -> Vec<String> {
    let mut classes: Vec<String> = Vec::new();

    for class in self
      .config_pinned
      .iter()
      .chain(self.pin_state.pinned.iter())
    {
      if self.is_pinned(class) && !classes.iter().any(|existing| existing == class) {
        classes.push(class.clone());
      }
    }

    classes
  }

  fn toggle_pin(&mut self, class: &str) {
    if self.is_pinned(class) {
      self.pin_state.pinned.retain(|pinned| pinned != class);
      if !self.pin_state.unpinned.iter().any(|pinned| pinned == class) {
        self.pin_state.unpinned.push(class.to_string());
      }
    } else {
      self.pin_state.unpinned.retain(|pinned| pinned != class);
      if !self.pin_state.pinned.iter().any(|pinned| pinned == class) {
        self.pin_state.pinned.push(class.to_string());
      }
    }
  }

  fn build_apps(&self, windows: &[Window], desktop: Option<&DesktopEntries>) -> Vec<DockApp> {
    let mut order: Vec<String> = Vec::new();
    let mut grouped: HashMap<String, Vec<&Window>> = HashMap::new();

    for class in self.pinned_classes() {
      grouped.entry(class.clone()).or_default();
      order.push(class);
    }

    for window in windows {
      let class = if window.class.trim().is_empty() {
        "unknown".to_string()
      } else {
        window.class.clone()
      };

      match grouped.entry(class.clone()) {
        Entry::Occupied(mut entry) => entry.get_mut().push(window),
        Entry::Vacant(entry) => {
          entry.insert(vec![window]);
          order.push(class);
        }
      }
    }

    order
      .into_iter()
      .map(|class| {
        let windows = grouped.remove(&class).unwrap_or_default();
        let active = windows.iter().any(|window| window.is_active);
        let title = desktop
          .map(|entries| entries.resolve_name(&class).to_string())
          .filter(|name| !name.is_empty())
          .unwrap_or_else(|| class.clone());

        let mut icon_names: Vec<Cow<'static, str>> =
          vec![Cow::Owned(class.clone()), Cow::Owned(title.to_lowercase())];
        icon_names.extend([
          Cow::Borrowed("window"),
          Cow::Borrowed("application-x-executable"),
          Cow::Borrowed("window-symbolic"),
          Cow::Borrowed("application-x-executable-symbolic"),
        ]);

        DockApp {
          pinned: self.is_pinned(&class),
          has_windows: !windows.is_empty(),
          active,
          windows: windows
            .iter()
            .map(|window| (window.id, window.is_active))
            .collect(),
          class,
          title,
          icon_names,
        }
      })
      .collect()
  }
}

fn cycle_id(ids: &[u64], current: Option<usize>, forward: bool) -> Option<u64> {
  if ids.is_empty() {
    return None;
  }

  let index = match (current, forward) {
    (Some(index), true) => (index + 1) % ids.len(),
    (Some(index), false) => (index + ids.len() - 1) % ids.len(),
    (None, true) => 0,
    (None, false) => ids.len() - 1,
  };

  ids.get(index).copied()
}

fn truncate_label(name: &str, max: usize) -> String {
  let mut chars = name.chars();
  let prefix: String = chars.by_ref().take(max).collect();

  if chars.next().is_some() {
    format!("{prefix}…")
  } else {
    prefix
  }
}
