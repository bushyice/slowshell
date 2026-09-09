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
  listeners::ListenerAction,
  message::{EventFilter, ItemMessage},
};
use slowshell_widgets::clickable;

use crate::{Component, ComponentContext, ComponentOptions};

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
    vec![EventFilter::UpdateCompositor]
  }

  fn watch(&mut self, _store: &mut Store, options: Option<&ComponentOptions>) {
    self.show_all = options.and_then(|o| o.bool("all")).unwrap_or(false);
    self.show_text = options.and_then(|o| o.bool("text")).unwrap_or(false);
    self.override_text = options
      .and_then(|o| o.table("override-text"))
      .map(|t| t.iter().cloned().collect());
  }

  fn view<'a>(
    &self,
    config: &Config,
    store: &Store,
    ctx: &ComponentContext,
    _options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let Some(Ok(state)) = store.borrow::<CompositorStore>().map(|x| x.state()) else {
      return row![].into();
    };
    let vertical = ctx.orientation == PanelOrientation::Vertical;

    let show_text = if vertical { false } else { self.show_text };

    let style = config.style("workspaces");

    let font_size = style.number("font.size").unwrap_or(12.);

    let spacing = style.number("spacing").unwrap_or(4.);

    let active_width = style.number("width.active").unwrap_or(24.0);

    let inactive_width = style.number("width.inactive").unwrap_or(20.0);

    let color =
      config
        .style("component")
        .color(&config.theme, "color.primary", config.theme.primary);

    let color_inactive = config
      .style("component")
      .color(&config.theme, "text", config.theme.text);

    let text_color =
      config
        .style("component")
        .color(&config.theme, "color.invert", config.theme.text);

    let text_color_inactive =
      config
        .style("component")
        .color(&config.theme, "color.invert", config.theme.text);

    let radius = style.number("radius").unwrap_or(999.0);

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

        let widget = if show_text {
          container(
            iced::widget::text(
              self
                .override_text
                .as_ref()
                .and_then(|t| t.get(&workspace.idx.to_string()).and_then(|v| v.as_str()))
                .map(|s| s.to_string())
                .unwrap_or(workspace.name.clone().unwrap_or(workspace.idx.to_string())),
            )
            .color(if active {
              text_color
            } else {
              text_color_inactive
            })
            .size(font_size),
          )
          .padding(style.padding([0., 4.]))
          .align_x(Alignment::Center)
          .align_y(Alignment::Center)
          .width(width)
        } else {
          container(if ctx.orientation == PanelOrientation::Horizontal {
            Space::new().height(Length::Fill).width(width)
          } else {
            Space::new().width(Length::Fill).height(width)
          })
        }
        .style(move |_t: &iced::Theme| container::Style {
          background: if active {
            Some(color.into())
          } else {
            Some(color_inactive.into())
          },
          border: iced::Border {
            radius: radius.into(),
            ..Default::default()
          },
          ..Default::default()
        });

        Some(clickable(widget.into(), move |_, _, _| {
          Some(ItemMessage::Action(ListenerAction::FocusWorkspace(idx)))
        }))
      })
      .collect();

    if ctx.orientation == PanelOrientation::Horizontal {
      row(workspaces).spacing(spacing).into()
    } else {
      column(workspaces).spacing(spacing).into()
    }
  }
}
