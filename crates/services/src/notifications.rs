use std::{
  collections::HashMap,
  os::fd::OwnedFd,
  path::PathBuf,
  sync::atomic::{AtomicU32, Ordering},
  time::{Duration, Instant},
};

use slowshell_commons::notifications::{
  NotificationCmd, NotificationImage, NotificationItem, SharedNotificationState, close_reason,
};
use zbus::{Connection, zvariant::Value};

pub struct NotificationServer {
  shared: SharedNotificationState,
  #[allow(dead_code)]
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

    let mut image = None;
    for (key, value) in &hints {
      match key.as_str() {
        "image-data" | "image_data" | "icon_data" => {
          if let Some(parsed) = image_from_hint(value) {
            image = Some(parsed);
          }
        }
        "image-path" | "image_path" => {
          if let Some(path) = hint_str(value).and_then(icon_path) {
            image = Some(NotificationImage::Path(path));
          }
        }
        _ => {}
      }
    }
    if image.is_none() {
      image = icon_path(&app_icon).map(NotificationImage::Path);
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
      image,
      summary,
      body,
      actions: action_pairs,
      urgency,
      created_at: Instant::now(),
      plugin: None,
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

    self.shared.bump();

    let timeout_ms = if expire_timeout > 0 {
      expire_timeout as u64
    } else {
      5000
    };

    let cmd_tx = self.shared.cmd_tx.clone();
    tokio::spawn(async move {
      tokio::time::sleep(Duration::from_millis(timeout_ms)).await;
      let _ = cmd_tx.send(NotificationCmd::ExpirePopup(id));
    });

    Ok(id)
  }

  async fn close_notification(&self, id: u32) -> zbus::fdo::Result<()> {
    let _ = self
      .shared
      .cmd_tx
      .send(NotificationCmd::CloseNotification(id));
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
) -> miette::Result<()> {
  if let Some(fd) = notify {
    use std::os::fd::AsRawFd;
    shared
      .notify_fd
      .store(fd.as_raw_fd(), std::sync::atomic::Ordering::Relaxed);
  }

  let server = NotificationServer {
    shared: shared.clone(),
    notify: notify.as_ref().map(|f| {
      use std::os::fd::AsFd;
      f.as_fd().try_clone_to_owned().unwrap()
    }),
    next_id: AtomicU32::new(1),
  };

  use miette::IntoDiagnostic;

  let conn = zbus::connection::Builder::session()
    .into_diagnostic()?
    .name("org.freedesktop.Notifications")
    .into_diagnostic()?
    .serve_at("/org/freedesktop/Notifications", server)
    .into_diagnostic()?
    .build()
    .await
    .into_diagnostic()?;

  loop {
    if let Some(cmd) = cmd_rx.recv().await {
      match cmd {
        NotificationCmd::DismissPopup(id) => {
          if drop_popup(shared, id) {
            notify_closed(&conn, id, close_reason::DISMISSED).await;
          }
        }
        NotificationCmd::DismissHistory(id) => {
          let dropped = {
            let mut state = shared.state.lock().unwrap();
            state.history.retain(|n| n.id != id);
            let before = state.popups.len();
            state.popups.retain(|n| n.id != id);
            before != state.popups.len()
          };
          if dropped {
            notify_closed(&conn, id, close_reason::DISMISSED).await;
          }
        }
        NotificationCmd::ClearAll => {
          let ids = {
            let mut state = shared.state.lock().unwrap();
            let ids: Vec<u32> = state.popups.iter().map(|n| n.id).collect();
            state.history.clear();
            state.popups.clear();
            state.unread_count = 0;
            ids
          };
          for id in ids {
            notify_closed(&conn, id, close_reason::DISMISSED).await;
          }
        }
        NotificationCmd::ToggleDnd => {
          let current = shared.dnd.load(Ordering::Relaxed);
          shared.dnd.store(!current, Ordering::Relaxed);
          if !current {
            notify_all_closed(shared, &conn).await;
          }
        }
        NotificationCmd::SetDnd(dnd) => {
          shared.dnd.store(dnd, Ordering::Relaxed);
          if dnd {
            notify_all_closed(shared, &conn).await;
          }
        }
        NotificationCmd::InvokeAction(id, action_key) => {
          notify_action_invoked(&conn, id, &action_key).await;
          drop_popup(shared, id);
          notify_closed(&conn, id, close_reason::DISMISSED).await;
        }
        NotificationCmd::InvokeActionWithText(id, action_key, _text) => {
          notify_action_invoked(&conn, id, &action_key).await;
          drop_popup(shared, id);
          notify_closed(&conn, id, close_reason::DISMISSED).await;
        }
        NotificationCmd::ExpireAfter(id, millis) => {
          let cmd_tx = shared.cmd_tx.clone();
          tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(millis)).await;
            let _ = cmd_tx.send(NotificationCmd::ExpirePopup(id));
          });
        }
        NotificationCmd::ExpirePopup(id) => {
          drop_popup(shared, id);
          notify_closed(&conn, id, close_reason::EXPIRED).await;
        }
        NotificationCmd::CloseNotification(id) => {
          drop_popup(shared, id);
          notify_closed(&conn, id, close_reason::CLOSED).await;
        }
      }
      shared.bump();
    }
  }
}

