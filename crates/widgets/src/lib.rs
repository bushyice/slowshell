use iced::Element;
use slowshell_core::{Store, message::ItemMessage, types::Ustr};
use std::collections::HashMap;

mod evwrap;
pub use evwrap::EventWrapper;

mod popup;
pub use popup::{Backdrop, SizedPopup};

pub trait Renderable {
  fn view<'a>(
    &self,
    store: &Store,
    id: iced_layershell::reexport::IcedId,
    data: &dyn std::any::Any,
  ) -> Element<'a, ItemMessage>;
}

pub type Renderables = HashMap<Ustr, Box<dyn Renderable>>;
