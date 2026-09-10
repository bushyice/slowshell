use std::{
  collections::HashMap,
  os::fd::AsRawFd,
  sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
  },
};

use iced::{
  Color, Element, Length,
  widget::{Row, container, image, text},
};
use nix::{
  fcntl::{FcntlArg, OFlag, fcntl},
  sys::epoll::EpollFlags,
};
use slowshell_commons::tray::{
  SharedTrayState, TrayCmd, TrayItem, TrayPayload, TrayState, TrayStateInner,
};
use slowshell_config::Config;
use slowshell_core::{
  Store,
  listeners::{FdHandle, ListenerAction},
  message::{EventFilter, ItemEffect, ItemMessage},
  types::OptionalPayloadBox,
};
use slowshell_services::util::drain_signal_fd;
use slowshell_widgets::{Icon, clickable};

use crate::{
  Component, ComponentContext, ComponentOptions, MenuConfig, popup_open_action, spaced_component,
};

const TICK: &str = "component/tray.tick";

struct GlobalTray {
  shared: SharedTrayState,
  rx: Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<TrayCmd>>>,
}

static GLOBAL: OnceLock<GlobalTray> = OnceLock::new();

fn global() -> &'static GlobalTray {
  GLOBAL.get_or_init(|| {
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    GlobalTray {
      shared: Arc::new(TrayState {
        state: Mutex::new(TrayStateInner::default()),
        revision: AtomicU64::new(0),
        selected: Mutex::new(None),
        menus: Mutex::new(HashMap::new()),
        nav: Mutex::new(Vec::new()),
        cmd_tx,
      }),
      rx: Mutex::new(Some(cmd_rx)),
    }
  })
}

pub struct SystemTray {
  signal_fd: Option<i32>,
  last_revision: u64,
}

impl SystemTray {
  pub fn new() -> Self {
    Self {
      signal_fd: None,
      last_revision: 0,
    }
  }

  fn redraw_if_changed(&mut self) -> ItemEffect {
    let revision = global().shared.revision.load(Ordering::Relaxed);
    if revision != self.last_revision {
      self.last_revision = revision;
      ItemEffect::Redraw
    } else {
      ItemEffect::None
    }
  }
}

impl Default for SystemTray {
  fn default() -> Self {
    Self::new()
  }
}

impl Component for SystemTray {
  fn events(&self) -> Vec<EventFilter> {
    vec![
      EventFilter::Named(TICK.into()),
      EventFilter::Payload("tray.activate".into()),
    ]
  }

  fn watch(&mut self, store: &mut Store, _options: Option<&ComponentOptions>) {
    store.insert(global().shared.clone());

    if self.signal_fd.is_some() {
      return;
    }

    let Some(rx) = global().rx.lock().unwrap().take() else {
      return;
    };

    let notify = store.borrow::<FdHandle>().and_then(|handle| {
      let (read_fd, write_fd) = nix::unistd::pipe().ok()?;
      fcntl(&read_fd, FcntlArg::F_SETFL(OFlag::O_NONBLOCK)).ok()?;

      let raw = read_fd.as_raw_fd();
      handle.watch_with_flags(
        read_fd,
        EpollFlags::EPOLLIN | EpollFlags::EPOLLET,
        ListenerAction::Signal {
          name: TICK.into(),
          fd: raw,
        },
      );
      self.signal_fd = Some(raw);

      Some(write_fd)
    });

    slowshell_services::tray::run(global().shared.clone(), rx, notify);
  }

  fn stop(&mut self, store: &Store, _options: Option<&ComponentOptions>) {
    if let (Some(fd), Some(handle)) = (self.signal_fd.take(), store.borrow::<FdHandle>()) {
      handle.stop_timer(fd);
    }
  }

  fn update(
    &mut self,
    _config: &Config,
    store: &mut Store,
    event: &ListenerAction,
    _options: Option<&ComponentOptions>,
  ) -> miette::Result<ItemEffect> {
    if store.borrow::<SharedTrayState>().is_none() {
      store.insert(global().shared.clone());
    }

    match event {
      ListenerAction::Named(n) if &**n == TICK => Ok(self.redraw_if_changed()),
      ListenerAction::Signal { name, fd } if &**name == TICK => {
        if Some(*fd) == self.signal_fd {
          drain_signal_fd(*fd);
        }
        Ok(self.redraw_if_changed())
      }
      ListenerAction::Payload { name, payload } if name.as_ref() == "tray.activate" => {
        if let Some(payload) = payload.transform::<TrayPayload>() {
          if let Some(menu_path) = payload.menu_path.clone() {
            let _ = global().shared.cmd_tx.send(TrayCmd::Activate {
              address: payload.address.to_string(),
              menu_path: menu_path.to_string(),
              submenu_id: payload.submenu_id,
            });
          }
        }
        Ok(ItemEffect::None)
      }
      _ => Ok(ItemEffect::None),
    }
  }

  fn view<'a>(
    &self,
    config: &Config,
    _store: &Store,
    ctx: &ComponentContext,
    options: Option<&ComponentOptions>,
  ) -> Element<'a, ItemMessage> {
    let style = config.style("component");
    let color = style.color(&config.theme, "color", config.theme.text);
    let icon_size = style.number("icon.size").unwrap_or(16.0) as u16;
    let font_size = style.number("font.size").unwrap_or(12.0);
    let spacing = options
      .and_then(|o| o.number("spacing"))
      .unwrap_or(style.number("spacing").unwrap_or(4.0));

    let items = global().shared.state.lock().unwrap().items.clone();

    let mut tray = Row::<ItemMessage>::new().spacing(spacing);
    if items.is_empty() {
      tray = tray.push(text("󰓄").size(font_size).color(color));
    }

    let shared = global().shared.clone();
    let position = ctx.position;
    for item in items {
      if let Some(el) = render_item(&item, icon_size, color) {
        let shared = shared.clone();
        let address = item.address.clone();
        let menu_path = item.menu_path.clone();
        let has_menu = menu_path.is_some();
        let el = clickable(el, move |_, _, cursor| {
          if !has_menu {
            return None;
          }
          let Some(point) = cursor.position() else {
            return None;
          };
          *shared.selected.lock().unwrap() = Some(address.clone());
          shared.reset_nav();
          if let Some(path) = &menu_path {
            let _ = shared.cmd_tx.send(TrayCmd::AboutToShow {
              address: address.clone(),
              menu_path: path.clone(),
              submenu_id: 0,
            });
          }
          let action = popup_open_action(
            &MenuConfig {
              content: "menus/tray".into(),
              panel_name: None,
              position,
            },
            point,
          );
          Some(ItemMessage::Action(action))
        });
        tray = tray.push(el);
      }
    }

    spaced_component(config, ctx, container(tray).into())
  }
}

fn render_item<'a>(item: &TrayItem, size: u16, color: Color) -> Option<Element<'a, ItemMessage>> {
  if let Some(name) = &item.icon_name {
    let mut icon = Icon::new(name.clone()).size(size);
    if name.contains("symbolic") {
      icon = icon.color(color);
    }
    return Some(icon.into());
  }

  if let Some((w, h, rgba)) = &item.pixmap {
    if *w > 0 && *h > 0 && !rgba.is_empty() {
      let handle = Icon::<ItemMessage>::from_pixmap(&item.address, *w, *h, rgba);
      return Some(
        image(handle)
          .width(Length::Fixed(size as f32))
          .height(Length::Fixed(size as f32))
          .into(),
      );
    }
  }

  None
}
