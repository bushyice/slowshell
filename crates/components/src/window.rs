use iced::{
  Element, Length,
  widget::{container, row, text},
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

use crate::{Component, ComponentContext, ComponentOptions};

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

    if ctx.orientation == PanelOrientation::Vertical {
      let icon_elem: Element<'_, ItemMessage> = if show_icon {
        Icon::any([
          &window.class,
          "window-symbolic",
          "window",
          "application-x-executable-symbolic",
        ])
        .size(icon_size as u16)
        .color(color)
        .into()
      } else {
        row![].into()
      };
      return container(icon_elem).into();
    }

    let show_name = options.and_then(|o| o.bool("name")).unwrap_or(true);

    let show_title = options.and_then(|o| o.bool("title")).unwrap_or(true);

    let mut content = row![].spacing(6);

    if show_icon {
      content = content.push(
        Icon::any([
          &window.class,
          "window-symbolic",
          "window",
          "application-x-executable-symbolic",
        ])
        .size(icon_size as u16),
      );
    }

    let class = window.class.clone();
    let title = window.title.clone();

    if show_name {
      content = content.push(
        text(
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
        .color(color),
      );
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
      content =
        content.push(container(text(title).size(font_size).color(color)).width(Length::Shrink));
    }

    container(content).into()
  }
}
