use std::cell::Cell;
use std::rc::Rc;

use iced::{
  Element, Length, Rectangle, Size, Vector,
  advanced::{
    Clipboard, Layout, Shell, layout, mouse, overlay, renderer,
    widget::{Operation, Tree, Widget},
  },
  widget::container,
};
use slowshell_core::message::ItemMessage;

#[derive(Clone)]
pub struct HoverFlag(Rc<Cell<bool>>);

impl Default for HoverFlag {
  fn default() -> Self {
    Self(Rc::new(Cell::new(false)))
  }
}

impl HoverFlag {
  pub fn get(&self) -> bool {
    self.0.get()
  }

  pub fn set(&self, hovered: bool) {
    self.0.set(hovered);
  }

  pub fn update(&self, bounds: Rectangle, cursor: mouse::Cursor) -> bool {
    let hovered = cursor.is_over(bounds);
    if hovered == self.0.get() {
      return false;
    }
    self.0.set(hovered);
    true
  }

  pub fn style(
    &self,
    base: container::Style,
    hover: container::Style,
  ) -> impl Fn(&iced::Theme) -> container::Style + 'static {
    let flag = self.0.clone();
    move |_| if flag.get() { hover } else { base }
  }

  pub fn track<'a>(
    &self,
    content: impl Into<Element<'a, ItemMessage>>,
  ) -> Element<'a, ItemMessage> {
    Element::new(HoverTracker {
      flag: self.clone(),
      content: content.into(),
    })
  }
}

struct HoverTracker<'a> {
  flag: HoverFlag,
  content: Element<'a, ItemMessage>,
}

impl Widget<ItemMessage, iced::Theme, iced::Renderer> for HoverTracker<'_> {
  fn size(&self) -> Size<Length> {
    self.content.as_widget().size()
  }

  fn children(&self) -> Vec<Tree> {
    vec![Tree::new(&self.content)]
  }

  fn diff(&self, tree: &mut Tree) {
    tree.diff_children(std::slice::from_ref(&self.content));
  }

  fn layout(
    &mut self,
    tree: &mut Tree,
    renderer: &iced::Renderer,
    limits: &layout::Limits,
  ) -> layout::Node {
    self
      .content
      .as_widget_mut()
      .layout(&mut tree.children[0], renderer, limits)
  }

  fn draw(
    &self,
    tree: &Tree,
    renderer: &mut iced::Renderer,
    theme: &iced::Theme,
    style: &renderer::Style,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    viewport: &Rectangle,
  ) {
    self.flag.update(layout.bounds(), cursor);

    self.content.as_widget().draw(
      &tree.children[0],
      renderer,
      theme,
      style,
      layout,
      cursor,
      viewport,
    );
  }

  fn update(
    &mut self,
    tree: &mut Tree,
    event: &iced::Event,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    renderer: &iced::Renderer,
    clipboard: &mut dyn Clipboard,
    shell: &mut Shell<'_, ItemMessage>,
    viewport: &Rectangle,
  ) {
    if self.flag.update(layout.bounds(), cursor) {
      shell.request_redraw();
    }

    self.content.as_widget_mut().update(
      &mut tree.children[0],
      event,
      layout,
      cursor,
      renderer,
      clipboard,
      shell,
      viewport,
    );
  }

  fn operate(
    &mut self,
    tree: &mut Tree,
    layout: Layout<'_>,
    renderer: &iced::Renderer,
    operation: &mut dyn Operation,
  ) {
    self
      .content
      .as_widget_mut()
      .operate(&mut tree.children[0], layout, renderer, operation);
  }

  fn mouse_interaction(
    &self,
    tree: &Tree,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    viewport: &Rectangle,
    renderer: &iced::Renderer,
  ) -> mouse::Interaction {
    self.content.as_widget().mouse_interaction(
      &tree.children[0],
      layout,
      cursor,
      viewport,
      renderer,
    )
  }

  fn overlay<'b>(
    &'b mut self,
    tree: &'b mut Tree,
    layout: Layout<'b>,
    renderer: &iced::Renderer,
    viewport: &Rectangle,
    translation: Vector,
  ) -> Option<overlay::Element<'b, ItemMessage, iced::Theme, iced::Renderer>> {
    self.content.as_widget_mut().overlay(
      &mut tree.children[0],
      layout,
      renderer,
      viewport,
      translation,
    )
  }
}
