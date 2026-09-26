use std::{
  cell::RefCell,
  collections::{HashMap, HashSet},
  sync::Arc,
  time::Duration,
};

use iced::{
  Alignment, Background, Color, ContentFit, Event, Length, Vector, keyboard, mouse,
  widget::{
    Space, column, container, image as image_widget, row, scrollable as scrollable_widget,
    scrollable::Direction as ScrollDirection, text, text_input,
  },
};
use icss::theme::{css::ast::PseudoClass, resolve::computed::ComputedStyle};
use slowshell_config::style::container_style;
use slowshell_core::{
  listeners::ListenerAction,
  message::{ItemEffect, ItemMessage},
  types::{PayloadBox, Ustr},
};
use slowshell_plugin_abi::{
  Align as AbiAlign, Animation, AnimationKind, Color as AbiColor, Easing as AbiEasing,
  Image as AbiImage, Length as AbiLength, Node, Padding as AbiPad, ROption, RString, RVec,
  Style as AbiStyle,
};
use slowshell_widgets::{Animated, Easing, HoverFlag, Icon, Mode, SlideIn, clickable};

use crate::{events::WidgetEvent, state::current_config};

#[derive(Clone)]
pub(crate) struct RenderCtx {
  pub id: Option<Ustr>,
  pub widget_event: Option<Ustr>,
}

pub(crate) fn render_nodes(node: &Node, ctx: RenderCtx) -> iced::Element<'static, ItemMessage> {
  image_cache_begin(ctx.widget_event.clone());
  let element = render_node(node, &ctx);
  image_cache_end();
  element
}

fn render_node(node: &Node, ctx: &RenderCtx) -> iced::Element<'static, ItemMessage> {
  match node {
    Node::Animated { child, animation } => render_animated(child, animation, ctx),
    _ => render_plain(node, ctx),
  }
}

fn render_plain(node: &Node, ctx: &RenderCtx) -> iced::Element<'static, ItemMessage> {
  match node {
    Node::Row {
      children,
      spacing,
      align,
      classes,
    } => {
      let body = row(render_children(children, ctx))
        .spacing(*spacing)
        .align_y(alignment(*align))
        .into();
      wrap_plain(body, classes)
    }

    Node::Column {
      children,
      spacing,
      align,
      classes,
    } => {
      let body = column(render_children(children, ctx))
        .spacing(*spacing)
        .align_x(alignment(*align))
        .into();
      wrap_plain(body, classes)
    }

    Node::Container {
      child,
      padding,
      align_x,
      align_y,
      style,
      width,
      height,
      classes,
    } => render_container(
      child, padding, *align_x, *align_y, style, *width, *height, classes, ctx,
    ),

    Node::Clickable {
      child,
      action,
      effect,
      classes,
    } => render_clickable(child, action, effect, classes, ctx),

    Node::Scrollable {
      child,
      width,
      height,
      report_scroll,
      classes,
    } => {
      let child = render_node(child, ctx);
      let scroll = scrollable_widget(child)
        .width(size(*width))
        .height(size(*height))
        .direction(scrollable_direction(*width, *height));
      let body = scroll_events(*report_scroll, ctx, scroll);
      wrap_plain(body, classes)
    }

    Node::Icon {
      name,
      size: icon_size,
      color,
      classes,
    } => {
      let style = node_style(classes);
      let mut icon = Icon::<ItemMessage>::new(name.to_string()).size((*icon_size).max(1.0) as u16);
      if let Some(color) = style.as_deref().and_then(text_color) {
        icon = icon.color(color);
      } else if let ROption::RSome(color) = color {
        icon = icon.color(to_color(*color));
      }
      wrap_plain(icon.into(), classes)
    }

    Node::Text {
      content,
      size: font_size,
      color,
      classes,
    } => {
      let style = node_style(classes);
      let color = style
        .as_deref()
        .and_then(text_color)
        .unwrap_or_else(|| match color {
          ROption::RSome(color) => to_color(*color),
          ROption::RNone => Color::WHITE,
        });
      let size = style
        .as_ref()
        .and_then(|style| style.length("font-size"))
        .unwrap_or(*font_size);
      let body = text(content.to_string()).size(size).color(color).into();
      wrap_plain(body, classes)
    }

    Node::Progress {
      value,
      color,
      classes,
    } => wrap_plain(progress_element(*value, color), classes),

    Node::Image {
      image,
      width,
      height,
      classes,
    } => wrap_plain(image_element(image, *width, *height), classes),

    Node::Canvas {
      cache_key,
      pixel_width,
      pixel_height,
      width,
      height,
      pixels,
      pool_id,
      classes,
    } => wrap_plain(
      canvas_element(
        *cache_key,
        *pixel_width,
        *pixel_height,
        *width,
        *height,
        *pool_id,
        pixels,
      ),
      classes,
    ),

    Node::TextInput {
      id,
      value,
      placeholder,
      secure,
      size: font_size,
      color,
      width,
      classes,
    } => {
      let body = text_input_element(
        id,
        value,
        placeholder,
        *secure,
        *font_size,
        color,
        *width,
        classes,
        ctx,
      );
      wrap_plain(body, classes)
    }

    Node::EventWrapper { child, id, classes } => {
      let child = render_node(child, ctx);
      let body = event_wrapper_element(id, ctx, child);
      wrap_plain(body, classes)
    }

    Node::Animated { child, animation } => render_animated(child, animation, ctx),
  }
}

