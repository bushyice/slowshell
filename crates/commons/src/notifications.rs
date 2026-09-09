use std::{
  collections::HashMap,
  sync::{Arc, Mutex, atomic::AtomicBool, atomic::AtomicU64},
  time::Instant,
};

pub const NOTIF_CHANGED: &str = "component/notifications.changed";

#[derive(Debug, Clone)]
pub struct NotificationItem {
  pub id: u32,
  pub app_name: String,
  pub app_icon: Option<String>,
  pub summary: String,
  pub body: String,
  pub actions: Vec<(String, String)>,
  pub urgency: u8,
  pub created_at: Instant,
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
}

pub struct NotificationState {
  pub dnd: AtomicBool,
  pub state: Mutex<NotificationStoreInner>,
  pub ui: Mutex<NotificationUi>,
  pub revision: AtomicU64,
  pub cmd_tx: tokio::sync::mpsc::UnboundedSender<NotificationCmd>,
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
