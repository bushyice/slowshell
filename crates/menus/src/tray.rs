use iced::{
  Color, Element, Event, Length, mouse,
  widget::{Row, Space, column, container, row, space, text},
};
use iced_layershell::reexport::IcedId;
use slowshell_commons::tray::{SharedTrayState, TrayCmd, TrayPayload};
use slowshell_config::{Config, style::Style};
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{ItemEffect, ItemMessage},
  types::{PayloadBox, ToUstr},
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{
  Backdrop, EventWrapper, Icon, Renderable, SizedPopup, clickable, separator,
};
use system_tray::menu::{MenuItem, MenuType, ToggleState, ToggleType, TrayMenu};

pub struct TrayRenderable;

impl TrayRenderable {
  fn selected_menu(shared: &SharedTrayState) -> Option<(String, Option<String>, TrayMenu)> {
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

  fn row_padding(style: &Style) -> [f32; 2] {
    [
      style.number("row.padding.y").unwrap_or(7.0),
      style.number("row.padding.x").unwrap_or(12.0),
    ]
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
  item: &MenuItem,
  address: &str,
  menu_path: Option<&str>,
  shared: SharedTrayState,
  id: IcedId,
  style: &Style,
  theme: &slowshell_config::style::Theme,
) -> Element<'a, ItemMessage> {
  let has_submenu = !item.submenu.is_empty();
  let enabled = has_submenu || item.enabled;

  let font_size = style.number("font.size").unwrap_or(13.0);
  let row_radius = style.number("row.radius").unwrap_or(6.0);
  let row_padding = TrayRenderable::row_padding(style);
  let row_spacing = style.number("row.spacing").unwrap_or(8.0);
  let checkbox_size = style.number("checkbox.size").unwrap_or(14.0) as u16;
  let icon_size = style.number("icon.size").unwrap_or(16.0) as u16;
  let arrow_size = style.number("arrow.size").unwrap_or(12.0) as u16;
  let bg = style.color(theme, "row.background", Color::TRANSPARENT);

  let color = if enabled {
    style.color(theme, "color", theme.text)
  } else {
    tint(style.color(theme, "color.faded", theme.subtext), 0.6)
  };

  let label = text(
    item
      .label
      .clone()
      .map(|l| l.replace("__", "_"))
      .unwrap_or_default(),
  )
  .size(font_size)
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
  } else if style.bool("list.indent") {
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
        .color(style.color(theme, "arrow.color", theme.overlay)),
    );
  }

  let row_elem: Element<'a, ItemMessage> =
    container(row.spacing(row_spacing).align_y(iced::Alignment::Center))
      .padding(row_padding)
      .width(Length::Fill)
      .style(move |_t: &iced::Theme| container::Style {
        background: Some(bg.into()),
        border: iced::Border {
          radius: row_radius.into(),
          ..Default::default()
        },
        ..container::Style::default()
      })
      .into();

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
      shared
        .nav
        .lock()
        .unwrap()
        .push((submenu_id, children.clone()));
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
  shared: SharedTrayState,
  id: IcedId,
  style: &Style,
  theme: &slowshell_config::style::Theme,
) -> Element<'a, ItemMessage> {
  let font_size = style.number("font.size").unwrap_or(13.0);
  let row_radius = style.number("row.radius").unwrap_or(6.0);
  let row_padding = TrayRenderable::row_padding(style);
  let row_spacing = style.number("row.spacing").unwrap_or(8.0);
  let bg = style.color(theme, "row.background", Color::TRANSPARENT);
  let color = style.color(theme, "color", theme.text);

  let content: Element<'a, ItemMessage> = container(
    row![
      Icon::new("go-previous-symbolic")
        .size(font_size as u16)
        .color(color),
      text("Back").size(font_size).color(color),
      Space::new().width(Length::Fill),
    ]
    .spacing(row_spacing)
    .align_y(iced::Alignment::Center),
  )
  .padding(row_padding)
  .width(Length::Fill)
  .style(move |_t: &iced::Theme| container::Style {
    background: Some(bg.into()),
    border: iced::Border {
      radius: row_radius.into(),
      ..Default::default()
    },
    ..container::Style::default()
  })
  .into();

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
    shared.nav.lock().unwrap().pop();
    Some(ItemMessage::Effect(id, ItemEffect::Redraw))
  })
  .into()
}

impl Renderable for TrayRenderable {
  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    data: &'a dyn std::any::Any,
  ) -> Element<'a, ItemMessage> {
    let data = data.downcast_ref::<PopupSettings>().unwrap();
    let style = config.style("menu");
    let theme = &config.theme;

    let mut children: Vec<Element<'a, ItemMessage>> = Vec::new();

    if let Some(shared) = store.borrow::<SharedTrayState>() {
      let nav = shared.nav.lock().unwrap().clone();

      let (address, menu_path, frame_items, drilled) = if let Some((_, items)) = nav.last() {
        let (address, menu_path, _) = Self::selected_menu(shared).unwrap_or((
          String::new(),
          None,
          system_tray::menu::TrayMenu {
            id: 0,
            submenus: Vec::new(),
          },
        ));
        (address, menu_path, items.clone(), true)
      } else if let Some((address, menu_path, tray_menu)) = Self::selected_menu(shared) {
        (address, menu_path, tray_menu.submenus, false)
      } else {
        (String::new(), None, Vec::new(), false)
      };

      if drilled {
        children.push(back_row(shared.clone(), id, &style, theme));
      }

      for menu_item in &frame_items {
        if !menu_item.visible {
          continue;
        }
        if menu_item.menu_type == MenuType::Separator {
          children.push(
            separator()
              .color(style.color(theme, "separator.color", theme.overlay))
              .into(),
          );
        } else {
          children.push(menu_row(
            menu_item,
            &address,
            menu_path.as_deref(),
            shared.clone(),
            id,
            &style,
            theme,
          ));
        }
      }
    }

    if children.is_empty() {
      children.push(
        text("no menu")
          .size(style.number("status.font.size").unwrap_or(11.0))
          .color(style.color(theme, "color.faded", theme.subtext))
          .into(),
      );
    }

    let list_spacing = style.number("list.spacing").unwrap_or(4.0);
    let list_width = style.number("list.width").unwrap_or(220.0);
    let list = column(children)
      .spacing(list_spacing)
      .width(Length::Fixed(list_width));

    let inner_padding = style.number("inner.padding").unwrap_or(6.0);
    let mut popup_style = style.container_style(theme);
    if let (Some(x), Some(y)) = (
      style.number("shadow.offset.x"),
      style.number("shadow.offset.y"),
    ) {
      popup_style.shadow.offset = iced::Vector::new(x, y);
    }

    SizedPopup::new(container(
      container(list)
        .padding(inner_padding)
        .style(move |_| popup_style),
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
