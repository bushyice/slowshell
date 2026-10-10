use iced::{Color, Element};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{EventFilter, ItemEffect, ItemMessage},
  types::Ustr,
};
use std::collections::HashMap;

mod evwrap;
pub use evwrap::{EventWrapper, clickable, shift_down, track_modifiers};

mod popup;
pub use popup::{Backdrop, SizedPopup};

mod options;
pub use options::ComponentOptions;

mod icons;
pub use icons::Icon;

mod cover;
pub use cover::mpris_cover_handle;

mod misc;
pub use misc::separator;

mod animation;
pub use animation::{
  Animate, Animated, AnimatedState, AnimatedValue, AnimationBuilder, CustomAnimation, Easing,
  Event, HoverBackground, HoverTransition, Mode, Motion, SlideIn, Spring, Transition,
  icss_edge_offset, icss_mode,
};

mod pseudo;
pub use pseudo::HoverFlag;

pub fn hover_row<'a>(
  config: &Config,
  classes: &[&str],
  content: impl Into<Element<'a, ItemMessage>>,
) -> Element<'a, ItemMessage> {
  let style = config.computed(classes);
  let radius = style.length("border-radius").unwrap_or(0.0);
  let base = style.color("background-color").map(|c| c.to_iced());
  let hover = style
    .color("hover-background")
    .map(|c| c.to_iced())
    .unwrap_or(Color::TRANSPARENT);
  let border_width = style.length("border-width").unwrap_or(0.0) as f32;
  let border_color = style.color("border-color").map(|c| c.to_iced());

  let mut widget = HoverBackground::new(content, hover, radius);

  if let Some(base) = base {
    widget = widget.with_base(base);
  }

  if border_width > 0.0 {
    widget = widget.with_border(border_width, border_color.unwrap_or(Color::TRANSPARENT));
  }

  widget.into()
}

mod systemmon;
pub use systemmon::{
  SystemMonOptions, SystemMonWidget, render_system_mon_widget, set_sample_processes,
};

mod confirm;
pub use confirm::{ConfirmDialogProps, render_confirm_dialog};

mod notifications {
  use iced::Element;
  use iced::{Color, Length};
  use slowshell_commons::notifications::NotificationImage;

  pub fn notification_icon<'a, Message: 'a + 'static>(
    source: Option<&NotificationImage>,
    app_icon: Option<&str>,
    size: u16,
    color: Color,
  ) -> Element<'a, Message> {
    let side = Length::Fixed(size as f32);

    match source {
      Some(NotificationImage::Rgba {
        width,
        height,
        pixels,
      }) => iced::widget::image(iced::widget::image::Handle::from_rgba(
        *width,
        *height,
        pixels.clone(),
      ))
      .width(side)
      .height(side)
      .into(),

      Some(NotificationImage::Path(path)) => {
        iced::widget::image(iced::widget::image::Handle::from_path(path))
          .width(side)
          .height(side)
          .into()
      }

      None => crate::Icon::any(
        app_icon
          .into_iter()
          .chain(std::iter::once("application-x-executable-symbolic")),
      )
      .size(size)
      .color(color)
      .into_element(),
    }
  }
}

pub use notifications::notification_icon;

mod quicksettings;
pub use quicksettings::render_tile;

pub trait Renderable {
  fn events(&self, _store: &Store) -> Vec<EventFilter> {
    Vec::new()
  }

  fn initialize(&self, config: &Config, store: &mut Store) -> Box<dyn std::any::Any + Send> {
    let _ = (config, store);
    Box::new(())
  }

  fn initialize_with_options(
    &self,
    config: &Config,
    store: &mut Store,
    options: Option<&ComponentOptions>,
  ) -> Box<dyn std::any::Any + Send> {
    let _ = options;
    self.initialize(config, store)
  }

