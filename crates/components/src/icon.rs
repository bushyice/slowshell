use iced::{Element, mouse, widget::mouse_area};
use slowshell_config::Config;
use slowshell_core::{Store, listeners::ListenerAction, message::ItemMessage};
use slowshell_widgets::{Icon, clickable};

use crate::{
  Component, ComponentContext, ComponentOptions, MenuConfig, menu_trigger, spaced_component,
};

#[derive(Default)]
pub struct IconComp {
  action: Option<ItemMessage>,
  exec: Option<Vec<String>>,
  exec_alt: Option<Vec<String>>,
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
    });

    self.exec = options
      .and_then(|o| o.strings("exec"))
      .map(|v| v.into_iter().map(Into::into).collect());

    self.exec_alt = options
      .and_then(|o| o.strings("exec-alt"))
      .map(|v| v.into_iter().map(Into::into).collect());
  }

  fn view<'a>(
    &'a self,
    config: &'a Config,
    _store: &'a Store,
    ctx: &ComponentContext,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let style = config.computed(&["component"]);
    let theme = config.palette();
    let color = style
      .color("color")
      .map(|c| c.to_iced())
      .unwrap_or(theme.text);
    let icon_size = style.length("icon-size").unwrap_or(14.0) as u16;

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
    } else if let Some(exec) = &self.exec {
      let exec_alt = &self.exec_alt;

      clickable(
        mouse_area(spaced_component(config, ctx, icon, true))
          .interaction(mouse::Interaction::Pointer)
          .into(),
        move |event, _, _| {
          let iced::Event::Mouse(iced::advanced::mouse::Event::ButtonPressed(button)) = event
          else {
            return None;
          };

          let exec = if matches!(button, mouse::Button::Right) {
            if let Some(exec) = exec_alt {
              exec
            } else {
              return None;
            }
          } else {
            exec
          };

          use std::process::{Command, Stdio};

          if let Some((program, args)) = exec.split_first() {
            let _ = Command::new(program)
              .args(args)
              .stdin(Stdio::null())
              .stdout(Stdio::null())
              .stderr(Stdio::null())
              .spawn();
          }

          None
        },
      )
    } else {
      spaced_component(config, ctx, icon, false)
    }
  }
}