fn render_animated(
  child: &Node,
  animation: &Animation,
  ctx: &RenderCtx,
) -> iced::Element<'static, ItemMessage> {
  let sliding = animation.kind == AnimationKind::Slide;

  if !animation.tween {
    let element = render_plain(child, ctx);
    return if sliding {
      slide_element(element, animation)
    } else {
      element
    };
  }

  let mode = animation_mode(animation);
  let element: iced::Element<'static, ItemMessage> = match child {
    Node::Progress { value, .. } => {
      let target = *value;
      let base = child.clone();
      let ctx = ctx.clone();
      Animated::new(target, move |value| {
        let mut local = base.clone();
        if let Node::Progress { value: target, .. } = &mut local {
          *target = *value;
        }
        render_plain(&local, &ctx)
      })
      .animation(mode)
      .animates_layout(true)
      .into()
    }

    Node::Text { color, .. } => {
      let target = match color {
        ROption::RSome(color) => to_color(*color),
        ROption::RNone => Color::WHITE,
      };
      let base = child.clone();
      let ctx = ctx.clone();
      Animated::new(target, move |color| {
        let mut local = base.clone();
        if let Node::Text { color: target, .. } = &mut local {
          *target = ROption::RSome(abi_color(*color));
        }
        render_plain(&local, &ctx)
      })
      .animation(mode)
      .into()
    }

    Node::Icon { color, .. } if matches!(color, ROption::RSome(_)) => {
      let target = match color {
        ROption::RSome(color) => to_color(*color),
        ROption::RNone => Color::WHITE,
      };
      let base = child.clone();
      let ctx = ctx.clone();
      Animated::new(target, move |color| {
        let mut local = base.clone();
        if let Node::Icon { color: target, .. } = &mut local {
          *target = ROption::RSome(abi_color(*color));
        }
        render_plain(&local, &ctx)
      })
      .animation(mode)
      .into()
    }

    _ => render_plain(child, ctx),
  };

  if sliding {
    slide_element(element, animation)
  } else {
    element
  }
}

fn render_children(
  children: &RVec<Node>,
  ctx: &RenderCtx,
) -> Vec<iced::Element<'static, ItemMessage>> {
  children
    .iter()
    .map(|child| render_node(child, ctx))
    .collect()
}

