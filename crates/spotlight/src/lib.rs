use iced::{
  Color, Element, Event, Length, keyboard,
  widget::{center, column, container, scrollable, text},
};
use iced_layershell::reexport::{
  Anchor, IcedId, KeyboardInteractivity, Layer, NewLayerShellSettings,
};
use slowshell_config::Config;
use slowshell_core::{Store, listeners::ListenerAction};
use slowshell_desktop::{
  DesktopItem, EventFilter, ItemEffect, ItemMessage, MonitorScope, UpdateWhen, Visibility,
};
use slowshell_widgets::EventWrapper;
use std::sync::{Arc, Mutex};

struct SpotlightInner {
  search: String,
  selected_index: usize,
  should_close: bool,
}

pub struct Spotlight {
  inner: Arc<Mutex<SpotlightInner>>,
  results: Vec<String>,
}

impl Spotlight {
  pub fn new() -> Self {
    Self {
      inner: Arc::new(Mutex::new(SpotlightInner {
        search: String::new(),
        selected_index: 0,
        should_close: false,
      })),
      results: vec![
        "Something".to_string(),
        "Something else".to_string(),
        "Another thing".to_string(),
        "Thingie".to_string(),
      ],
    }
  }

  fn filtered_results(&self, search: &str) -> Vec<(usize, &str)> {
    let query = search.to_lowercase();
    self
      .results
      .iter()
      .enumerate()
      .filter(|(_, r)| query.is_empty() || r.to_lowercase().contains(&query))
      .map(|(i, r)| (i, r.as_str()))
      .collect()
  }
}

impl DesktopItem for Spotlight {
  fn id(&self) -> &str {
    "spotlight"
  }

  fn layer(&self, _config: &Config, _monitor: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
      layer: Layer::Overlay,
      anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
      exclusive_zone: Some(-1),
      size: None,
      margin: Some((0, 0, 0, 0)),
      keyboard_interactivity: KeyboardInteractivity::Exclusive,
      events_transparent: false,
      namespace: Some("slowshell-spotlight".into()),
      ..Default::default()
    }
  }

  fn visibility(&self) -> Visibility {
    Visibility::Transient
  }

  fn update_strategy(&self) -> UpdateWhen {
    UpdateWhen::OnEvent
  }

  fn monitor(&self, _config: &Config) -> MonitorScope {
    MonitorScope::Single(None)
  }

  fn init_events(&self) -> Vec<EventFilter> {
    vec![EventFilter::Named("spotlight.toggle".into())]
  }

  fn update(&mut self, _store: &Store, event: &ListenerAction) -> anyhow::Result<ItemEffect> {
    match event {
      ListenerAction::Named(name) if name.as_ref() == "spotlight.toggle" => {
        let mut inner = self.inner.lock().unwrap();
        inner.search.clear();
        inner.selected_index = 0;
        inner.should_close = false;
        Ok(ItemEffect::Show)
      }
      _ => Ok(ItemEffect::None),
    }
  }

  fn handle_message(&mut self, message: &ItemMessage) -> ItemEffect {
    if let ItemMessage::Effect(_, ItemEffect::Redraw) = message {
      let inner = self.inner.lock().unwrap();
      if inner.should_close {
        return ItemEffect::Hide;
      }
      return ItemEffect::Redraw;
    }
    ItemEffect::None
  }

  fn view(&self, _store: &Store, id: IcedId) -> Element<'_, ItemMessage> {
    let inner = self.inner.lock().unwrap();

    let search_display = if inner.search.is_empty() {
      container(
        text("Search...")
          .size(24)
          .color(Color::from_rgba(1.0, 1.0, 1.0, 0.35)),
      )
    } else {
      container(
        text(format!("{}|", inner.search))
          .size(24)
          .color(Color::WHITE),
      )
    };

    let search_bar = container(search_display)
      .width(Length::Fill)
      .padding(12)
      .style(|_t| container::Style {
        background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into()),
        border: iced::Border {
          radius: 8.0.into(),
          width: 1.0,
          color: Color::from_rgba(1.0, 1.0, 1.0, 0.15),
        },
        ..container::Style::default()
      });

    let filtered = self.filtered_results(&inner.search);
    let selected = inner.selected_index;

    let mut results_col = column![].spacing(4);
    for (i, (_orig_idx, label)) in filtered.iter().enumerate() {
      let is_selected = i == selected;

      let row_content = container(text(*label).size(18).color(Color::WHITE))
        .width(Length::Fill)
        .padding(10)
        .style(move |_t| {
          if is_selected {
            container::Style {
              background: Some(Color::from_rgba(0.5, 0.0, 0.0, 0.8).into()),
              border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
              },
              ..container::Style::default()
            }
          } else {
            container::Style {
              background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.04).into()),
              border: iced::Border {
                radius: 8.0.into(),
                ..Default::default()
              },
              ..container::Style::default()
            }
          }
        });

      results_col = results_col.push(row_content);
    }

    let content = column![
      search_bar,
      scrollable(results_col).height(Length::Fixed(600.0))
    ]
    .spacing(8)
    .padding(20)
    .width(Length::Fixed(600.0));

    let main_container = center(container(content))
      .width(Length::Fill)
      .height(Length::Fill)
      .center_x(Length::Fill)
      .center_y(Length::Fill)
      .style(|_t| container::Style {
        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()),
        ..container::Style::default()
      });

    drop(inner);

    let inner_ref = self.inner.clone();
    let result_count = filtered.len();

    EventWrapper::new(
      main_container.into(),
      move |event, layout, cursor| match event {
        Event::Keyboard(keyboard::Event::KeyPressed { key, text, .. }) => {
          match key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::Escape) => {
              inner_ref.lock().unwrap().should_close = true;
              return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
            }
            keyboard::Key::Named(keyboard::key::Named::Enter) => {
              inner_ref.lock().unwrap().should_close = true;
              return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
              let mut s = inner_ref.lock().unwrap();
              if s.selected_index > 0 {
                s.selected_index -= 1;
              }
              return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
              let mut s = inner_ref.lock().unwrap();
              if s.selected_index + 1 < result_count {
                s.selected_index += 1;
              }
              return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
            }
            keyboard::Key::Named(keyboard::key::Named::Backspace) => {
              let mut s = inner_ref.lock().unwrap();
              s.search.pop();
              s.selected_index = 0;
              return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
            }
            _ => {
              if let Some(txt) = text {
                let ch = txt.as_str();
                if !ch.is_empty() && ch.chars().all(|c| !c.is_control()) {
                  let mut s = inner_ref.lock().unwrap();
                  s.search.push_str(ch);
                  s.selected_index = 0;
                  return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
                }
              }
            }
          }
          None
        }
        Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)) => {
          if let Some(col_layout) = layout.children().next() {
            let mut col_children = col_layout.children();
            let _ = col_children.next();
            if let Some(scrollable_layout) = col_children.next() {
              if let Some(results_col) = scrollable_layout.children().next() {
                for (i, row_layout) in results_col.children().enumerate() {
                  if cursor.is_over(row_layout.bounds()) {
                    let mut s = inner_ref.lock().unwrap();
                    s.selected_index = i;
                    s.should_close = true;
                    return Some(ItemMessage::Effect(id, ItemEffect::Redraw));
                  }
                }
              }
            }
          }
          None
        }
        _ => None,
      },
    )
    .into()
  }
}
