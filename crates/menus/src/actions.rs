use std::sync::{Arc, Mutex};

use iced::{
  Alignment, Color, Element, Length,
  widget::{Space, column, container, row, text},
};
use iced_layershell::reexport::IcedId;
use slowshell_config::{Config, style::Style, style::Theme};
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{ItemEffect, ItemMessage},
  types::ToUstr,
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{
  Backdrop, ConfirmDialogProps, Icon, Renderable, SizedPopup, clickable, render_confirm_dialog,
};

pub struct ActionsRenderable {
  action: Arc<Mutex<Option<PowerAction>>>,
}

impl Default for ActionsRenderable {
  fn default() -> Self {
    ActionsRenderable {
      action: Arc::new(Mutex::new(None)),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PowerAction {
  Lock,
  Suspend,
  Reboot,
  Shutdown,
  Logout,
}

impl PowerAction {
  fn title(self) -> &'static str {
    match self {
      PowerAction::Lock => "Lock Session",
      PowerAction::Suspend => "Suspend System",
      PowerAction::Reboot => "Restart Computer",
      PowerAction::Shutdown => "Shut Down Computer",
      PowerAction::Logout => "Log Out",
    }
  }

  fn prompt(self) -> &'static str {
    match self {
      PowerAction::Lock => "Are you sure you want to lock the session?",
      PowerAction::Suspend => "Are you sure you want to suspend the system?",
      PowerAction::Reboot => "Are you sure you want to restart the computer?",
      PowerAction::Shutdown => "Are you sure you want to shut down the computer?",
      PowerAction::Logout => "Are you sure you want to log out of this session?",
    }
  }

  fn confirm_label(self) -> &'static str {
    match self {
      PowerAction::Lock => "Lock",
      PowerAction::Suspend => "Suspend",
      PowerAction::Reboot => "Restart",
      PowerAction::Shutdown => "Shut Down",
      PowerAction::Logout => "Log Out",
    }
  }

  fn icon(self) -> &'static str {
    match self {
      PowerAction::Lock => "system-lock-screen-symbolic",
      PowerAction::Suspend => "system-suspend-symbolic",
      PowerAction::Reboot => "system-reboot-symbolic",
      PowerAction::Shutdown => "system-shutdown-symbolic",
      PowerAction::Logout => "system-log-out-symbolic",
    }
  }

  fn is_danger(self) -> bool {
    matches!(self, PowerAction::Shutdown | PowerAction::Reboot)
  }

  fn needs_confirmation(self) -> bool {
    matches!(
      self,
      PowerAction::Suspend | PowerAction::Shutdown | PowerAction::Reboot | PowerAction::Logout
    )
  }

  fn execute(self) {
    match self {
      PowerAction::Lock => {
        let _ = std::process::Command::new("loginctl")
          .arg("lock-session")
          .spawn();
      }
      PowerAction::Suspend => {
        let _ = std::process::Command::new("systemctl")
          .arg("suspend")
          .spawn();
      }
      PowerAction::Reboot => {
        let _ = std::process::Command::new("systemctl")
          .arg("reboot")
          .spawn();
      }
      PowerAction::Shutdown => {
        let _ = std::process::Command::new("systemctl")
          .arg("poweroff")
          .spawn();
      }
      PowerAction::Logout => {
        let _ = std::process::Command::new("loginctl")
          .args(["terminate-user", ""])
          .spawn();
      }
    }
  }

  fn trigger(self, mutex: Arc<Mutex<Option<PowerAction>>>, id: IcedId) -> ItemMessage {
    if self.needs_confirmation() {
      if let Ok(mut pending) = mutex.lock() {
        *pending = Some(self);
      }
      ItemMessage::EffectAction(
        id,
        ItemEffect::Redraw,
        ListenerAction::Payload {
          name: "popup.redraw".to_ustr(),
          payload: None,
        },
      )
    } else {
      self.execute();
      ItemMessage::Effect(id, ItemEffect::Hide)
    }
  }

  fn confirm(self, id: IcedId) -> ItemMessage {
    ItemMessage::Effect(id, ItemEffect::Custom([40, 0, 0, 0]))
  }

  fn cancel(id: IcedId) -> ItemMessage {
    ItemMessage::EffectAction(
      id,
      ItemEffect::Redraw,
      ListenerAction::Payload {
        name: "popup.redraw".to_ustr(),
        payload: None,
      },
    )
  }
}

