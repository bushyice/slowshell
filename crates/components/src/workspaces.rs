use std::collections::HashMap;

use iced::{
  Alignment, Element, Length,
  widget::{Space, column, container, row},
};
use slowshell_commons::panels::PanelOrientation;
use slowshell_compositor::CompositorStore;
use slowshell_config::{Config, Value};
use slowshell_core::{
  Store,
  listeners::{CompositorEvent, ListenerAction},
  message::{EventFilter, ItemMessage},
};
use slowshell_widgets::{Animated, clickable};

use crate::{Component, ComponentContext, ComponentOptions, spaced_component};

pub struct Workspaces {
  show_all: bool,
  show_text: bool,
  override_text: Option<HashMap<String, Value>>,
}

impl Default for Workspaces {
  fn default() -> Self {
    Self {
      show_all: false,
      show_text: false,
      override_text: None,
    }
  }
}

impl Component for Workspaces {
  fn events(&self) -> Vec<slowshell_core::message::EventFilter> {
    vec![EventFilter::Compositor(
      CompositorEvent::WORKSPACES | CompositorEvent::MONITORS,
    )]
  }

  fn watch(&mut self, _store: &mut Store, options: Option<&ComponentOptions>) {
    self.show_all = options.and_then(|o| o.bool("all")).unwrap_or(false);
    self.show_text = options.and_then(|o| o.bool("text")).unwrap_or(false);
    self.override_text = options
      .and_then(|o| o.table("override-text"))
      .map(|t| t.iter().cloned().collect());
  }

  fn view<'a>(
    &'a self,
    config: &'a Config,
    store: &'a Store,
    ctx: &ComponentContext,
    _options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let Some(Ok(state)) = store.borrow::<CompositorStore>().map(|x| x.state()) else {
      return row![].into();
    };
    let vertical = ctx.orientation == PanelOrientation::Vertical;

    let show_text = if vertical { false } else { self.show_text };

    let style = config.computed(&["workspaces"]);
    let size = config.sizing(&["workspaces"]);

    let active_width = style.length("width-active").unwrap_or(24.0);
    let inactive_width = style.length("width-inactive").unwrap_or(20.0);
    let spacing = size.gap;

    let horizontal = ctx.orientation == PanelOrientation::Horizontal;
    let padding = size.padding();

    let workspaces: Vec<Element<'a, ItemMessage>> = state
      .workspaces
      .iter()
      .filter_map(|workspace| {
        if !self.show_all
          && workspace
            .output
            .as_ref()
            .map(|o| o != ctx.monitor)
            .unwrap_or(true)
        {
          return None;
        }

        let active = workspace.is_active;
        let width = if active { active_width } else { inactive_width };
        let idx = workspace.idx;

        let label = self
          .override_text
          .as_ref()
          .and_then(|t| t.get(&workspace.idx.to_string()).and_then(|v| v.as_str()))
          .map(|s| s.to_string())
          .unwrap_or(workspace.name.clone().unwrap_or(workspace.idx.to_string()));

        let active_classes: &'static [&'static str] = if active {
          &["workspaces", "active"]
        } else {
          &["workspaces", "inactive"]
        };

        let widget = Animated::new(width, move |width| {
          let width = *width;

          let content: Element<'_, ItemMessage> = if show_text {
            let label = config.text(label.clone(), active_classes);
            container(label)
              .padding(padding)
              .align_x(Alignment::Center)
              .align_y(Alignment::Center)
              .width(width)
              .into()
          } else if horizontal {
            container(Space::new().height(Length::Fill).width(width)).into()
          } else {
            container(Space::new().width(Length::Fill).height(width)).into()
          };

          container(content)
            .style(config.container_cached(active_classes))
            .into()
        })
        .animates_layout(true);

        Some(clickable(widget.into(), move |_, _, _| {
          Some(ItemMessage::Action(ListenerAction::FocusWorkspace(idx)))
        }))
      })
      .collect();

    let content: Element<'a, ItemMessage> = if horizontal {
      row(workspaces).spacing(spacing).into()
    } else {
      column(workspaces).spacing(spacing).into()
    };

    spaced_component(config, ctx, content, false)
  }
}
