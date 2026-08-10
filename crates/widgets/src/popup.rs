use iced::{
  Color, Element, Length, Padding,
  widget::{Pin, Space, container, mouse_area, stack},
};
use slowshell_commons::panels::{PanelEdge, PanelPositions};
use slowshell_core::{Store, message::ItemMessage};

#[derive(Debug, Clone)]
pub struct Backdrop {
  pub color: Color,
  pub close_on_click: bool,
}

impl Default for Backdrop {
  fn default() -> Self {
    Self {
      color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
      close_on_click: true,
    }
  }
}

impl Backdrop {
  pub fn transparent() -> Self {
    Self {
      color: Color::TRANSPARENT,
      close_on_click: true,
    }
  }

  pub fn with_opacity(mut self, opacity: f32) -> Self {
    self.color = Color::from_rgba(0.0, 0.0, 0.0, opacity.clamp(0.0, 1.0));
    self
  }

  pub fn with_color(mut self, color: Color) -> Self {
    self.color = color;
    self
  }

  pub fn with_close_on_click(mut self, v: bool) -> Self {
    self.close_on_click = v;
    self
  }
}

pub struct SizedPopup<'a> {
  content: Element<'a, ItemMessage>,
  x: f32,
  y: f32,
  size: (f32, f32),
  backdrop: Option<Backdrop>,
  on_close: Option<ItemMessage>,
  insets: Padding,
}

impl<'a> SizedPopup<'a> {
  pub fn new(content: impl Into<Element<'a, ItemMessage>>) -> Self {
    Self {
      content: content.into(),
      x: 0.0,
      y: 0.0,
      size: (0.0, 0.0),
      backdrop: None,
      on_close: None,
      insets: Padding::ZERO,
    }
  }

  pub fn with_width(mut self, size: f32) -> Self {
    self.size.0 = size;
    self
  }

  pub fn with_height(mut self, size: f32) -> Self {
    self.size.1 = size;
    self
  }

  pub fn with_position(mut self, x: f32, y: f32) -> Self {
    self.x = x;
    self.y = y;
    self
  }

  pub fn with_backdrop(mut self, backdrop: Backdrop) -> Self {
    self.backdrop = Some(backdrop);
    self
  }

  pub fn with_on_close(mut self, msg: ItemMessage) -> Self {
    self.on_close = Some(msg);
    self
  }

  pub fn with_panel_insets(mut self, store: &Store) -> Self {
    if let Some(positions) = store.borrow::<PanelPositions>() {
      let mut top: f32 = 0.0;
      let mut bottom: f32 = 0.0;
      let mut left: f32 = 0.0;
      let mut right: f32 = 0.0;

      positions.for_each(|_name, edge| match edge {
        PanelEdge::Top { height } => top += *height as f32,
        PanelEdge::Bottom { height } => bottom += *height as f32,
        PanelEdge::Left { width } => left += *width as f32,
        PanelEdge::Right { width } => right += *width as f32,
      });

      self.insets = Padding {
        top,
        right,
        bottom,
        left,
      };
    }
    self
  }

  pub fn with_insets(mut self, insets: Padding) -> Self {
    self.insets = insets;
    self
  }

  pub fn into_element(self) -> Element<'a, ItemMessage> {
    let x = (self.x + self.insets.left) - self.size.0;
    let y = (self.y + self.insets.top) - self.size.1;

    let content_box = container(self.content).padding(Padding {
      top: 0.0,
      left: 0.0,
      right: self.insets.right.max(0.0),
      bottom: self.insets.bottom.max(0.0),
    });

    let shielded_content = mouse_area(content_box).on_press(ItemMessage::Noop);

    let actual = Pin::new(shielded_content).x(x.max(0.0)).y(y.max(0.0));

    match self.backdrop {
      Some(backdrop) => {
        let backdrop_color = backdrop.color;
        let backdrop_fill = container(Space::new().width(Length::Fill).height(Length::Fill))
          .width(Length::Fill)
          .height(Length::Fill)
          .style(move |_t: &iced::Theme| container::Style {
            background: Some(backdrop_color.into()),
            ..container::Style::default()
          });

        let backdrop_element: Element<'a, ItemMessage> = if backdrop.close_on_click {
          if let Some(on_close) = self.on_close {
            mouse_area(backdrop_fill).on_press(on_close).into()
          } else {
            backdrop_fill.into()
          }
        } else {
          backdrop_fill.into()
        };

        stack([backdrop_element, actual.into()])
          .width(Length::Fill)
          .height(Length::Fill)
          .into()
      }
      None => actual.into(),
    }
  }
}

impl<'a> From<SizedPopup<'a>> for Element<'a, ItemMessage> {
  fn from(popup: SizedPopup<'a>) -> Self {
    popup.into_element()
  }
}
