use std::collections::{HashMap, hash_map::Entry};

use iced::{
  Alignment, Element, mouse,
  widget::{column, container, row, text},
};
use slowshell_commons::{desktop::DesktopEntries, panels::PanelOrientation};
use slowshell_compositor::CompositorStore;
use slowshell_config::Config;
use slowshell_core::{
  Store, Window,
  listeners::ListenerAction,
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
  icon_names: Vec<String>,
  windows: Vec<(u64, bool)>,
  active: bool,
  has_windows: bool,
  pinned: bool,
}

pub struct Windows {
  icon_size: Option<f32>,
  show_titles: bool,
  title_length: usize,
  config_pinned: Vec<String>,
  pin_state: PinState,
}

impl Default for Windows {
  fn default() -> Self {
    Self {
      icon_size: None,
      show_titles: false,
      title_length: 8,
      config_pinned: Vec::new(),
      pin_state: PinState::default(),
    }
  }
}

impl Component for Windows {
  fn events(&self) -> Vec<EventFilter> {
    vec![
      EventFilter::UpdateCompositor,
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

    has_windows || !self.pinned_classes().is_empty()
  }

  fn view<'a>(
    &self,
    config: &Config,
    store: &Store,
    ctx: &ComponentContext,
    _options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let Some(Ok(state)) = store.borrow::<CompositorStore>().map(|c| c.state()) else {
      return row![].into();
    };

    let style = config.style("windows");
    let theme = &config.theme;

    let icon_size = self
      .icon_size
      .unwrap_or_else(|| style.number("icon.size").unwrap_or(22.0)) as u16;
    let font_size = style.number("font.size").unwrap_or(11.0);
    let spacing = style.number("spacing").unwrap_or(3.0);
    let radius = style.number("radius").unwrap_or(6.0);
    let padding = style.padding([2.0, 4.0]);

    let color = style.color(theme, "color", theme.text);
    let faded = style.color(theme, "color.faded", theme.subtext);
    let active_color = style.color(theme, "color.active", theme.primary);
    let active_bg = style.color(theme, "background.active", theme.primary);
    let bg = style.color(theme, "background", theme.crust);

    let desktop = store.borrow::<DesktopEntries>();
    let apps = self.build_apps(&state.active_windows, desktop);

    let horizontal = ctx.orientation == PanelOrientation::Horizontal;

    let separator_opacity = style.number("separator.opacity").unwrap_or(0.25);

    let mut items: Vec<Element<'a, ItemMessage>> = Vec::with_capacity(apps.len());
    let mut previous_pinned: Option<bool> = None;

    for app in apps {
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

      let icon = Icon::any(app.icon_names.iter().map(String::as_str)).size(icon_size);
      let symbolic = icon
        .resolved_name()
        .is_some_and(|name| name.contains("symbolic"));
      let icon_color = if app.active {
        active_color
      } else if app.has_windows {
        color
      } else {
        faded
      };

      let icon_el: Element<'a, ItemMessage> = if symbolic {
        icon.color(icon_color).into_element()
      } else {
        icon.into_element()
      };

      let body: Element<'a, ItemMessage> = if self.show_titles {
        column![
          icon_el,
          text(truncate_label(&app.title, self.title_length))
            .size(font_size)
            .color(icon_color),
        ]
        .spacing(1)
        .align_x(Alignment::Center)
        .into()
      } else {
        icon_el
      };

      let background = if app.active {
        Some(active_bg)
      } else if app.has_windows {
        Some(bg)
      } else {
        None
      };

      let button = container(body)
        .padding(padding)
        .style(move |_theme: &iced::Theme| container::Style {
          background: background.map(|color| color.into()),
          border: iced::Border {
            radius: radius.into(),
            ..Default::default()
          },
          ..Default::default()
        });

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

        let mut icon_names = vec![class.clone(), title.to_lowercase()];
        icon_names.extend(
          [
            "window",
            "application-x-executable",
            "window-symbolic",
            "application-x-executable-symbolic",
          ]
          .into_iter()
          .map(String::from),
        );

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
