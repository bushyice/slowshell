use std::time::Duration;

use iced::{
  Color, Element, Length, Rectangle, Size, Vector,
  advanced::{
    Clipboard, Layout, Renderer, Shell, layout, mouse, overlay, renderer,
    widget::{Operation, Tree, Widget, tree},
  },
  window,
};
use slowshell_core::message::ItemMessage;

pub use iced_anim::animated::Mode;
pub use iced_anim::{
  Animate, Animated as AnimatedValue, AnimatedState, AnimationBuilder, Easing, Event, Motion,
  Spring, Transition,
};

pub const DURATION: Duration = Duration::from_millis(130);

pub fn quick() -> Mode {
  Easing::EASE_OUT.with_duration(DURATION).into()
}

/// ```ignore
/// Animated::new(self.opacity, |opacity| {
///   container(content.clone()).style(move |_| container::Style {
///     background: Some(Color { a: *opacity, ..base }.into()),
///     ..Default::default()
///   })
///   .into()
/// })
/// .animation(Easing::EASE.with_duration(Duration::from_millis(150)))
/// ```
pub struct Animated<'a, T>
where
  T: Animate + Clone + PartialEq + 'static,
{
  target: T,
  build: Box<dyn Fn(&T) -> Element<'a, ItemMessage> + 'a>,
  mode: Mode,
  animates_layout: bool,
  disabled: bool,
}

impl<'a, T> Animated<'a, T>
where
  T: Animate + Clone + PartialEq + 'static,
{
  pub fn new(target: T, build: impl Fn(&T) -> Element<'a, ItemMessage> + 'a) -> Self {
    Self {
      target,
      build: Box::new(build),
      mode: quick(),
      animates_layout: false,
      disabled: !slowshell_core::animations::enabled(),
    }
  }
  pub fn animation(mut self, mode: impl Into<Mode>) -> Self {
    self.mode = mode.into();
    self
  }

  pub fn animates_layout(mut self, animates_layout: bool) -> Self {
    self.animates_layout = animates_layout;
    self
  }

  pub fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }

  pub fn into_element(self) -> Element<'a, ItemMessage> {
    self.into()
  }
}

impl<'a, T> From<Animated<'a, T>> for Element<'a, ItemMessage>
where
  T: Animate + Clone + PartialEq + 'static,
{
  fn from(animated: Animated<'a, T>) -> Self {
    let Animated {
      target,
      build,
      mode,
      animates_layout,
      disabled,
    } = animated;

    if disabled {
      return build(&target);
    }

    AnimationBuilder::new(target, move |value| build(&value))
      .animation(mode)
      .animates_layout(animates_layout)
      .into()
  }
}

struct HoverState<T> {
  animated: AnimatedState<bool, T>,
}

/// ```ignore
/// HoverTransition::new(0.0, 1.0, |hover| {
///   container(content.clone()).style(move |_| container::Style {
///     background: Some(Color { a: *hover, ..base }.into()),
///     ..Default::default()
///   })
///   .into()
/// })
/// .animation(Easing::EASE.with_duration(Duration::from_millis(150)))
/// ```
pub struct HoverTransition<'a, T, F>
where
  T: Animate + Clone + PartialEq + 'static,
  F: Fn(&T) -> Element<'a, ItemMessage> + 'a,
{
  rest: T,
  hovered: T,
  mode: Mode,
  animates_layout: bool,
  disabled: bool,
  build: F,
  cached: Element<'a, ItemMessage>,
}

impl<'a, T, F> HoverTransition<'a, T, F>
where
  T: Animate + Clone + PartialEq + 'static,
  F: Fn(&T) -> Element<'a, ItemMessage> + 'a,
{
  pub fn new(rest: T, hovered: T, build: F) -> Self {
    let cached = build(&rest);

    Self {
      rest,
      hovered,
      mode: quick(),
      animates_layout: false,
      disabled: !slowshell_core::animations::enabled(),
      build,
      cached,
    }
  }

  pub fn animation(mut self, mode: impl Into<Mode>) -> Self {
    self.mode = mode.into();
    self
  }

  pub fn animates_layout(mut self, animates_layout: bool) -> Self {
    self.animates_layout = animates_layout;
    self
  }

  pub fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }

  fn target(&self, hovered: bool) -> T {
    if hovered {
      self.hovered.clone()
    } else {
      self.rest.clone()
    }
  }

  pub fn into_element(self) -> Element<'a, ItemMessage> {
    self.into()
  }
}

impl<'a, T, F> From<HoverTransition<'a, T, F>> for Element<'a, ItemMessage>
where
  T: Animate + Clone + PartialEq + 'static,
  F: Fn(&T) -> Element<'a, ItemMessage> + 'a,
{
  fn from(widget: HoverTransition<'a, T, F>) -> Self {
    Element::new(widget)
  }
}