fn wrap_plain(
  body: iced::Element<'static, ItemMessage>,
  classes: &RVec<RString>,
) -> iced::Element<'static, ItemMessage> {
  let class_list = node_classes(classes);
  let computed = class_list.as_deref().and_then(computed_style);
  let mut wrapped = container(body);
  let mut hover_flag = None;

  if let Some(computed) = computed.as_deref()
    && let Some(base) = container_style(computed)
  {
    match class_list
      .as_deref()
      .and_then(|classes| node_hover_style(classes, base))
    {
      Some(hover) => {
        let flag = HoverFlag::default();
        wrapped = wrapped.style(flag.style(base, hover));
        hover_flag = Some(flag);
      }
      None => wrapped = wrapped.style(move |_| base),
    }
  }

  let element: iced::Element<'static, ItemMessage> = wrapped.into();
  match hover_flag {
    Some(flag) => flag.track(element),
    None => element,
  }
}

#[allow(clippy::too_many_arguments)]
fn render_container(
  child: &Node,
  padding: &AbiPad,
  align_x: AbiAlign,
  align_y: AbiAlign,
  style: &ROption<AbiStyle>,
  width: AbiLength,
  height: AbiLength,
  classes: &RVec<RString>,
  ctx: &RenderCtx,
) -> iced::Element<'static, ItemMessage> {
  let body = render_node(child, ctx);
  let padding = iced::Padding {
    top: padding.top,
    right: padding.right,
    bottom: padding.bottom,
    left: padding.left,
  };

  let mut wrapped = container(body)
    .padding(padding)
    .align_x(alignment(align_x))
    .align_y(alignment(align_y))
    .width(size(width))
    .height(size(height));

  let mut hover_flag = None;
  let class_list = node_classes(classes);
  let computed = class_list.as_deref().and_then(computed_style);

  if let Some(computed) = computed.as_deref()
    && let Some(base) = container_style(computed)
  {
    match class_list
      .as_deref()
      .and_then(|classes| node_hover_style(classes, base))
    {
      Some(hover) => {
        let flag = HoverFlag::default();
        wrapped = wrapped.style(flag.style(base, hover));
        hover_flag = Some(flag);
      }
      None => wrapped = wrapped.style(move |_| base),
    }
  } else if let ROption::RSome(style) = style {
    let style = *style;
    wrapped = wrapped.style(move |_theme| inline_container_style(style));
  }

  let element: iced::Element<'static, ItemMessage> = wrapped.into();
  match hover_flag {
    Some(flag) => flag.track(element),
    None => element,
  }
}

fn render_clickable(
  child: &Node,
  action: &ROption<RString>,
  effect: &ROption<[usize; 4]>,
  classes: &RVec<RString>,
  ctx: &RenderCtx,
) -> iced::Element<'static, ItemMessage> {
  let body = render_node(child, ctx);
  let mut wrapped = container(body);
  let mut hover_flag = None;

  let class_list = node_classes(classes);
  let computed = class_list.as_deref().and_then(computed_style);
  if let Some(computed) = computed.as_deref()
    && let Some(base) = container_style(computed)
  {
    match class_list
      .as_deref()
      .and_then(|classes| node_hover_style(classes, base))
    {
      Some(hover) => {
        let flag = HoverFlag::default();
        wrapped = wrapped.style(flag.style(base, hover));
        hover_flag = Some(flag);
      }
      None => wrapped = wrapped.style(move |_| base),
    }
  }

  let command = match action {
    ROption::RSome(action) => action.to_string(),
    ROption::RNone => String::new(),
  };
  let effect = match effect {
    ROption::RSome(effect) => Some(*effect),
    ROption::RNone => None,
  };

  let effect_target = ctx.id.as_ref().map(|_| iced::window::Id::unique());
  let effect = effect.filter(|_| effect_target.is_some());

  let element: iced::Element<'static, ItemMessage> = if effect.is_none() && command.is_empty() {
    wrapped.into()
  } else {
    clickable(wrapped.into(), move |_, _, _| {
      let action = (!command.is_empty()).then(|| ListenerAction::Payload {
        name: Ustr::from(command.as_str()),
        payload: None,
      });

      if let (Some(effect), Some(target)) = (effect, effect_target) {
        return Some(match action {
          Some(action) => ItemMessage::EffectAction(target, ItemEffect::Custom(effect), action),
          None => ItemMessage::Effect(target, ItemEffect::Custom(effect)),
        });
      }

      action.map(ItemMessage::Action)
    })
  };

  match hover_flag {
    Some(flag) => flag.track(element),
    None => element,
  }
}

