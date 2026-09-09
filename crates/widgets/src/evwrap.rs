use iced::{
  Element, Event, Length,
  advanced::{
    Clipboard, Layout, Shell, layout, mouse, renderer,
    widget::{Tree, Widget},
  },
};
use slowshell_core::message::ItemMessage;

pub struct EventWrapper<'a, Message> {
  content: Element<'a, Message>,
  on_event: Box<dyn Fn(&Event, Layout<'_>, mouse::Cursor) -> Option<Message> + 'a>,
}

impl<'a, Message> EventWrapper<'a, Message> {
  pub fn new(
    content: Element<'a, Message>,
    on_event: impl Fn(&Event, Layout<'_>, mouse::Cursor) -> Option<Message> + 'a,
  ) -> Self {
    Self {
      content,
      on_event: Box::new(on_event),
    }
  }
}

impl<'a, Message> Widget<Message, iced::Theme, iced::Renderer> for EventWrapper<'a, Message> {
  fn size(&self) -> iced::Size<Length> {
    self.content.as_widget().size()
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
    viewport: &iced::Rectangle,
  ) {
    self.content.as_widget().draw(
      &tree.children[0],
      renderer,
      theme,
      style,
      layout,
      cursor,
      viewport,
    )
  }

  fn children(&self) -> Vec<Tree> {
    vec![Tree::new(&self.content)]
  }

  fn diff(&self, tree: &mut Tree) {
    tree.diff_children(std::slice::from_ref(&self.content))
  }

  fn update(
    &mut self,
    tree: &mut Tree,
    event: &Event,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    renderer: &iced::Renderer,
    clipboard: &mut dyn Clipboard,
    shell: &mut Shell<'_, Message>,
    viewport: &iced::Rectangle,
  ) {
    if let Some(msg) = (self.on_event)(event, layout, cursor) {
      shell.publish(msg);
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
    )
  }

  fn mouse_interaction(
    &self,
    tree: &Tree,
    layout: Layout<'_>,
    cursor: mouse::Cursor,
    viewport: &iced::Rectangle,
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
    viewport: &iced::Rectangle,
    translation: iced::Vector,
  ) -> Option<iced::advanced::overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
    self.content.as_widget_mut().overlay(
      &mut tree.children[0],
      layout,
      renderer,
      viewport,
      translation,
    )
  }
}

impl<'a, Message: 'a> From<EventWrapper<'a, Message>> for Element<'a, Message> {
  fn from(wrapper: EventWrapper<'a, Message>) -> Self {
    Element::new(wrapper)
  }
}

pub fn clickable<'a>(
  content: Element<'a, ItemMessage>,
  on_click: impl Fn(&Event, Layout<'_>, mouse::Cursor) -> Option<ItemMessage> + 'a,
) -> Element<'a, ItemMessage> {
  EventWrapper::new(content, move |event, layout, cursor| {
    if !cursor.is_over(layout.bounds()) {
      return None;
    }
    let Event::Mouse(mouse::Event::ButtonPressed(button)) = event else {
      return None;
    };
    if !matches!(button, mouse::Button::Left | mouse::Button::Right) {
      return None;
    }

    on_click(event, layout, cursor)
  })
  .into()
}
