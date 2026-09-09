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
