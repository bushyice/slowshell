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

pub fn icss_mode(easing: icss::widgets::animation::Easing, duration: Duration) -> Mode {
  let curve = match easing {
    icss::widgets::animation::Easing::Linear => iced_anim::transition::Curve::Linear,
    icss::widgets::animation::Easing::EaseIn => iced_anim::transition::Curve::EaseIn,
    icss::widgets::animation::Easing::EaseOut => iced_anim::transition::Curve::EaseOut,
    icss::widgets::animation::Easing::EaseInOut => iced_anim::transition::Curve::EaseInOut,
  };
  iced_anim::Easing::new(curve).with_duration(duration).into()
}

pub fn icss_edge_offset(edge: icss::widgets::animation::Edge, distance: f32) -> Vector {
  match edge {
    icss::widgets::animation::Edge::Top => Vector::new(0.0, -distance),
    icss::widgets::animation::Edge::Bottom => Vector::new(0.0, distance),
    icss::widgets::animation::Edge::Left => Vector::new(-distance, 0.0),
    icss::widgets::animation::Edge::Right => Vector::new(distance, 0.0),
  }
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
  initial: Option<T>,
  build: Box<dyn Fn(&T) -> Element<'a, ItemMessage> + 'a>,
  mode: Mode,
  animates_layout: bool,
  disabled: bool,
  generation: u64,
  cached: Element<'a, ItemMessage>,
}

