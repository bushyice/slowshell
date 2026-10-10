use std::sync::Arc;

use iced::{
  Color, Element, Event, Length, mouse,
  widget::{Row, Space, column, container, row, space},
};
use iced_exwlshell::reexport::IcedId;
use slowshell_commons::tray::{SharedTrayState, TrayCmd, TrayPayload};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{EventFilter, ItemEffect, ItemMessage},
  types::{PayloadBox, ToUstr},
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{
  Backdrop, EventWrapper, Icon, Renderable, SizedPopup, clickable, hover_row, separator,
};
use system_tray::menu::{MenuItem, MenuType, ToggleState, ToggleType, TrayMenu};

const TICK: &str = "component/tray.tick";

pub struct TrayRenderable;

impl TrayRenderable {
  fn selected_menu(shared: &SharedTrayState) -> Option<(String, Option<String>, Arc<TrayMenu>)> {
    let address = shared.selected.lock().unwrap().clone()?;
    let menu = shared.menus.lock().unwrap().get(&address).cloned()?;
    let menu_path = shared
      .state
      .lock()
      .unwrap()
      .items
      .iter()
      .find(|i| i.address == address)
      .and_then(|i| i.menu_path.clone());
    Some((address, menu_path, menu))
  }

  fn activate_payload(address: &str, menu_path: Option<&str>, submenu_id: i32) -> PayloadBox {
    PayloadBox::new(TrayPayload {
      address: address.to_string(),
      menu_path: menu_path.map(String::from),
      submenu_id: submenu_id,
    })
  }
}

fn toggle_icon_name(item: &MenuItem) -> Option<&'static str> {
  match item.toggle_type {
    ToggleType::Checkmark => Some(match item.toggle_state {
      ToggleState::On => "checkbox-checked-symbolic",
      _ => "checkbox-symbolic",
    }),
    ToggleType::Radio => Some(match item.toggle_state {
      ToggleState::On => "radio-checked-symbolic",
      _ => "radio-symbolic",
    }),
    ToggleType::CannotBeToggled => None,
  }
}

fn tint(color: Color, opacity: f32) -> Color {
  Color::from_rgba(color.r, color.g, color.b, color.a * opacity)
}

fn menu_row<'a>(
  config: &'a Config,
  item: &MenuItem,
  address: &str,
  menu_path: Option<&str>,
  shared: SharedTrayState,
  id: IcedId,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();

  let has_submenu = !item.submenu.is_empty();
  let enabled = has_submenu || item.enabled;

  let row_spacing = config.sizing(&["menu", "row", "compact"]).gap;
  let checkbox_size = config
    .computed(&["menu", "checkbox"])
    .length("checkbox-size")
    .unwrap_or(14.0) as u16;
  let icon_size = config
    .computed(&["menu", "icon"])
    .length("icon-size")
    .unwrap_or(16.0) as u16;
  let arrow_size = config
    .computed(&["menu", "arrow-size"])
    .length("arrow-size")
    .unwrap_or(12.0) as u16;
  let list_indent = config
    .computed(&["menu", "list", "indent"])
    .keyword("list-indent")
    == Some("true");

  let color = if enabled {
    theme.text
  } else {
    tint(theme.subtext, 0.6)
  };

  let label = config
    .text(
      item
        .label
        .clone()
        .map(|l| l.replace("__", "_"))
        .unwrap_or_default(),
      &["menu"],
    )
    .color(color);

  let mut row = Row::new();

  if let Some(icon_name) = toggle_icon_name(item) {
    row = row.push(Icon::new(icon_name).size(checkbox_size).color(color));
  } else if let Some(ref name) = item.icon_name {
    row = row.push(Icon::new(name.clone()).size(icon_size).color(color));
  } else if let Some(ref raw_bytes) = item.icon_data {
    if !raw_bytes.is_empty() {
      let handle = Icon::<ItemMessage>::from_bytes(item.id, raw_bytes);
      row = row.push(
        iced::widget::image(handle)
          .width(Length::Fixed(icon_size as f32))
          .height(Length::Fixed(icon_size as f32)),
      );
    }
  } else if list_indent {
    row = row.push(
      space()
        .width(Length::Fixed(icon_size as f32))
        .height(Length::Fixed(icon_size as f32)),
    );
  }

  row = row.extend(vec![label.into(), Space::new().width(Length::Fill).into()]);

  if has_submenu {
    row = row.push(
      Icon::new("go-next-symbolic")
        .size(arrow_size)
        .color(theme.overlay),
    );
  }

  let row_elem: Element<'a, ItemMessage> = hover_row(
    config,
    &["menu", "row", "compact"],
    container(row.spacing(row_spacing).align_y(iced::Alignment::Center))
      .padding(config.sizing(&["menu", "row", "compact"]).padding())
      .width(Length::Fill),
  );

  if !enabled {
    return row_elem;
  }

  let address = address.to_string();
  let menu_path = menu_path.map(|s| s.to_string());
  let submenu_id = item.id;
  let children = item.submenu.clone();

  clickable(row_elem, move |_, _, _| {
    if has_submenu {
      if let Some(path) = &menu_path {
        let _ = shared.cmd_tx.send(TrayCmd::AboutToShow {
          address: address.clone(),
          menu_path: path.clone(),
          submenu_id,
        });
      }
      let mut nav = shared.nav.lock().unwrap();
      Arc::make_mut(&mut nav).push((submenu_id, children.clone()));
      Some(ItemMessage::Effect(id, ItemEffect::Redraw))
    } else {
      Some(ItemMessage::EffectAction(
        id,
        ItemEffect::Hide,
        ListenerAction::Payload {
          name: "tray.activate".to_ustr(),
          payload: Some(TrayRenderable::activate_payload(
            &address,
            menu_path.as_deref(),
            submenu_id,
          )),
        },
      ))
    }
  })
  .into()
  // EventWrapper::new(row_elem, move |event, layout, cursor| {
  //   if !cursor.is_over(layout.bounds()) {
  //     return None;
  //   }
  //   let Event::Mouse(mouse::Event::ButtonPressed(button)) = event else {
  //     return None;
  //   };
  //   if !matches!(button, mouse::Button::Left) {
  //     return None;
  //   }

  // })
  // .into()
}