#[allow(clippy::too_many_arguments)]
fn text_input_element(
  id: &RString,
  value: &RString,
  placeholder: &RString,
  secure: bool,
  font_size: f32,
  color: &ROption<AbiColor>,
  width: AbiLength,
  classes: &RVec<RString>,
  ctx: &RenderCtx,
) -> iced::Element<'static, ItemMessage> {
  let style = node_style(classes);
  let color = style
    .as_deref()
    .and_then(text_color)
    .or_else(|| match color {
      ROption::RSome(color) => Some(to_color(*color)),
      ROption::RNone => None,
    });
  let widget_event = ctx.widget_event.clone();

  let mut input = text_input(placeholder.as_str(), value.as_str())
    .width(size(width))
    .secure(secure);

  let font_size = style
    .as_ref()
    .and_then(|style| style.length("font-size"))
    .or_else(|| (font_size > 0.0).then_some(font_size));
  if let Some(font_size) = font_size {
    input = input.size(font_size);
  }

  if let Some(widget_event) = widget_event {
    let input_id = id.to_string();
    let input_event = widget_event.clone();
    input = input.on_input(move |value| {
      widget_event_message(&input_event, WidgetEvent::input(input_id.clone(), value))
    });

    let submit_id = id.to_string();
    input = input.on_submit(widget_event_message(
      &widget_event,
      WidgetEvent::submit(submit_id, value.to_string()),
    ));
  }

  if let Some(color) = color {
    input = input.style(move |_theme, _status| text_input::Style {
      value: color,
      ..text_input::default(_theme, _status)
    });
  }

  input.into()
}

fn event_wrapper_element(
  id: &RString,
  ctx: &RenderCtx,
  child: iced::Element<'static, ItemMessage>,
) -> iced::Element<'static, ItemMessage> {
  let Some(widget_event) = ctx.widget_event.clone() else {
    return child;
  };

  let id = id.to_string();
  slowshell_widgets::EventWrapper::new(child, move |event, layout, cursor| {
    encode_wrapper_event(&widget_event, &id, event, layout.bounds(), cursor)
  })
  .into()
}

fn encode_wrapper_event(
  event_name: &Ustr,
  id: &str,
  event: &Event,
  bounds: iced::Rectangle,
  cursor: mouse::Cursor,
) -> Option<ItemMessage> {
  let value = match event {
    Event::Keyboard(keyboard::Event::KeyPressed {
      physical_key,
      text,
      modifiers,
      repeat,
      ..
    }) => {
      let code = physical_key_code(physical_key);
      let text = text.as_deref().unwrap_or("");
      let repeat = u32::from(*repeat);
      let mods = modifiers.bits();
      format!("keydown {code} {repeat} {mods} {text}")
    }
    Event::Keyboard(keyboard::Event::KeyReleased {
      physical_key,
      modifiers,
      ..
    }) => {
      let code = physical_key_code(physical_key);
      let mods = modifiers.bits();
      format!("keyup {code} {mods}")
    }
    Event::Mouse(mouse::Event::ButtonPressed(button)) if cursor.is_over(bounds) => {
      format!("mousedown {}", mouse_button_code(*button))
    }
    Event::Mouse(mouse::Event::ButtonReleased(button)) if cursor.is_over(bounds) => {
      format!("mouseup {}", mouse_button_code(*button))
    }
    Event::Mouse(mouse::Event::CursorMoved { position }) if cursor.is_over(bounds) => {
      format!("mousemove {} {}", position.x, position.y)
    }
    Event::Mouse(mouse::Event::WheelScrolled { delta }) if cursor.is_over(bounds) => match delta {
      mouse::ScrollDelta::Lines { x, y } => format!("wheel {x} {y} lines"),
      mouse::ScrollDelta::Pixels { x, y } => format!("wheel {x} {y} pixels"),
    },
    Event::Mouse(mouse::Event::CursorEntered) => "enter".to_owned(),
    Event::Mouse(mouse::Event::CursorLeft) => "exit".to_owned(),
    _ => return None,
  };

  Some(widget_event_message(
    event_name,
    WidgetEvent::custom(id.to_owned(), value),
  ))
}