impl<'a, T> Animated<'a, T>
where
  T: Animate + Clone + PartialEq + 'static,
{
  pub fn new(target: T, build: impl Fn(&T) -> Element<'a, ItemMessage> + 'a) -> Self {
    let cached = build(&target);
    Self {
      target,
      initial: None,
      build: Box::new(build),
      mode: quick(),
      animates_layout: false,
      disabled: !slowshell_core::animations::enabled(),
      generation: 0,
      cached,
    }
  }
  pub fn initial(mut self, initial: T) -> Self {
    self.initial = Some(initial);
    self
  }

  pub fn generation(mut self, generation: u64) -> Self {
    self.generation = generation;
    self
  }

  pub fn animation(mut self, mode: impl Into<Mode>) -> Self {
    self.mode = mode.into();
    self
  }

  pub fn with_icss_easing(
    mut self,
    easing: icss::widgets::animation::Easing,
    duration: Duration,
  ) -> Self {
    self.mode = icss_mode(easing, duration);
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

struct AnimatedWidgetState<T> {
  animation: iced_anim::Animated<T>,
  mode: Mode,
  generation: u64,
}

impl<'a, T> From<Animated<'a, T>> for Element<'a, ItemMessage>
where
  T: Animate + Clone + PartialEq + 'static,
{
  fn from(animated: Animated<'a, T>) -> Self {
    if animated.disabled {
      return (animated.build)(&animated.target);
    }

    Element::new(animated)
  }
}

impl<T> Widget<ItemMessage, iced::Theme, iced::Renderer> for Animated<'_, T>
where
  T: Animate + Clone + PartialEq + 'static,
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
    let initial = self.initial.clone().unwrap_or_else(|| self.target.clone());
    let mut animation = iced_anim::Animated::new(initial.clone(), self.mode);

    if initial != self.target {
      animation.set_target(self.target.clone());
    }

    tree::State::new(AnimatedWidgetState {
      animation,
      mode: self.mode,
      generation: self.generation,
    })
  }

  fn tag(&self) -> tree::Tag {
    tree::Tag::of::<AnimatedWidgetState<T>>()
  }

  fn children(&self) -> Vec<Tree> {
    vec![Tree::new(&self.cached)]
  }

  fn diff(&self, tree: &mut Tree) {
    let state = tree.state.downcast_mut::<AnimatedWidgetState<T>>();

    let restart = state.generation != self.generation || state.mode != self.mode;

    if restart {
      let initial = self.initial.clone().unwrap_or_else(|| self.target.clone());
      let mut animation = iced_anim::Animated::new(initial.clone(), self.mode);

      if initial != self.target {
        animation.set_target(self.target.clone());
      }

      state.animation = animation;
      state.mode = self.mode;
      state.generation = self.generation;
    } else if state.animation.target() != &self.target {
      state.animation.set_target(self.target.clone());
    }

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

    let iced::Event::Window(window::Event::RedrawRequested(now)) = event else {
      return;
    };

    let state = tree.state.downcast_mut::<AnimatedWidgetState<T>>();

    if state.animation.is_animating() {
      shell.request_redraw();

      if self.animates_layout {
        shell.invalidate_layout();
      }

      state.animation.tick(*now);
      self.cached = (self.build)(state.animation.value());
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

  pub fn with_icss_easing(
    mut self,
    easing: icss::widgets::animation::Easing,
    duration: Duration,
  ) -> Self {
    self.mode = icss_mode(easing, duration);
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
  base: Option<Color>,
  color: Color,
  radius: f32,
  border_width: f32,
  border_color: Color,
  mode: Mode,
  disabled: bool,
}

impl<'a> HoverBackground<'a> {
  pub fn new(content: impl Into<Element<'a, ItemMessage>>, color: Color, radius: f32) -> Self {
    Self {
      content: content.into(),
      base: None,
      color,
      radius,
      border_width: 0.0,
      border_color: Color::TRANSPARENT,
      mode: quick(),
      disabled: !slowshell_core::animations::enabled(),
    }
  }

  pub fn with_base(mut self, base: Color) -> Self {
    self.base = Some(base);
    self
  }

  pub fn with_border(mut self, width: f32, color: Color) -> Self {
    self.border_width = width;
    self.border_color = color;
    self
  }

  fn quad(&self, bounds: Rectangle) -> renderer::Quad {
    renderer::Quad {
      bounds,
      border: iced::Border {
        radius: self.radius.into(),
        width: self.border_width,
        color: self.border_color,
      },
      ..Default::default()
    }
  }

  pub fn animation(mut self, mode: impl Into<Mode>) -> Self {
    self.mode = mode.into();
    self
  }

  pub fn with_icss_easing(
    mut self,
    easing: icss::widgets::animation::Easing,
    duration: Duration,
  ) -> Self {
    self.mode = icss_mode(easing, duration);
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

    if self.base.is_some() || self.border_width > 0.0 {
      renderer.fill_quad(
        self.quad(layout.bounds()),
        self.base.unwrap_or(Color::TRANSPARENT),
      );
    }

    if alpha > 0.0 {
      renderer.fill_quad(
        self.quad(layout.bounds()),
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
  mode: Mode,
  generation: u64,
}

pub struct SlideIn<'a> {
  content: Element<'a, ItemMessage>,
  offset: Vector,
  closing: bool,
  mode: Mode,
  disabled: bool,
  generation: u64,
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
      generation: 0,
      on_close: None,
    }
  }

  pub fn from_edge(
    content: impl Into<Element<'a, ItemMessage>>,
    edge: icss::widgets::animation::Edge,
    distance: f32,
  ) -> Self {
    Self::new(content, icss_edge_offset(edge, distance))
  }

  pub fn closing(mut self, closing: bool) -> Self {
    self.closing = closing;
    self
  }

  pub fn animation(mut self, mode: impl Into<Mode>) -> Self {
    self.mode = mode.into();
    self
  }

  pub fn generation(mut self, generation: u64) -> Self {
    self.generation = generation;
    self
  }

  pub fn with_icss_easing(
    mut self,
    easing: icss::widgets::animation::Easing,
    duration: Duration,
  ) -> Self {
    self.mode = icss_mode(easing, duration);
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
    let initial = if self.disabled {
      target
    } else if self.closing {
      1.0
    } else {
      0.0
    };

    let mut progress = iced_anim::Animated::new(initial, self.mode);

    if initial != target {
      progress.set_target(target);
    }

    tree::State::new(SlideState {
      progress,
      announced: false,
      mode: self.mode,
      generation: self.generation,
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

    let restart = state.generation != self.generation || state.mode != self.mode;

    if self.disabled {
      state.progress.settle_at(target);
      state.generation = self.generation;
      state.mode = self.mode;
    } else if restart {
      let initial = if self.closing { 1.0 } else { 0.0 };
      let mut progress = iced_anim::Animated::new(initial, self.mode);

      if initial != target {
        progress.set_target(target);
      }

      state.progress = progress;
      state.generation = self.generation;
      state.mode = self.mode;
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
    let size = node.size();

    layout::Node::with_children(size, vec![node.translate(self.offset * shift)])
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
      child_layout(layout),
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
      child_layout(layout),
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
    self.content.as_widget_mut().operate(
      &mut tree.children[0],
      child_layout(layout),
      renderer,
      operation,
    );
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
      child_layout(layout),
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
      child_layout(layout),
      renderer,
      viewport,
      translation,
    )
  }
}

fn child_layout(layout: Layout<'_>) -> Layout<'_> {
  layout
    .children()
    .next()
    .expect("animated widget should have exactly one child")
}

struct CustomAnimState {
  progress: iced_anim::Animated<f32>,
  announced: bool,
  mode: Mode,
  generation: u64,
}

pub struct CustomAnimation<'a> {
  content: Element<'a, ItemMessage>,
  offset_fn: Box<dyn Fn(f32) -> Vector + 'a>,
  closing: bool,
  mode: Mode,
  disabled: bool,
  generation: u64,
  on_close: Option<ItemMessage>,
}

impl<'a> CustomAnimation<'a> {
  pub fn new(
    content: impl Into<Element<'a, ItemMessage>>,
    offset_fn: impl Fn(f32) -> Vector + 'a,
  ) -> Self {
    Self {
      content: content.into(),
      offset_fn: Box::new(offset_fn),
      closing: false,
      mode: quick(),
      disabled: !slowshell_core::animations::enabled(),
      generation: 0,
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

  pub fn generation(mut self, generation: u64) -> Self {
    self.generation = generation;
    self
  }

  pub fn with_icss_easing(
    mut self,
    easing: icss::widgets::animation::Easing,
    duration: Duration,
  ) -> Self {
    self.mode = icss_mode(easing, duration);
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

impl<'a> From<CustomAnimation<'a>> for Element<'a, ItemMessage> {
  fn from(widget: CustomAnimation<'a>) -> Self {
    Element::new(widget)
  }
}

impl Widget<ItemMessage, iced::Theme, iced::Renderer> for CustomAnimation<'_> {
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
    let initial = if self.disabled {
      target
    } else if self.closing {
      1.0
    } else {
      0.0
    };

    let mut progress = iced_anim::Animated::new(initial, self.mode);

    if initial != target {
      progress.set_target(target);
    }

    tree::State::new(CustomAnimState {
      progress,
      announced: false,
      mode: self.mode,
      generation: self.generation,
    })
  }

  fn tag(&self) -> tree::Tag {
    tree::Tag::of::<CustomAnimState>()
  }

  fn children(&self) -> Vec<Tree> {
    vec![Tree::new(&self.content)]
  }

  fn diff(&self, tree: &mut Tree) {
    let state = tree.state.downcast_mut::<CustomAnimState>();
    let target = if self.closing { 0.0 } else { 1.0 };

    let restart = state.generation != self.generation || state.mode != self.mode;

    if self.disabled {
      state.progress.settle_at(target);
      state.generation = self.generation;
      state.mode = self.mode;
    } else if restart {
      let initial = if self.closing { 1.0 } else { 0.0 };
      let mut progress = iced_anim::Animated::new(initial, self.mode);

      if initial != target {
        progress.set_target(target);
      }

      state.progress = progress;
      state.generation = self.generation;
      state.mode = self.mode;
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
    let offset = {
      let state = tree.state.downcast_ref::<CustomAnimState>();
      (self.offset_fn)(*state.progress.value())
    };

    let node = self
      .content
      .as_widget_mut()
      .layout(&mut tree.children[0], renderer, limits);
    let size = node.size();

    layout::Node::with_children(size, vec![node.translate(offset)])
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
      child_layout(layout),
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
      child_layout(layout),
      cursor,
      renderer,
      clipboard,
      shell,
      viewport,
    );

    let state = tree.state.downcast_mut::<CustomAnimState>();

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
    self.content.as_widget_mut().operate(
      &mut tree.children[0],
      child_layout(layout),
      renderer,
      operation,
    );
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
      child_layout(layout),
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
      child_layout(layout),
      renderer,
      viewport,
      translation,
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::time::Duration;

  #[test]
  fn test_icss_edge_offset() {
    assert_eq!(
      icss_edge_offset(icss::widgets::animation::Edge::Top, 50.0),
      Vector::new(0.0, -50.0)
    );
    assert_eq!(
      icss_edge_offset(icss::widgets::animation::Edge::Bottom, 50.0),
      Vector::new(0.0, 50.0)
    );
    assert_eq!(
      icss_edge_offset(icss::widgets::animation::Edge::Left, 80.0),
      Vector::new(-80.0, 0.0)
    );
    assert_eq!(
      icss_edge_offset(icss::widgets::animation::Edge::Right, 80.0),
      Vector::new(80.0, 0.0)
    );
  }

  #[test]
  fn test_icss_mode() {
    let mode = icss_mode(
      icss::widgets::animation::Easing::EaseOut,
      Duration::from_millis(200),
    );
    match mode {
      Mode::Transition(t) => {
        assert_eq!(t.duration, Duration::from_millis(200));
        assert_eq!(t.curve, iced_anim::transition::Curve::EaseOut);
      }
      _ => panic!("expected transition mode"),
    }
  }

  #[test]
  fn test_slide_in_from_edge() {
    let dummy = iced::widget::text("hello");
    let slide = SlideIn::from_edge(dummy, icss::widgets::animation::Edge::Left, 120.0)
      .with_icss_easing(
        icss::widgets::animation::Easing::EaseInOut,
        Duration::from_millis(300),
      );
    assert_eq!(slide.offset, Vector::new(-120.0, 0.0));
    match slide.mode {
      Mode::Transition(t) => {
        assert_eq!(t.duration, Duration::from_millis(300));
        assert_eq!(t.curve, iced_anim::transition::Curve::EaseInOut);
      }
      _ => panic!("expected transition mode"),
    }
  }

  #[test]
  fn test_slide_in_generation_restarts() {
    let slide: Element<'_, ItemMessage> =
      SlideIn::new(iced::widget::text("hello"), Vector::new(10.0, 0.0))
        .disabled(false)
        .generation(1)
        .into();
    let mut tree = Tree::new(&slide);

    assert!(
      tree
        .state
        .downcast_ref::<SlideState>()
        .progress
        .is_animating(),
      "a fresh slide-in should animate"
    );

    tree
      .state
      .downcast_mut::<SlideState>()
      .progress
      .settle_at(1.0);
    assert!(
      !tree
        .state
        .downcast_ref::<SlideState>()
        .progress
        .is_animating()
    );

    let same: Element<'_, ItemMessage> =
      SlideIn::new(iced::widget::text("hello"), Vector::new(10.0, 0.0))
        .disabled(false)
        .generation(1)
        .into();
    tree.diff(&same);
    assert!(
      !tree
        .state
        .downcast_ref::<SlideState>()
        .progress
        .is_animating(),
      "diffing with the same generation must not restart the animation"
    );

    let replay: Element<'_, ItemMessage> =
      SlideIn::new(iced::widget::text("hello"), Vector::new(10.0, 0.0))
        .disabled(false)
        .generation(2)
        .into();
    tree.diff(&replay);
    assert!(
      tree
        .state
        .downcast_ref::<SlideState>()
        .progress
        .is_animating(),
      "a new generation must restart the animation"
    );
  }
}
