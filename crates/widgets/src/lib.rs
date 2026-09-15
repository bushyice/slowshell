use iced::Element;
use slowshell_config::Config;
use slowshell_core::{
  Store,
  message::{ItemEffect, ItemMessage},
  types::Ustr,
};
use std::collections::HashMap;

mod evwrap;
pub use evwrap::{EventWrapper, clickable};

mod popup;
pub use popup::{Backdrop, SizedPopup};

mod icons;
pub use icons::Icon;

mod misc;
pub use misc::separator;

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

pub trait Renderable {
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

  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: iced_layershell::reexport::IcedId,
    data: &'a dyn std::any::Any,
  ) -> Element<'a, ItemMessage>;
}

pub type Renderables = HashMap<Ustr, Box<dyn Renderable>>;