  fn view_with_options<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: iced_exwlshell::reexport::IcedId,
    data: &'a dyn std::any::Any,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let _ = options;
    self.view(config, store, id, data)
  }

  fn handle_message(&mut self, message: &ItemMessage) -> Option<ItemEffect> {
    let _ = message;
    Some(ItemEffect::None)
  }

  fn update(
    &mut self,
    config: &Config,
    store: &mut Store,
    data: &dyn std::any::Any,
  ) -> Option<ItemEffect> {
    let _ = (config, store, data);
    None
  }

  fn on_event(
    &mut self,
    _config: &Config,
    _store: &mut Store,
    _event: &ListenerAction,
  ) -> Option<ItemEffect> {
    None
  }

  fn close(&mut self, _store: &mut Store) {}

  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: iced_exwlshell::reexport::IcedId,
    data: &'a dyn std::any::Any,
  ) -> Element<'a, ItemMessage>;
}

pub type Renderables = HashMap<Ustr, Box<dyn Renderable>>;

pub fn sl_slider<'a>(
  _id: &'static str,
  command: &'static str,
  value: f32,
  range: std::ops::RangeInclusive<f32>,
  disabled: bool,
  style: &icss::theme::resolve::computed::ComputedStyle,
  theme: &slowshell_config::style::Theme,
) -> Element<'a, ItemMessage> {
  use iced::Length;
  use iced::widget::container;
  use iced::widget::slider;
  use slowshell_core::types::ToUstr;

  let bg = style
    .color("background-color")
    .map(|c| c.to_iced())
    .unwrap_or(theme.subtext);
  let bg_rail = style
    .color("background-rail")
    .map(|c| c.to_iced())
    .unwrap_or(theme.mantle);
  let bg_rail_filled = style
    .color("background-rail-filled")
    .map(|c| c.to_iced())
    .unwrap_or(theme.primary);
  let border_color = style
    .color("border-color")
    .map(|c| c.to_iced())
    .unwrap_or(Color::TRANSPARENT);
  let border_width = style.length("border-width").unwrap_or(0.0);
  let rail_width = style.length("rail-width").unwrap_or(4.0);
  let rail_border_width = style.length("rail-border-width").unwrap_or(0.0);
  let rail_border_radius = style.length("rail-border-radius").unwrap_or(2.0);
  let rail_border_color = style
    .color("rail-border-color")
    .map(|c| c.to_iced())
    .unwrap_or(Color::TRANSPARENT);
  let shape = style.keyword("rect") == Some("true");
  let shape_size = style.length("slider-size").unwrap_or(7.0);

  let slider = slider(range, value, move |v| {
    ItemMessage::Action(ListenerAction::Payload {
      name: command.to_ustr(),
      payload: Some(slowshell_core::types::PayloadBox::new(v)),
    })
  })
  .style(move |_, _| slider::Style {
    handle: slider::Handle {
      background: bg.into(),
      border_color,
      border_width,
      shape: if !shape {
        slider::HandleShape::Circle { radius: shape_size }
      } else {
        slider::HandleShape::Rectangle {
          width: shape_size as u16,
          border_radius: iced::border::Radius::default(),
        }
      },
    },
    rail: slider::Rail {
      backgrounds: (bg_rail_filled.into(), bg_rail.into()),
      width: rail_width,
      border: iced::border::Border {
        radius: rail_border_radius.into(),
        width: rail_border_width,
        color: rail_border_color,
      },
    },
  })
  .step(0.01f32)
  .width(Length::Fill);

  let slider: Element<'a, ItemMessage> = if disabled {
    container(slider).into()
  } else {
    slider.into()
  };

  container(slider).padding([0.0, 2.0]).into()
}

pub fn truncate(input: &str, max: usize) -> String {
  if input.chars().count() > max {
    format!(
      "{}…",
      input
        .chars()
        .take(max.saturating_sub(1))
        .collect::<String>()
    )
  } else {
    input.to_string()
  }
}