fn physical_key_code(physical: &keyboard::key::Physical) -> u32 {
  match physical {
    keyboard::key::Physical::Code(code) => *code as u32,
    keyboard::key::Physical::Unidentified(native) => match native {
      keyboard::key::NativeCode::Android(code) | keyboard::key::NativeCode::Xkb(code) => *code,
      keyboard::key::NativeCode::MacOS(code) | keyboard::key::NativeCode::Windows(code) => {
        *code as u32
      }
      keyboard::key::NativeCode::Unidentified => u32::MAX,
    },
  }
}

fn mouse_button_code(button: mouse::Button) -> u32 {
  match button {
    mouse::Button::Left => 0,
    mouse::Button::Right => 1,
    mouse::Button::Middle => 2,
    mouse::Button::Back => 3,
    mouse::Button::Forward => 4,
    mouse::Button::Other(code) => 100 + code as u32,
  }
}

fn scrollable_direction(width: AbiLength, height: AbiLength) -> ScrollDirection {
  let horizontal = width != AbiLength::Shrink;
  let vertical = height != AbiLength::Shrink;

  match (horizontal, vertical) {
    (true, true) => ScrollDirection::Both {
      vertical: scrollable_widget::Scrollbar::default(),
      horizontal: scrollable_widget::Scrollbar::default(),
    },
    (true, false) => ScrollDirection::Horizontal(scrollable_widget::Scrollbar::default()),
    _ => ScrollDirection::Vertical(scrollable_widget::Scrollbar::default()),
  }
}

fn scroll_events(
  report_scroll: bool,
  ctx: &RenderCtx,
  scroll: scrollable_widget::Scrollable<'static, ItemMessage>,
) -> iced::Element<'static, ItemMessage> {
  if !report_scroll {
    return scroll.into();
  }

  let Some(widget_event) = ctx.widget_event.clone() else {
    return scroll.into();
  };

  let id = String::new();
  scroll
    .on_scroll(move |viewport| {
      let offset = viewport.absolute_offset();
      widget_event_message(
        &widget_event,
        WidgetEvent::scroll(id.clone(), [offset.x, offset.y]),
      )
    })
    .into()
}

fn progress_element(value: f32, color: &ROption<AbiColor>) -> iced::Element<'static, ItemMessage> {
  let value = value.clamp(0.0, 1.0);
  let color = match color {
    ROption::RSome(color) => to_color(*color),
    ROption::RNone => Color::WHITE,
  };

  let filled = (value * 100.0).round().max(1.0) as u16;
  let empty = ((1.0 - value) * 100.0).round().max(1.0) as u16;

  iced::widget::row![
    container(Space::new())
      .width(Length::FillPortion(filled))
      .height(Length::Fixed(4.0))
      .style(move |_theme| iced::widget::container::Style {
        background: Some(color.into()),
        ..Default::default()
      }),
    container(Space::new())
      .width(Length::FillPortion(empty))
      .height(Length::Fixed(4.0)),
  ]
  .width(Length::Fixed(48.0))
  .into()
}

fn canvas_element(
  cache_key: u64,
  pixel_width: u32,
  pixel_height: u32,
  width: AbiLength,
  height: AbiLength,
  pool_id: u64,
  pixels: &RVec<u8>,
) -> iced::Element<'static, ItemMessage> {
  if pixel_width == 0 || pixel_height == 0 {
    return iced::widget::space().into();
  }

  let expected = pixel_width as usize * pixel_height as usize * 4;

  if cache_key != 0
    && let Some(handle) = image_cache_get(cache_key)
  {
    return image_widget(handle)
      .width(size(width))
      .height(size(height))
      .content_fit(ContentFit::Fill)
      .into();
  }

  let handle = if pool_id != 0 {
    crate::state::canvas_with(pool_id, |bytes| {
      (bytes.len() >= expected).then(|| {
        image_widget::Handle::from_rgba(pixel_width, pixel_height, bytes[..expected].to_vec())
      })
    })
    .flatten()
  } else {
    if pixels.len() < expected {
      eprintln!(
        "[plugin] canvas is {} bytes, expected {expected}",
        pixels.len()
      );
      return iced::widget::space().into();
    }
    Some(image_widget::Handle::from_rgba(
      pixel_width,
      pixel_height,
      pixels[..expected].to_vec(),
    ))
  };

  let Some(handle) = handle else {
    return iced::widget::space().into();
  };

  if cache_key != 0 {
    image_cache_put(cache_key, handle.clone());
  }

  image_widget(handle)
    .width(size(width))
    .height(size(height))
    .content_fit(ContentFit::Fill)
    .into()
}