fn drop_popup(shared: &SharedNotificationState, id: u32) -> bool {
  let mut state = shared.state.lock().unwrap();
  let before = state.popups.len();
  state.popups.retain(|n| n.id != id);
  before != state.popups.len()
}

async fn notify_all_closed(shared: &SharedNotificationState, conn: &Connection) {
  let ids: Vec<u32> = {
    let mut state = shared.state.lock().unwrap();
    let ids: Vec<u32> = state.popups.iter().map(|n| n.id).collect();
    state.popups.clear();
    ids
  };
  for id in ids {
    notify_closed(conn, id, close_reason::DISMISSED).await;
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

async fn notify_closed(conn: &Connection, id: u32, reason: u32) {
  let _: zbus::Result<()> = conn
    .emit_signal(
      None::<&str>,
      "/org/freedesktop/Notifications",
      "org.freedesktop.Notifications",
      "NotificationClosed",
      &(id, reason),
    )
    .await;
}

fn hint_str<'a>(value: &'a Value<'_>) -> Option<&'a str> {
  match value {
    Value::Str(text) => Some(text.as_str()),
    _ => None,
  }
}

fn icon_path(value: &str) -> Option<PathBuf> {
  let path = value.strip_prefix("file://").unwrap_or(value);
  let path = PathBuf::from(path);
  if path.is_absolute() { Some(path) } else { None }
}

fn value_int(value: &Value<'_>) -> Option<i64> {
  Some(match value {
    Value::I16(n) => i64::from(*n),
    Value::I32(n) => i64::from(*n),
    Value::I64(n) => *n,
    Value::U8(n) => i64::from(*n),
    Value::U16(n) => i64::from(*n),
    Value::U32(n) => i64::from(*n),
    Value::U64(n) => *n as i64,
    _ => return None,
  })
}

fn value_bool(value: &Value<'_>) -> Option<bool> {
  match value {
    Value::Bool(flag) => Some(*flag),
    _ => None,
  }
}

fn image_from_hint(value: &Value<'_>) -> Option<NotificationImage> {
  let Value::Structure(structure) = value else {
    return None;
  };
  let fields = structure.fields();

  let width = value_int(fields.get(0)?)? as usize;
  let height = value_int(fields.get(1)?)? as usize;
  let stride = value_int(fields.get(2)?)? as usize;
  let has_alpha = value_bool(fields.get(3)?)?;
  let channels = value_int(fields.get(5)?)? as usize;

  if width == 0 || height == 0 || channels < 3 {
    return None;
  }

  let Value::Array(data) = fields.get(6)? else {
    return None;
  };
  let bytes: Vec<u8> = data
    .inner()
    .iter()
    .filter_map(|byte| match byte {
      Value::U8(byte) => Some(*byte),
      _ => None,
    })
    .collect();

  let stride = stride.max(width * channels);
  let mut pixels = Vec::with_capacity(width * height * 4);
  for row in 0..height {
    let start = row * stride;
    let line = bytes.get(start..start + width * channels)?;
    for column in 0..width {
      let pixel = &line[column * channels..column * channels + channels];
      pixels.extend_from_slice(&pixel[0..3]);
      pixels.push(if has_alpha && channels >= 4 {
        pixel[3]
      } else {
        255
      });
    }
  }

  Some(NotificationImage::Rgba {
    width: width as u32,
    height: height as u32,
    pixels,
  })
}