impl ActionsRenderable {
  fn power_row<'a>(
    &self,
    icon_name: &'static str,
    label: &'static str,
    action: PowerAction,
    id: IcedId,
    style: &Style,
    theme: &Theme,
  ) -> Element<'a, ItemMessage> {
    let color = if matches!(action, PowerAction::Shutdown) {
      style.color(theme, "color.danger", theme.red)
    } else {
      style.color(theme, "color", theme.text)
    };

    let icon_color = if matches!(action, PowerAction::Shutdown) {
      style.color(theme, "color.danger", theme.red)
    } else {
      style.color(theme, "color.primary", theme.primary)
    };

    let icon: Element<'a, ItemMessage> = Icon::new(icon_name).size(16).color(icon_color).into();

    let label_el = text(label)
      .size(style.number("name.font.size").unwrap_or(13.0))
      .color(color);

    let full_row = row![icon, Space::new().width(8), label_el]
      .align_y(Alignment::Center)
      .width(Length::Fill);

    let bg = style.color(theme, "row.background", Color::TRANSPARENT);
    let row_radius = style.number("row.radius").unwrap_or(6.0);

    let wrapped = container(full_row)
      .padding(style.padding([6.0, 10.0]))
      .width(Length::Fill)
      .style(move |_| container::Style {
        background: Some(bg.into()),
        border: iced::Border {
          radius: row_radius.into(),
          ..Default::default()
        },
        ..Default::default()
      });

    let pending = self.action.clone();
    clickable(wrapped.into(), move |_, _, _| {
      Some(action.trigger(pending.clone(), id))
    })
  }
}

impl Renderable for ActionsRenderable {
  fn handle_message(&mut self, message: &ItemMessage) -> Option<ItemEffect> {
    match message {
      ItemMessage::Effect(_, effect) => match effect {
        ItemEffect::Custom([40, _, _, _]) => match &self.action.lock().ok().and_then(|p| *p) {
          Some(pending) => {
            pending.execute();

            self.action = Arc::new(Mutex::new(None));
            Some(ItemEffect::Hide)
          }
          None => None,
        },
        _ => None,
      },
      _ => None,
    }
  }

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

    let body: Element<'a, ItemMessage> =
      if let Some(action) = &self.action.lock().ok().and_then(|p| *p) {
        let confirm_widget = render_confirm_dialog(
          ConfirmDialogProps {
            title: action.title(),
            message: action.prompt(),
            icon: Some(action.icon()),
            confirm_label: action.confirm_label(),
            cancel_label: "Cancel",
            is_danger: action.is_danger(),
            on_confirm: action.confirm(id),
            on_cancel: PowerAction::cancel(id),
          },
          &style,
          theme,
        );

        column![confirm_widget].width(Length::Fixed(280.0)).into()
      } else {
        let title = "Power options";

        let items = vec![
          self.power_row(
            "system-lock-screen-symbolic",
            "Lock Screen",
            PowerAction::Lock,
            id,
            &style,
            theme,
          ),
          self.power_row(
            "system-suspend-symbolic",
            "Suspend",
            PowerAction::Suspend,
            id,
            &style,
            theme,
          ),
          self.power_row(
            "system-log-out-symbolic",
            "Log Out…",
            PowerAction::Logout,
            id,
            &style,
            theme,
          ),
          self.power_row(
            "system-reboot-symbolic",
            "Restart…",
            PowerAction::Reboot,
            id,
            &style,
            theme,
          ),
          self.power_row(
            "system-shutdown-symbolic",
            "Shut Down…",
            PowerAction::Shutdown,
            id,
            &style,
            theme,
          ),
        ];

        column![
          title,
          column(items).spacing(style.number("list.spacing").unwrap_or(6.0)),
        ]
        .spacing(style.number("spacing").unwrap_or(14.0))
        .width(Length::Fixed(280.0))
        .into()
      };

    let inner_padding = style.number("inner.padding").unwrap_or(8.0);
    let mut popup_style = style.container_style(theme);
    if let (Some(x), Some(y)) = (
      style.number("shadow.offset.x"),
      style.number("shadow.offset.y"),
    ) {
      popup_style.shadow.offset = iced::Vector::new(x, y);
    }

    let on_close = {
      if let Ok(mut pending) = self.action.lock() {
        *pending = None;
      }
      ItemMessage::Effect(id, ItemEffect::Hide)
    };

    SizedPopup::new(container(
      container(body)
        .padding(inner_padding)
        .style(move |_| popup_style),
    ))
    .with_width(240.)
    .with_backdrop(Backdrop::transparent().with_close_on_click(true))
    .with_position(data.position.0.get(store), data.position.1.get(store))
    .with_panel_edge(data.panel_edge)
    .with_panel_insets(store)
    .with_on_close(on_close)
    .into()
  }
}