fn image_element(
  image: &AbiImage,
  width: AbiLength,
  height: AbiLength,
) -> iced::Element<'static, ItemMessage> {
  let handle = match image {
    AbiImage::Rgba {
      cache_key,
      width: image_width,
      height: image_height,
      pixels,
    } => {
      if pixels.is_empty() || *image_width == 0 || *image_height == 0 {
        return iced::widget::space().into();
      }

      let expected = *image_width as usize * *image_height as usize * 4;
      if pixels.len() < expected {
        eprintln!(
          "[plugin] raw RGBA image is {} bytes, expected {expected}",
          pixels.len()
        );
        return iced::widget::space().into();
      }

      if *cache_key != 0
        && let Some(handle) = image_cache_get(*cache_key)
      {
        return image_widget(handle)
          .width(size(width))
          .height(size(height))
          .into();
      }

      let handle =
        image_widget::Handle::from_rgba(*image_width, *image_height, pixels[..expected].to_vec());
      if *cache_key != 0 {
        image_cache_put(*cache_key, handle.clone());
      }
      handle
    }

    AbiImage::Encoded { cache_key, bytes } => {
      if bytes.is_empty() {
        return iced::widget::space().into();
      }

      let key = if *cache_key == 0 {
        image_hash_key(bytes)
      } else {
        *cache_key
      };
      if let Some(handle) = image_cache_get(key) {
        return image_widget(handle)
          .width(size(width))
          .height(size(height))
          .into();
      }

      match ::image::load_from_memory(bytes) {
        Ok(decoded) => {
          let rgba = decoded.to_rgba8();
          let (width, height) = rgba.dimensions();
          let handle = image_widget::Handle::from_rgba(width, height, rgba.into_raw());
          image_cache_put(key, handle.clone());
          handle
        }
        Err(err) => {
          eprintln!("[plugin] failed to decode image: {err}");
          return iced::widget::space().into();
        }
      }
    }

    AbiImage::Path(path) => image_widget::Handle::from_path(path.as_str()),
  };

  image_widget(handle)
    .width(size(width))
    .height(size(height))
    .into()
}

#[derive(Default)]
struct ImageCache {
  scope: Option<Ustr>,
  entries: HashMap<(Option<Ustr>, u64), image_widget::Handle>,
  seen: HashSet<(Option<Ustr>, u64)>,
}

thread_local! {
  static IMAGE_CACHE: RefCell<ImageCache> = RefCell::new(ImageCache::default());
}

fn image_cache_begin(scope: Option<Ustr>) {
  IMAGE_CACHE.with(|cache| {
    let mut cache = cache.borrow_mut();
    cache.scope = scope;
    cache.seen.clear();
  });
}

fn image_cache_end() {
  IMAGE_CACHE.with(|cache| {
    let mut cache = cache.borrow_mut();
    let ImageCache {
      scope,
      entries,
      seen,
    } = &mut *cache;
    let scope = scope.clone();
    entries.retain(|key, _| key.0 != scope || seen.contains(key));
  });
}

fn image_cache_get(key: u64) -> Option<image_widget::Handle> {
  IMAGE_CACHE.with(|cache| {
    let mut cache = cache.borrow_mut();
    let id = (cache.scope.clone(), key);
    cache.seen.insert(id.clone());
    cache.entries.get(&id).cloned()
  })
}