impl<'a, T, F> Widget<ItemMessage, iced::Theme, iced::Renderer> for HoverTransition<'a, T, F>
where
  T: Animate + Clone + PartialEq + 'static,
  F: Fn(&T) -> Element<'a, ItemMessage> + 'a,
{
  fn size(&self) -> Size<Length> {
    let size = self.cached.as_widget().size();

    if size.is_void() {
      Size::new(Length::Shrink, Length::Shrink)
    } else {
      size
    }
  }

  fn state(&self) -> tree::State {
    let animated = AnimatedState::new(false, self.mode.clone());
    let _ = animated.current_value(|_| self.rest.clone());

    tree::State::new(HoverState { animated })
  }

  fn tag(&self) -> tree::Tag {
    tree::Tag::of::<HoverState<T>>()
  }

  fn children(&self) -> Vec<Tree> {
    vec![Tree::new(&self.cached)]
  }

  fn diff(&self, tree: &mut Tree) {
    let state = tree.state.downcast_mut::<HoverState<T>>();
    state.animated.diff(self.mode.clone());
    tree.diff_children(std::slice::from_ref(&self.cached));
  }

  fn layout(
    &mut self,
    tree: &mut Tree,
    renderer: &iced::Renderer,
    limits: &layout::Limits,
  ) -> layout::Node {
    self
      .cached
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
    self.cached.as_widget().draw(
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
    self.cached.as_widget_mut().update(
      &mut tree.children[0],
      event,
      layout,
      cursor,
      renderer,
      clipboard,
      shell,
      viewport,
    );

    let hovered = cursor.is_over(layout.bounds());
    let state = tree.state.downcast_mut::<HoverState<T>>();

    let status_changed = state.animated.needs_redraw(hovered);

    if self.disabled {
      state.animated.settle_at(self.target(hovered));

      {
        let value = state
          .animated
          .current_value(|hovered| self.target(*hovered));
        self.cached = (self.build)(&value);
      }

      if status_changed {
        shell.request_redraw();

        if self.animates_layout {
          shell.invalidate_layout();
        }
      }

      return;
    }

    if let iced::Event::Window(window::Event::RedrawRequested(now)) = event {
      state.animated.tick(*now);

      {
        let value = state
          .animated
          .current_value(|hovered| self.target(*hovered));
        self.cached = (self.build)(&value);
      }
    } else {
      let _ = state
        .animated
        .current_value(|hovered| self.target(*hovered));
    }

    if state.animated.needs_redraw(hovered) {
      shell.request_redraw();

      if self.animates_layout {
        shell.invalidate_layout();
      }
    }
  }

  fn operate(
    &mut self,
    tree: &mut Tree,
    layout: Layout<'_>,
    renderer: &iced::Renderer,
    operation: &mut dyn Operation,
  ) {
    self
      .cached
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
    self
      .cached
      .as_widget()
      .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
  }

  fn overlay<'b>(
    &'b mut self,
    tree: &'b mut Tree,
    layout: Layout<'b>,
    renderer: &iced::Renderer,
    viewport: &Rectangle,
    translation: Vector,
  ) -> Option<overlay::Element<'b, ItemMessage, iced::Theme, iced::Renderer>> {
    self.cached.as_widget_mut().overlay(
      &mut tree.children[0],
      layout,
      renderer,
      viewport,
      translation,
    )
  }
}

struct HoverBackgroundState {
  progress: iced_anim::Animated<f32>,
}

pub struct HoverBackground<'a> {
  content: Element<'a, ItemMessage>,
  color: Color,
  radius: f32,
  mode: Mode,
  disabled: bool,
}

impl<'a> HoverBackground<'a> {
  pub fn new(content: impl Into<Element<'a, ItemMessage>>, color: Color, radius: f32) -> Self {
    Self {
      content: content.into(),
      color,
      radius,
      mode: quick(),
      disabled: !slowshell_core::animations::enabled(),
    }
  }

  pub fn animation(mut self, mode: impl Into<Mode>) -> Self {
    self.mode = mode.into();
    self
  }

  pub fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }

  pub fn into_element(self) -> Element<'a, ItemMessage> {
    self.into()
  }
}

impl<'a> From<HoverBackground<'a>> for Element<'a, ItemMessage> {
  fn from(widget: HoverBackground<'a>) -> Self {
    Element::new(widget)
  }
}

