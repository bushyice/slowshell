use iced::{
  Color, Element, Event, Length, Padding, Point, Rectangle, Size, Vector,
  advanced::{
    Clipboard, Layout, Shell, layout, mouse, overlay, renderer,
    widget::{self, Widget},
  },
  widget::{Space, container, mouse_area, stack},
};
use iced_layershell::reexport::core::keyboard;
use slowshell_commons::panels::{PanelEdge, PanelPositions};
use slowshell_core::{Store, message::ItemMessage};

use crate::EventWrapper;

pub struct PopupPin<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
  Renderer: renderer::Renderer,
{
  content: Element<'a, Message, Theme, Renderer>,
  width: Length,
  height: Length,
  position: Point,
  from_bottom: bool,
}

impl<'a, Message, Theme, Renderer> PopupPin<'a, Message, Theme, Renderer>
where
  Renderer: renderer::Renderer,
{
  pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
    Self {
      content: content.into(),
      width: Length::Fill,
      height: Length::Fill,
      position: Point::ORIGIN,
      from_bottom: false,
    }
  }

  pub fn x(mut self, x: f32) -> Self {
    self.position.x = x;
    self
  }

  pub fn y(mut self, y: f32) -> Self {
    self.position.y = y;
    self
  }

  pub fn anchor_bottom(mut self, from_bottom: bool) -> Self {
    self.from_bottom = from_bottom;
    self
  }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
  for PopupPin<'a, Message, Theme, Renderer>
where
  Renderer: renderer::Renderer,
{
  fn tag(&self) -> widget::tree::Tag {
    self.content.as_widget().tag()
  }

  fn state(&self) -> widget::tree::State {
    self.content.as_widget().state()
  }

  fn children(&self) -> Vec<widget::Tree> {
    self.content.as_widget().children()
  }

  fn diff(&self, tree: &mut widget::Tree) {
    self.content.as_widget().diff(tree);
  }

  fn size(&self) -> Size<Length> {
    Size {
      width: self.width,
      height: self.height,
    }
  }

  fn layout(
    &mut self,
    tree: &mut widget::Tree,
    renderer: &Renderer,
    limits: &layout::Limits,
  ) -> layout::Node {
    let limits = limits.width(self.width).height(self.height);

    let node = self.content.as_widget_mut().layout(tree, renderer, &limits);

    let max = limits.max();
    let child = node.size();
    let max_x = (max.width - child.width).max(0.0);
    let max_y = (max.height - child.height).max(0.0);

    let clamped_x = self.position.x.clamp(0.0, max_x);
    let clamped_y = if self.from_bottom {
      (max.height - self.position.y - child.height).clamp(0.0, max_y)
    } else {
      self.position.y.clamp(0.0, max_y)
    };

    let node = node.move_to(Point::new(clamped_x, clamped_y));

    let size = limits.resolve(self.width, self.height, node.size());
    layout::Node::with_children(size, vec![node])
  }

  fn operate(
    &mut self,
    tree: &mut widget::Tree,
    layout: Layout<'_>,
    renderer: &Renderer,
    operation: &mut dyn widget::Operation,
  ) {
    self.content.as_widget_mut().operate(
      tree,
      layout.children().next().unwrap(),
      renderer,
      operation,
    );
  }

  fn update(
    &mut self,
    tree: &mut widget::Tree,
    event: &Event,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    renderer: &Renderer,
    clipboard: &mut dyn Clipboard,
    shell: &mut Shell<'_, Message>,
    viewport: &Rectangle,
  ) {
    self.content.as_widget_mut().update(
      tree,
      event,
      layout.children().next().unwrap(),
      cursor,
      renderer,
      clipboard,
      shell,
      viewport,
    );
  }

  fn mouse_interaction(
    &self,
    tree: &widget::Tree,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    viewport: &Rectangle,
    renderer: &Renderer,
  ) -> mouse::Interaction {
    self.content.as_widget().mouse_interaction(
      tree,
      layout.children().next().unwrap(),
      cursor,
      viewport,
      renderer,
    )
  }

  fn draw(
    &self,
    tree: &widget::Tree,
    renderer: &mut Renderer,
    theme: &Theme,
    style: &renderer::Style,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    viewport: &Rectangle,
  ) {
    let bounds = layout.bounds();
    if let Some(clipped_viewport) = bounds.intersection(viewport) {
      self.content.as_widget().draw(
        tree,
        renderer,
        theme,
        style,
        layout.children().next().unwrap(),
        cursor,
        &clipped_viewport,
      );
    }
  }

  fn overlay<'b>(
    &'b mut self,
    tree: &'b mut widget::Tree,
    layout: Layout<'b>,
    renderer: &Renderer,
    viewport: &Rectangle,
    translation: Vector,
  ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
    self.content.as_widget_mut().overlay(
      tree,
      layout.children().next().unwrap(),
      renderer,
      viewport,
      translation,
    )
  }
}