fn image_cache_put(key: u64, handle: image_widget::Handle) {
  IMAGE_CACHE.with(|cache| {
    let mut cache = cache.borrow_mut();
    let id = (cache.scope.clone(), key);
    cache.seen.insert(id.clone());
    cache.entries.insert(id, handle);
  });
}

fn image_hash_key(bytes: &[u8]) -> u64 {
  let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
  for byte in bytes {
    hash ^= u64::from(*byte);
    hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
  }
  hash | (1u64 << 63)
}

fn widget_event_message(event_name: &Ustr, event: WidgetEvent) -> ItemMessage {
  ItemMessage::Action(ListenerAction::Payload {
    name: event_name.clone(),
    payload: Some(PayloadBox::new(event)),
  })
}

fn to_color(color: AbiColor) -> Color {
  Color::from_rgba(color.r, color.g, color.b, color.a)
}

fn abi_color(color: Color) -> AbiColor {
  AbiColor {
    r: color.r,
    g: color.g,
    b: color.b,
    a: color.a,
  }
}

fn alignment(align: AbiAlign) -> Alignment {
  match align {
    AbiAlign::Center => Alignment::Center,
    AbiAlign::End => Alignment::End,
    _ => Alignment::Start,
  }
}

fn size(length: AbiLength) -> Length {
  match length {
    AbiLength::Fill => Length::Fill,
    AbiLength::Fixed(value) => Length::Fixed(value),
    AbiLength::Shrink => Length::Shrink,
  }
}

fn animation_mode(animation: &Animation) -> Mode {
  let easing = match animation.easing {
    AbiEasing::Linear => Easing::LINEAR,
    AbiEasing::Ease => Easing::EASE,
    AbiEasing::EaseIn => Easing::EASE_IN,
    AbiEasing::EaseOut => Easing::EASE_OUT,
    AbiEasing::EaseInOut => Easing::EASE_IN_OUT,
  };

  easing
    .with_duration(Duration::from_millis(animation.duration_ms.max(1) as u64))
    .into()
}

fn slide_element(
  element: iced::Element<'static, ItemMessage>,
  animation: &Animation,
) -> iced::Element<'static, ItemMessage> {
  let offset = Vector::new(animation.offset_x, animation.offset_y);
  SlideIn::new(element, offset)
    .animation(animation_mode(animation))
    .into()
}

fn node_classes(classes: &RVec<RString>) -> Option<Vec<&str>> {
  let mut result = Vec::new();
  for class in classes.iter() {
    result.extend(class.split_whitespace());
  }
  (!result.is_empty()).then_some(result)
}

fn computed_style(classes: &[&str]) -> Option<Arc<ComputedStyle>> {
  let config = current_config()?;
  Some(config.computed(classes))
}

fn node_style(classes: &RVec<RString>) -> Option<Arc<ComputedStyle>> {
  let classes = node_classes(classes)?;
  computed_style(&classes)
}

fn node_hover_style(
  classes: &[&str],
  base: iced::widget::container::Style,
) -> Option<iced::widget::container::Style> {
  let config = current_config()?;
  let hover = config.container_style_with_pseudo(classes, Some(PseudoClass::Hover));
  (hover != base).then_some(hover)
}

fn text_color(style: &ComputedStyle) -> Option<Color> {
  style.color("color").map(|color| color.to_iced())
}

fn inline_container_style(style: AbiStyle) -> iced::widget::container::Style {
  let background = match style.background {
    ROption::RSome(color) => Some(Background::Color(to_color(color))),
    ROption::RNone => None,
  };
  let text_color = match style.text_color {
    ROption::RSome(color) => Some(to_color(color)),
    ROption::RNone => None,
  };
  let border = match style.border {
    ROption::RSome(border) => iced::Border {
      color: to_color(border.color),
      width: border.width,
      radius: border.radius.into(),
    },
    ROption::RNone => iced::Border::default(),
  };

  iced::widget::container::Style {
    background,
    border,
    text_color,
    ..Default::default()
  }
}