impl Widget<ItemMessage, iced::Theme, iced::Renderer> for HoverBackground<'_> {
  fn size(&self) -> Size<Length> {
    let size = self.content.as_widget().size();

    if size.is_void() {
      Size::new(Length::Shrink, Length::Shrink)
    } else {
      size
    }
  }

  fn state(&self) -> tree::State {
    tree::State::new(HoverBackgroundState {
      progress: iced_anim::Animated::new(0.0, self.mode.clone()),
    })
  }

  fn tag(&self) -> tree::Tag {
    tree::Tag::of::<HoverBackgroundState>()
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
    let alpha = *tree
      .state
      .downcast_ref::<HoverBackgroundState>()
      .progress
      .value();

    if alpha > 0.0 {
      renderer.fill_quad(
        renderer::Quad {
          bounds: layout.bounds(),
          border: iced::Border {
            radius: self.radius.into(),
            ..Default::default()
          },
          ..Default::default()
        },
        Color {
          a: (self.color.a * alpha.clamp(0.0, 1.0)).clamp(0.0, 1.0),
          ..self.color
        },
      );
    }

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

    let target = if cursor.is_over(layout.bounds()) {
      1.0
    } else {
      0.0
    };
    let state = tree.state.downcast_mut::<HoverBackgroundState>();

    if let iced::Event::Window(window::Event::RedrawRequested(now)) = event {
      state.progress.tick(*now);
    }

    if self.disabled {
      if *state.progress.target() != target {
        state.progress.settle_at(target);
        shell.request_redraw();
      }

      return;
    }

    if *state.progress.target() != target {
      state.progress.set_target(target);
      shell.request_redraw();
    }

    if state.progress.is_animating() {
      shell.request_redraw();
    }
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

struct SlideState {
  progress: iced_anim::Animated<f32>,
  announced: bool,
}

pub struct SlideIn<'a> {
  content: Element<'a, ItemMessage>,
  offset: Vector,
  closing: bool,
  mode: Mode,
  disabled: bool,
  on_close: Option<ItemMessage>,
}

impl<'a> SlideIn<'a> {
  pub fn new(content: impl Into<Element<'a, ItemMessage>>, offset: Vector) -> Self {
    Self {
      content: content.into(),
      offset,
      closing: false,
      mode: quick(),
      disabled: !slowshell_core::animations::enabled(),
      on_close: None,
    }
  }

  pub fn closing(mut self, closing: bool) -> Self {
    self.closing = closing;
    self
  }

  pub fn animation(mut self, mode: impl Into<Mode>) -> Self {
    self.mode = mode.into();
    self
  }

  pub fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }

  pub fn on_close(mut self, message: ItemMessage) -> Self {
    self.on_close = Some(message);
    self
  }

  pub fn into_element(self) -> Element<'a, ItemMessage> {
    self.into()
  }
}

impl<'a> From<SlideIn<'a>> for Element<'a, ItemMessage> {
  fn from(widget: SlideIn<'a>) -> Self {
    Element::new(widget)
  }
}

impl Widget<ItemMessage, iced::Theme, iced::Renderer> for SlideIn<'_> {
  fn size(&self) -> Size<Length> {
    let size = self.content.as_widget().size();

    if size.is_void() {
      Size::new(Length::Shrink, Length::Shrink)
    } else {
      size
    }
  }

  fn state(&self) -> tree::State {
    let target = if self.closing { 0.0 } else { 1.0 };
    let initial = if self.disabled { target } else { 0.0 };

    let mut progress = iced_anim::Animated::new(initial, self.mode.clone());

    if initial != target {
      progress.set_target(target);
    }

    tree::State::new(SlideState {
      progress,
      announced: false,
    })
  }

  fn tag(&self) -> tree::Tag {
    tree::Tag::of::<SlideState>()
  }

  fn children(&self) -> Vec<Tree> {
    vec![Tree::new(&self.content)]
  }

  fn diff(&self, tree: &mut Tree) {
    let state = tree.state.downcast_mut::<SlideState>();
    let target = if self.closing { 0.0 } else { 1.0 };

    if self.disabled {
      state.progress.settle_at(target);
    } else if *state.progress.target() != target {
      state.progress.set_target(target);
    }

    if !self.closing {
      state.announced = false;
    }

    tree.diff_children(std::slice::from_ref(&self.content));
  }

  fn layout(
    &mut self,
    tree: &mut Tree,
    renderer: &iced::Renderer,
    limits: &layout::Limits,
  ) -> layout::Node {
    let shift = {
      let state = tree.state.downcast_ref::<SlideState>();
      1.0 - *state.progress.value()
    };

    let node = self
      .content
      .as_widget_mut()
      .layout(&mut tree.children[0], renderer, limits);

    node.translate(self.offset * shift)
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

    let state = tree.state.downcast_mut::<SlideState>();

    if !self.disabled {
      if let iced::Event::Window(window::Event::RedrawRequested(now)) = event {
        state.progress.tick(*now);
      }
    }

    if self.closing
      && !state.announced
      && *state.progress.target() <= 0.0
      && !state.progress.is_animating()
    {
      state.announced = true;

      if let Some(message) = &self.on_close {
        shell.publish(message.clone());
      }
    }

    if !self.disabled && state.progress.is_animating() {
      shell.request_redraw();
      shell.invalidate_layout();
    }
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
