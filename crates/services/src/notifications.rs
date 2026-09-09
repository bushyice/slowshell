use std::{
  collections::HashMap,
  os::fd::OwnedFd,
  sync::atomic::{AtomicU32, Ordering},
  time::{Duration, Instant},
};

use slowshell_commons::notifications::{
  NotificationCmd, NotificationItem, SharedNotificationState,
};
use zbus::{Connection, zvariant::Value};

use crate::util;

pub struct NotificationServer {
  shared: SharedNotificationState,
  notify: Option<OwnedFd>,
  next_id: AtomicU32,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
  async fn notify(
    &self,
    app_name: String,
    replaces_id: u32,
    app_icon: String,
    summary: String,
    body: String,
    actions: Vec<String>,
    hints: HashMap<String, Value<'_>>,
    expire_timeout: i32,
  ) -> zbus::fdo::Result<u32> {
    let id = if replaces_id != 0 {
      replaces_id
    } else {
      self.next_id.fetch_add(1, Ordering::SeqCst)
    };

    let urgency = hints
      .get("urgency")
      .and_then(|v| match v {
        Value::U8(u) => Some(*u),
        _ => None,
      })
      .unwrap_or(1);

    let mut action_pairs = Vec::new();
    let mut iter = actions.into_iter();
    while let Some(key) = iter.next() {
      let label = iter.next().unwrap_or_default();
      action_pairs.push((key, label));
    }

    let item = NotificationItem {
      id,
      app_name: if app_name.is_empty() {
        "Notification".to_string()
      } else {
        app_name
      },
      app_icon: if app_icon.is_empty() {
        None
      } else {
        Some(app_icon)
      },
      summary,
      body,
      actions: action_pairs,
      urgency,
      created_at: Instant::now(),
    };

    let is_dnd = self.shared.dnd.load(Ordering::Relaxed);

    {
      let mut state = self.shared.state.lock().unwrap();
      state.history.retain(|n| n.id != id);
      state.history.insert(0, item.clone());
      state.unread_count += 1;

      if !is_dnd {
        state.popups.retain(|n| n.id != id);
        state.popups.push(item);
      }
    }

    bump(&self.shared, &self.notify);

    if !is_dnd {
      let timeout_ms = if expire_timeout > 0 {
        expire_timeout as u64
      } else {
        5000
      };

      let cmd_tx = self.shared.cmd_tx.clone();
      tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(timeout_ms)).await;
        let _ = cmd_tx.send(NotificationCmd::DismissPopup(id));
      });
    }

    Ok(id)
  }

  async fn close_notification(&self, id: u32) -> zbus::fdo::Result<()> {
    {
      let mut state = self.shared.state.lock().unwrap();
      state.popups.retain(|n| n.id != id);
    }
    bump(&self.shared, &self.notify);
    Ok(())
  }

  async fn get_capabilities(&self) -> zbus::fdo::Result<Vec<String>> {
    Ok(vec![
      "body".to_string(),
      "actions".to_string(),
      "icon-static".to_string(),
      "persistence".to_string(),
    ])
  }

  async fn get_server_information(&self) -> zbus::fdo::Result<(String, String, String, String)> {
    Ok((
      "slowshell".to_string(),
      "slowshell".to_string(),
      "0.1.0".to_string(),
      "1.2".to_string(),
    ))
  }
}

pub fn run(
  shared: SharedNotificationState,
  mut cmd_rx: tokio::sync::mpsc::UnboundedReceiver<NotificationCmd>,
  notify: Option<OwnedFd>,
) {
  crate::shared_runtime().spawn(async move {
    loop {
      match notif_loop(&shared, &mut cmd_rx, &notify).await {
        Ok(()) => break,
        Err(e) => {
          eprintln!("[notifications] dbus service error: {e}");
          tokio::time::sleep(Duration::from_millis(2000)).await;
        }
      }
    }
  });
}

async fn notif_loop(
  shared: &SharedNotificationState,
  cmd_rx: &mut tokio::sync::mpsc::UnboundedReceiver<NotificationCmd>,
  notify: &Option<OwnedFd>,
) -> anyhow::Result<()> {
  let server = NotificationServer {
    shared: shared.clone(),
    notify: notify.as_ref().map(|f| {
      use std::os::fd::AsFd;
      f.as_fd().try_clone_to_owned().unwrap()
    }),
    next_id: AtomicU32::new(1),
  };

  let conn = zbus::connection::Builder::session()?
    .name("org.freedesktop.Notifications")?
    .serve_at("/org/freedesktop/Notifications", server)?
    .build()
    .await?;

  loop {
    if let Some(cmd) = cmd_rx.recv().await {
      match cmd {
        NotificationCmd::DismissPopup(id) => {
          let mut state = shared.state.lock().unwrap();
          state.popups.retain(|n| n.id != id);
        }
        NotificationCmd::DismissHistory(id) => {
          let mut state = shared.state.lock().unwrap();
          state.history.retain(|n| n.id != id);
          state.popups.retain(|n| n.id != id);
        }
        NotificationCmd::ClearAll => {
          let mut state = shared.state.lock().unwrap();
          state.history.clear();
          state.popups.clear();
          state.unread_count = 0;
        }
        NotificationCmd::ToggleDnd => {
          let current = shared.dnd.load(Ordering::Relaxed);
          shared.dnd.store(!current, Ordering::Relaxed);
          if !current {
            let mut state = shared.state.lock().unwrap();
            state.popups.clear();
          }
        }
        NotificationCmd::SetDnd(dnd) => {
          shared.dnd.store(dnd, Ordering::Relaxed);
          if dnd {
            let mut state = shared.state.lock().unwrap();
            state.popups.clear();
          }
        }
        NotificationCmd::InvokeAction(id, action_key) => {
          notify_action_invoked(&conn, id, &action_key).await;
          let mut state = shared.state.lock().unwrap();
          state.popups.retain(|n| n.id != id);
        }
        NotificationCmd::InvokeActionWithText(id, action_key, _text) => {
          notify_action_invoked(&conn, id, &action_key).await;
          let mut state = shared.state.lock().unwrap();
          state.popups.retain(|n| n.id != id);
        }
      }
      bump(shared, notify);
    }
  }
}

async fn notify_action_invoked(conn: &Connection, id: u32, action_key: &str) {
  let _: zbus::Result<()> = conn
    .emit_signal(
      None::<&str>,
      "/org/freedesktop/Notifications",
      "org.freedesktop.Notifications",
      "ActionInvoked",
      &(id, action_key),
    )
    .await;
}

fn bump(shared: &SharedNotificationState, notify: &Option<OwnedFd>) {
  shared.revision.fetch_add(1, Ordering::SeqCst);
  if let Some(fd) = notify {
    util::notify_signal(fd);
  }
}
