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

mod icons;
pub use icons::Icon;

mod misc;
pub use misc::separator;

mod animation;
pub use animation::{
  Animate, Animated, AnimatedState, AnimatedValue, AnimationBuilder, Easing, Event,
  HoverBackground, HoverTransition, Mode, Motion, SlideIn, Spring, Transition,
};

mod systemmon;
pub use systemmon::{SystemMonOptions, SystemMonWidget, render_system_mon_widget};

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
  fn events(&self) -> Vec<EventFilter> {
    Vec::new()
  }

  fn initialize(&self, config: &Config, store: &mut Store) -> Box<dyn std::any::Any + Send> {
    let _ = (config, store);
    Box::new(())
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
    id: iced_layershell::reexport::IcedId,
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
  style: &slowshell_config::style::Style,
  theme: &slowshell_config::style::Theme,
) -> Element<'a, ItemMessage> {
  use iced::Length;
  use iced::widget::container;
  use iced::widget::slider;
  use slowshell_core::types::ToUstr;

  let bg = style.color(theme, "background", theme.subtext);
  let bg_rail = style.color(theme, "background.rail", theme.mantle);
  let bg_rail_filled = style.color(theme, "background.rail.filled", theme.primary);
  let border_color = style.color(theme, "border.color", Color::TRANSPARENT);
  let border_width = style.number("border.width").unwrap_or(0.0);
  let rail_width = style.number("rail.width").unwrap_or(4.0);
  let rail_border_width = style.number("rail.border.width").unwrap_or(0.0);
  let rail_border_radius = style.number("rail.border.radius").unwrap_or(2.0);
  let rail_border_color = style.color(theme, "rail.border.color", Color::TRANSPARENT);
  let shape = style.bool("rect");
  let shape_size = style.number("slider.size").unwrap_or(7.0);

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