impl<'a, Message, Theme, Renderer> From<PopupPin<'a, Message, Theme, Renderer>>
  for Element<'a, Message, Theme, Renderer>
where
  Message: 'a,
  Theme: 'a,
  Renderer: renderer::Renderer + 'a,
{
  fn from(
    popup_pin: PopupPin<'a, Message, Theme, Renderer>,
  ) -> Element<'a, Message, Theme, Renderer> {
    Element::new(popup_pin)
  }
}

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

fn resolve_position(
  panel_edge: Option<PanelEdge>,
  x: f32,
  y: f32,
  size: (f32, f32),
  insets: Padding,
) -> (f32, f32, bool) {
  match panel_edge {
    Some(PanelEdge::Top { .. }) => ((x + insets.left) - size.0, insets.top, false),
    Some(PanelEdge::Bottom { .. }) => ((x + insets.left) - size.0, insets.bottom, true),
    Some(PanelEdge::Left { .. }) => (x, (y + insets.top) - size.1, false),
    Some(PanelEdge::Right { .. }) | None => {
      ((x + insets.left) - size.0, (y + insets.top) - size.1, false)
    }
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
  panel_edge: Option<PanelEdge>,
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
      panel_edge: None,
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

  pub fn with_panel_edge(mut self, panel_edge: Option<PanelEdge>) -> Self {
    self.panel_edge = panel_edge;
    self
  }

  pub fn into_element(self) -> Element<'a, ItemMessage> {
    let (x, y, from_bottom) =
      resolve_position(self.panel_edge, self.x, self.y, self.size, self.insets);

    let content_box = container(self.content).padding(Padding {
      top: 0.0,
      left: 0.0,
      right: self.insets.right.max(0.0),
      bottom: if from_bottom {
        0.0
      } else {
        self.insets.bottom.max(0.0)
      },
    });

    let shielded_content = mouse_area(content_box).on_press(ItemMessage::Noop);

    let close = self.on_close.clone();
    let actual = EventWrapper::new(
      PopupPin::new(shielded_content)
        .x(x.max(0.0))
        .y(y.max(0.0))
        .anchor_bottom(from_bottom)
        .into(),
      move |event, _, _| {
        if let Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event {
          match key.as_ref() {
            keyboard::Key::Named(keyboard::key::Named::Escape) => return close.clone(),
            _ => {}
          }
        }
        None
      },
    );

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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn top_panel_drops_below_the_bar() {
    let insets = Padding {
      top: 32.0,
      bottom: 0.0,
      left: 0.0,
      right: 0.0,
    };

    let (x, y, from_bottom) = resolve_position(
      Some(PanelEdge::Top { height: 32 }),
      100.0,
      32.0,
      (280.0, 0.0),
      insets,
    );

    assert!(!from_bottom);
    assert_eq!(x, 100.0 - 280.0);
    assert_eq!(y, 32.0);
  }

  #[test]
  fn bottom_panel_anchors_from_the_bottom() {
    let insets = Padding {
      top: 0.0,
      bottom: 48.0,
      left: 0.0,
      right: 0.0,
    };

    let (_, y, from_bottom) = resolve_position(
      Some(PanelEdge::Bottom { height: 48 }),
      100.0,
      48.0,
      (280.0, 0.0),
      insets,
    );

    assert!(from_bottom);
    assert_eq!(y, 48.0);
  }

  #[test]
  fn fixed_popups_keep_the_old_offset() {
    let insets = Padding {
      top: 32.0,
      bottom: 0.0,
      left: 0.0,
      right: 0.0,
    };

    let (_, y, from_bottom) = resolve_position(None, 10.0, 20.0, (0.0, 100.0), insets);

    assert!(!from_bottom);
    assert_eq!(y, 20.0 + 32.0 - 100.0);
  }
}
