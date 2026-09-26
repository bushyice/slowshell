use std::sync::{Arc, Mutex};

use iced::{
  Alignment, Element, Length,
  widget::{Space, column, container, row},
};
use iced_layershell::reexport::IcedId;
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  message::{ItemEffect, ItemMessage},
  types::ToUstr,
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{
  Backdrop, ConfirmDialogProps, Icon, Renderable, SizedPopup, clickable, hover_row,
  render_confirm_dialog,
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
        let user = std::env::var("USER")
          .ok()
          .or_else(|| {
            std::env::var("HOME")
              .ok()
              .map(|h| h.rsplit('/').next().unwrap_or_default().to_string())
          })
          .unwrap_or_default();
        if !user.is_empty() {
          let _ = std::process::Command::new("loginctl")
            .arg("terminate-user")
            .arg(user)
            .spawn();
        }
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
    ActionsRenderable::confirm_message(id)
  }

  fn cancel(id: IcedId) -> ItemMessage {
    ActionsRenderable::clear_message(id)
  }
}

impl ActionsRenderable {
  const CLEAR: usize = 1;
  const CONFIRM: usize = 0;
  const DISMISS: usize = 2;

  fn confirm_message(id: IcedId) -> ItemMessage {
    ItemMessage::Effect(id, ItemEffect::Custom([40, Self::CONFIRM, 0, 0]))
  }

  fn clear_message(id: IcedId) -> ItemMessage {
    ItemMessage::Effect(id, ItemEffect::Custom([40, Self::CLEAR, 0, 0]))
  }

  fn dismiss_message(id: IcedId) -> ItemMessage {
    ItemMessage::Effect(id, ItemEffect::Custom([40, Self::DISMISS, 0, 0]))
  }

  fn power_row<'a>(
    &self,
    config: &'a Config,
    icon_name: &'static str,
    label: &'static str,
    action: PowerAction,
    id: IcedId,
  ) -> Element<'a, ItemMessage> {
    let theme = config.palette();
    let danger = matches!(action, PowerAction::Shutdown);

    let icon_color = if danger { theme.red } else { theme.primary };
    let icon_size = config
      .computed(&["menu", "icon"])
      .length("icon-size")
      .unwrap_or(16.0) as u16;

    let icon: Element<'a, ItemMessage> = Icon::new(icon_name)
      .size(icon_size)
      .color(icon_color)
      .into();

    let label_classes: &[&str] = if danger {
      &["menu", "text-name", "text-danger"]
    } else {
      &["menu", "text-name"]
    };
    let label_el = config.text(label, label_classes);

    let full_row = row![icon, Space::new().width(8), label_el]
      .align_y(Alignment::Center)
      .width(Length::Fill);

    let wrapped = hover_row(
      config,
      &["menu", "row"],
      container(full_row)
        .padding(config.sizing(&["menu", "row"]).padding())
        .width(Length::Fill),
    );

    let pending = self.action.clone();
    clickable(wrapped, move |_, _, _| {
      Some(action.trigger(pending.clone(), id))
    })
  }
}

impl Renderable for ActionsRenderable {
  fn handle_message(&mut self, message: &ItemMessage) -> Option<ItemEffect> {
    match message {
      ItemMessage::Effect(_, effect) => match effect {
        ItemEffect::Custom([40, kind, 0, 0]) => match *kind {
          Self::CONFIRM => match &self.action.lock().ok().and_then(|p| *p) {
            Some(pending) => {
              pending.execute();

              self.action = Arc::new(Mutex::new(None));
              Some(ItemEffect::Hide)
            }
            None => None,
          },
          Self::CLEAR => {
            self.action = Arc::new(Mutex::new(None));
            Some(ItemEffect::Redraw)
          }
          Self::DISMISS => {
            self.action = Arc::new(Mutex::new(None));
            Some(ItemEffect::Hide)
          }
          _ => None,
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
          config,
        );

        column![confirm_widget].width(Length::Fixed(280.0)).into()
      } else {
        let title = "Power options";

        let items = vec![
          self.power_row(
            config,
            "system-lock-screen-symbolic",
            "Lock Screen",
            PowerAction::Lock,
            id,
          ),
          self.power_row(
            config,
            "system-suspend-symbolic",
            "Suspend",
            PowerAction::Suspend,
            id,
          ),
          self.power_row(
            config,
            "system-log-out-symbolic",
            "Log Out…",
            PowerAction::Logout,
            id,
          ),
          self.power_row(
            config,
            "system-reboot-symbolic",
            "Restart…",
            PowerAction::Reboot,
            id,
          ),
          self.power_row(
            config,
            "system-shutdown-symbolic",
            "Shut Down…",
            PowerAction::Shutdown,
            id,
          ),
        ];

        column![
          title,
          column(items).spacing(config.sizing(&["menu", "list"]).gap),
        ]
        .spacing(config.sizing(&["menu"]).gap)
        .width(Length::Fixed(280.0))
        .into()
      };

    let inner_padding = config.sizing(&["menu", "inner"]).padding();
    let popup_style = config.container_cached(&["menu"]);

    let on_close = Self::dismiss_message(id);

    SizedPopup::new(container(
      container(body).padding(inner_padding).style(popup_style),
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
