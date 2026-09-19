use iced::{Element, mouse, widget::mouse_area};
use slowshell_config::Config;
use slowshell_core::{Store, listeners::ListenerAction, message::ItemMessage};
use slowshell_widgets::Icon;

use crate::{
  Component, ComponentContext, ComponentOptions, MenuConfig, menu_trigger, spaced_component,
};

#[derive(Default)]
pub struct IconComp {
  action: Option<ItemMessage>,
}

impl Component for IconComp {
  fn watch(&mut self, _store: &mut Store, options: Option<&ComponentOptions>) {
    self.action = options.and_then(|o| o.str("action")).map(|action| {
      ItemMessage::Action(
        options
          // .and_then(|o| o.table("payload"))
          // .map(|t| ListenerAction::Payload {
          //   name: action.into(),
          //   payload: Some(
          //     t.iter()
          //       .filter_map(|(key, value)| Some((key.to_ustr(), value.as_str()?.to_ustr())))
          //       .collect(),
          //   ),
          // })
          .map(|_| ListenerAction::Named(action.into()))
          .unwrap_or(ListenerAction::Named(action.into())),
      )
    })
  }

  fn view<'a>(
    &self,
    config: &Config,
    _store: &Store,
    ctx: &ComponentContext,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let style = config.style("component");
    let theme = &config.theme;
    let color = style.color(theme, "color", theme.text);
    let icon_size = style.number("icon.size").unwrap_or(14.0) as u16;

    let icon = if let Some(icons) = options
      .and_then(|o| o.table("icons"))
      .map(|x| x.iter().filter_map(|(_, v)| v.as_str()).collect::<Vec<_>>())
    {
      Icon::any(icons)
    } else if let Some(icon) = options.and_then(|o| o.str("icon")) {
      Icon::any([icon, "applications-system-symbolic"])
    } else {
      Icon::new("applications-system-symbolic")
    }
    .color(color)
    .size(icon_size)
    .into_element();

    if let Some(menu) = options.and_then(|o| o.str("menu")) {
      menu_trigger(
        mouse_area(icon)
          .interaction(mouse::Interaction::Pointer)
          .into(),
        MenuConfig {
          content: menu.into(),
          panel_name: None,
          position: ctx.position,
        },
        config,
        ctx,
        true,
      )
    } else if let Some(action) = &self.action {
      mouse_area(spaced_component(config, ctx, icon, true))
        .interaction(mouse::Interaction::Pointer)
        .on_press(action.clone())
        .into()
    } else {
      spaced_component(config, ctx, icon, false)
    }
  }
}
