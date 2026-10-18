use std::{
  collections::HashMap,
  sync::{Arc, Mutex, atomic::AtomicU64},
};

#[derive(Clone)]
pub struct TrayItem {
  pub address: String,
  pub title: String,
  pub icon_name: Option<String>,
  pub pixmap: Option<(u32, u32, Vec<u8>)>, // fallback
  pub menu_path: Option<String>,
}

#[derive(Default)]
pub struct TrayStateInner {
  pub items: Arc<Vec<TrayItem>>,
}

#[derive(Debug, Clone)]
pub enum TrayCmd {
  Activate {
    address: String,
    menu_path: String,
    submenu_id: i32,
  },
  AboutToShow {
    address: String,
    menu_path: String,
    submenu_id: i32,
  },
}

pub type NavFrame = (i32, Vec<system_tray::menu::MenuItem>);

pub struct TrayState {
  pub state: Mutex<TrayStateInner>,
  pub revision: AtomicU64,
  pub selected: Mutex<Option<String>>,
  pub menus: Mutex<HashMap<String, Arc<system_tray::menu::TrayMenu>>>,
  pub nav: Mutex<Arc<Vec<NavFrame>>>,
  pub cmd_tx: tokio::sync::mpsc::UnboundedSender<TrayCmd>,
}

impl TrayState {
  pub fn reset_nav(&self) {
    *self.nav.lock().unwrap() = Arc::new(Vec::new());
  }
}

pub type SharedTrayState = Arc<TrayState>;

#[derive(Clone)]
pub struct TrayPayload {
  pub address: String,
  pub menu_path: Option<String>,
  pub submenu_id: i32,
}
