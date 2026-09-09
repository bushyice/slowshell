use iced::{
  Alignment, Color, Element, Length,
  widget::{Space, container},
};

#[derive(Default)]
pub struct Separator {
  vertical: bool,
  circle: bool,
  size: Option<f32>,
  opacity: Option<f32>,
  padding: Option<[u16; 2]>,
  color: Option<Color>,
}

impl Separator {
  pub fn vertical(mut self, vertical: bool) -> Self {
    self.vertical = vertical;
    self
  }

  pub fn circle(mut self, circle: bool) -> Self {
    self.circle = circle;
    self
  }

  pub fn size(mut self, size: f32) -> Self {
    self.size = Some(size);
    self
  }

  pub fn opacity(mut self, opacity: f32) -> Self {
    self.opacity = Some(opacity);
    self
  }

  pub fn padding(mut self, padding: [u16; 2]) -> Self {
    self.padding = Some(padding);
    self
  }

  pub fn color(mut self, color: Color) -> Self {
    self.color = Some(color);
    self
  }
}

impl<Message> From<Separator> for Element<'static, Message>
where
  Message: 'static,
{
  fn from(separator: Separator) -> Self {
    let size = separator.size.unwrap_or(1.5);

    let content = if separator.circle {
      Space::new().width(size).height(size)
    } else if separator.vertical {
      Space::new().height(Length::Fill).width(size)
    } else {
      Space::new().width(Length::Fill).height(size)
    };

    let radius = if separator.circle { size / 2.0 } else { 6.0 };

    let mut cont = container(content)
      .padding(separator.padding.unwrap_or([0, 12]))
      .style(move |_t: &iced::Theme| container::Style {
        background: Some(
          Color {
            a: separator.opacity.unwrap_or(1.0),
            ..separator
              .color
              .unwrap_or(Color::from_rgba(1.0, 1.0, 1.0, 0.08))
          }
          .into(),
        ),
        border: iced::Border {
          radius: radius.into(),
          ..Default::default()
        },
        ..container::Style::default()
      });

    if separator.circle {
      cont = cont.width(size).height(size);

      container(container(cont).width(size).height(size * 1.25))
        .width(if separator.vertical {
          Length::Shrink
        } else {
          Length::Fill
        })
        .height(if separator.vertical {
          Length::Fill
        } else {
          Length::Shrink
        })
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
    } else {
      cont.into()
    }
  }
}

pub fn separator() -> Separator {
  Separator::default()
}
