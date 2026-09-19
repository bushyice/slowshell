use iced::{
  Element,
  widget::{container, row, text, text::Wrapping},
};
use slowshell_commons::desktop::DesktopEntries;
use slowshell_commons::panels::PanelOrientation;
use slowshell_compositor::CompositorStore;
use slowshell_config::Config;
use slowshell_core::{
  Store,
  message::{EventFilter, ItemMessage},
};
use slowshell_widgets::{Icon, separator};

use crate::{Component, ComponentContext, ComponentOptions, spaced_component};

#[derive(Default)]
pub struct Window;

impl Component for Window {
  fn events(&self) -> Vec<EventFilter> {
    vec![EventFilter::UpdateCompositor]
  }

  fn check_view(&self, store: &Store, _options: Option<&ComponentOptions>) -> bool {
    let Some(Ok(state)) = store.borrow::<CompositorStore>().map(|x| x.state()) else {
      return false;
    };

    state.active_window.is_some()
  }

  fn view<'a>(
    &self,
    config: &Config,
    store: &Store,
    ctx: &ComponentContext,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let Some(Ok(state)) = store.borrow::<CompositorStore>().map(|x| x.state()) else {
      return row![].into();
    };

    let Some(window) = state.active_window.as_ref() else {
      return row![].into();
    };

    let show_icon = options.and_then(|o| o.bool("icon")).unwrap_or(true);

    let style = ctx.resolve_style(config);
    let theme = &config.theme;

    let font_size = style.number("font.size").unwrap_or(13.0);
    let icon_size = style.number("icon.size").unwrap_or(14.0);
    let color = style.color(&theme, "color", theme.text);
    let name_width = options
      .and_then(|o| o.number("name-width"))
      .unwrap_or(180.0);
    let title_width = options
      .and_then(|o| o.number("title-width"))
      .unwrap_or(320.0);

    if ctx.orientation == PanelOrientation::Vertical {
      let icon_elem: Element<'_, ItemMessage> = if show_icon {
        window_icon(window, icon_size, color)
      } else {
        row![].into()
      };
      return spaced_component(config, ctx, icon_elem, false);
    }

    let show_name = options.and_then(|o| o.bool("name")).unwrap_or(true);

    let show_title = options.and_then(|o| o.bool("title")).unwrap_or(true);

    let mut content = row![].spacing(6);

    if show_icon {
      content = content.push(window_icon(window, icon_size, color));
    }

    let class = window.class.clone();
    let title = window.title.clone();

    if show_name {
      let name = text(
        if let Some(name) = store
          .borrow::<DesktopEntries>()
          .map(|d| d.resolve_name(&class))
        {
          name.to_string()
        } else {
          class
        },
      )
      .size(font_size)
      .color(color)
      .wrapping(Wrapping::None);

      content = content.push(clipped(name, name_width));
    }

    if show_title {
      if show_name {
        content = content.push(
          separator()
            .circle(true)
            .vertical(true)
            .opacity(0.3)
            .size(5.0),
        );
      }
      content = content.push(clipped(
        text(title)
          .size(font_size)
          .color(color)
          .wrapping(Wrapping::None),
        title_width,
      ));
    }

    spaced_component(config, ctx, content.into(), false)
  }
}

fn clipped<'a>(
  content: impl Into<Element<'a, ItemMessage>>,
  max_width: f32,
) -> Element<'a, ItemMessage> {
  if max_width > 0.0 {
    container(content).max_width(max_width).clip(true).into()
  } else {
    content.into()
  }
}

fn window_icon<'a>(
  window: &slowshell_core::Window,
  size: f32,
  color: iced::Color,
) -> Element<'a, ItemMessage> {
  let icon = Icon::any([
    &window.class,
    "window",
    "application-x-executable",
    "window-symbolic",
    "application-x-executable-symbolic",
  ])
  .size(size as u16);

  let symbolic = icon
    .resolved_name()
    .is_some_and(|name| name.contains("symbolic"));

  if symbolic {
    icon.color(color).into()
  } else {
    icon.into()
  }
}