fn back_row<'a>(
  config: &'a Config,
  shared: SharedTrayState,
  id: IcedId,
) -> Element<'a, ItemMessage> {
  let theme = config.palette();

  let font_size = config.sizing(&["menu"]).font_size;
  let row_spacing = config.sizing(&["menu", "row", "compact"]).gap;
  let color = theme.text;

  let content: Element<'a, ItemMessage> = hover_row(
    config,
    &["menu", "row", "compact"],
    container(
      row![
        Icon::new("go-previous-symbolic")
          .size(font_size as u16)
          .color(color),
        config.text("Back", &["menu"]),
        Space::new().width(Length::Fill),
      ]
      .spacing(row_spacing)
      .align_y(iced::Alignment::Center),
    )
    .padding(config.sizing(&["menu", "row", "compact"]).padding())
    .width(Length::Fill),
  );

  EventWrapper::new(content, move |event, layout, cursor| {
    if !cursor.is_over(layout.bounds()) {
      return None;
    }
    let Event::Mouse(mouse::Event::ButtonPressed(button)) = event else {
      return None;
    };
    if !matches!(button, mouse::Button::Left) {
      return None;
    }
    let mut nav = shared.nav.lock().unwrap();
    Arc::make_mut(&mut nav).pop();
    Some(ItemMessage::Effect(id, ItemEffect::Redraw))
  })
  .into()
}

impl Renderable for TrayRenderable {
  fn events(&self, _store: &Store) -> Vec<EventFilter> {
    vec![EventFilter::Named(TICK.into())]
  }

  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    data: &'a dyn std::any::Any,
  ) -> Element<'a, ItemMessage> {
    let data = data.downcast_ref::<PopupSettings>().unwrap();
    let theme = config.palette();

    let mut children: Vec<Element<'a, ItemMessage>> = Vec::new();

    if let Some(shared) = store.borrow::<SharedTrayState>() {
      let nav = shared.nav.lock().unwrap().clone();

      let selected = Self::selected_menu(shared);
      let empty_menu = Arc::new(TrayMenu {
        id: 0,
        submenus: Vec::new(),
      });
      let (address, menu_path, tray_menu) = match &selected {
        Some((address, menu_path, menu)) => (address.as_str(), menu_path.as_deref(), menu.clone()),
        None => ("", None, empty_menu),
      };

      let drilled = nav.last().is_some();
      let frame_items: &[MenuItem] = if let Some((_, items)) = nav.last() {
        items.as_slice()
      } else {
        tray_menu.submenus.as_slice()
      };

      if drilled {
        children.push(back_row(config, shared.clone(), id));
      }

      for menu_item in frame_items {
        if !menu_item.visible {
          continue;
        }
        if menu_item.menu_type == MenuType::Separator {
          children.push(
            separator()
              .color(
                config
                  .color(&["menu", "separator"], "color")
                  .unwrap_or(theme.base),
              )
              .into(),
          );
        } else {
          children.push(menu_row(
            config,
            menu_item,
            address,
            menu_path,
            shared.clone(),
            id,
          ));
        }
      }
    }

    if children.is_empty() {
      children.push(
        config
          .text("no menu", &["menu", "text-status", "text-faded"])
          .into(),
      );
    }

    let list_style = config.computed(&["menu", "list"]);
    let list_spacing = config.sizing(&["menu", "list"]).gap;
    let list_width = list_style.length("list-width").unwrap_or(220.0);
    let list = column(children)
      .spacing(list_spacing)
      .width(Length::Fixed(list_width));

    let inner_padding = config.sizing(&["menu", "inner"]).padding();
    let popup_style = config.container_cached(&["menu"]);

    SizedPopup::new(container(
      container(list).padding(inner_padding).style(popup_style),
    ))
    .with_width(120.)
    .with_backdrop(Backdrop::transparent().with_close_on_click(true))
    .with_position(data.position.0.get(store), data.position.1.get(store))
    .with_panel_edge(data.panel_edge)
    .with_panel_insets(store)
    .with_on_close(ItemMessage::Effect(id, ItemEffect::Hide))
    .into()
  }
}
