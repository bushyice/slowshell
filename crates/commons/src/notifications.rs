use std::{
  collections::HashMap,
  path::PathBuf,
  sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering},
  },
  time::Instant,
};

pub const NOTIF_CHANGED: &str = "component/notifications.changed";

#[derive(Debug, Clone)]
pub enum NotificationImage {
  Rgba {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
  },
  Path(PathBuf),
}

#[derive(Debug, Clone)]
pub struct NotificationItem {
  pub id: u32,
  pub app_name: String,
  pub app_icon: Option<String>,
  pub image: Option<NotificationImage>,
  pub summary: String,
  pub body: String,
  pub actions: Vec<(String, String)>,
  pub urgency: u8,
  pub created_at: Instant,
  pub plugin: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct NotificationStoreInner {
  pub history: Vec<NotificationItem>,
  pub popups: Vec<NotificationItem>,
  pub unread_count: usize,
}

#[derive(Debug, Clone)]
pub enum NotificationCmd {
  DismissPopup(u32),
  DismissHistory(u32),
  ClearAll,
  ToggleDnd,
  SetDnd(bool),
  InvokeAction(u32, String),
  InvokeActionWithText(u32, String, String),
  ExpireAfter(u32, u64),
  ExpirePopup(u32),
  CloseNotification(u32),
}

pub mod close_reason {
  pub const EXPIRED: u32 = 1;
  pub const DISMISSED: u32 = 2;
  pub const CLOSED: u32 = 3;
  pub const UNDEFINED: u32 = 4;
}

pub struct NotificationState {
  pub dnd: AtomicBool,
  pub state: Mutex<NotificationStoreInner>,
  pub ui: Mutex<NotificationUi>,
  pub revision: AtomicU64,
  pub cmd_tx: tokio::sync::mpsc::UnboundedSender<NotificationCmd>,
  pub notify_fd: AtomicI32,
}

impl NotificationState {
  pub fn bump(&self) {
    self.revision.fetch_add(1, Ordering::SeqCst);

    let fd = self.notify_fd.load(Ordering::Relaxed);
    if fd >= 0 {
      let byte = [1u8];
      unsafe {
        libc::write(fd, byte.as_ptr() as *const libc::c_void, 1);
      }
    }
  }
}

#[derive(Debug, Clone, Default)]
pub struct NotificationUi {
  pub replies: HashMap<u32, String>,
}

pub type SharedNotificationState = Arc<NotificationState>;

pub fn reply_action_key(item: &NotificationItem) -> Option<String> {
  item
    .actions
    .iter()
    .find(|(key, _)| {
      let k = key.to_lowercase();
      k == "inline-reply" || k == "reply"
    })
    .map(|(key, _)| key.clone())
}

#[derive(Clone, Default)]
pub struct NotificationPayload {
  pub id: u32,
  pub key: Option<String>,
  pub text: Option<String>,
}
