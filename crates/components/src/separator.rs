use iced::{Color, Element};
use slowshell_commons::panels::PanelOrientation;
use slowshell_config::Config;
use slowshell_core::{Store, message::ItemMessage};
use slowshell_widgets::separator;

use crate::{Component, ComponentContext, ComponentOptions, spaced_component};

#[derive(Default)]
pub struct Separator;

impl Component for Separator {
  fn view<'a>(
    &self,
    config: &Config,
    _store: &Store,
    ctx: &ComponentContext,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let divider: Element<'a, ItemMessage> = separator()
      .vertical(ctx.orientation == PanelOrientation::Horizontal)
      .padding([0, 0])
      .circle(options.and_then(|x| x.bool("circle")).unwrap_or(false))
      .size(options.and_then(|x| x.number("size")).unwrap_or(2.0))
      .opacity(options.and_then(|x| x.number("opacity")).unwrap_or(0.1))
      .color(config.style("component").color(
        &config.theme,
        "color.faded",
        Color::from_rgb(0., 0., 0.),
      ))
      .into();

    spaced_component(config, ctx, divider, false)
  }
}
