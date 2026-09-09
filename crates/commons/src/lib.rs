use std::collections::HashMap;

use slowshell_core::types::Ustr;

pub mod audio;
pub mod bluetooth;
pub mod network;
pub mod notifications;
pub mod power;
pub mod system;

// TODO: move to own files
pub mod popups {
  use slowshell_core::{Store, types::Ustr};

  use crate::panels::{PanelEdge, PanelPositions};

  #[derive(Debug, Clone)]
  pub enum PopupPosition {
    Fixed(f32),

    Percentage(f32),

    FollowMouse(f32),

    Panel { name: Ustr },
  }

  impl PopupPosition {
    pub fn get(&self, store: &Store) -> f32 {
      match self {
        PopupPosition::Fixed(f) => *f,
        PopupPosition::Percentage(_) => 0.,
        PopupPosition::FollowMouse(f) => *f,
        PopupPosition::Panel { name } => {
          match store.borrow::<PanelPositions>().and_then(|p| p.get(name)) {
            Some(pos) => pos.get() as f32,
            None => 0.,
          }
        }
      }
    }
  }

  #[derive(Clone)]
  pub struct PopupSettings {
    pub position: (PopupPosition, PopupPosition),
    pub content: Ustr,
    pub panel_edge: Option<PanelEdge>,
  }

  impl PopupPosition {
    pub fn to_payload_string(&self) -> String {
      match self {
        PopupPosition::Fixed(val) => format!("fixed,{val}"),
        PopupPosition::FollowMouse(val) => format!("cursor,{val}"),
        PopupPosition::Panel { name } => format!("panel,{name}"),
        PopupPosition::Percentage(val) => format!("fixed,{val}"),
      }
    }
  }
}

pub mod panels {
  use std::fmt::Debug;

  use super::*;

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum PanelEdge {
    Top { height: u32 },
    Bottom { height: u32 },
    Left { width: u32 },
    Right { width: u32 },
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum PanelOrientation {
    Horizontal,
    Vertical,
  }

  impl PanelEdge {
    pub fn get(&self) -> u32 {
      match self {
        PanelEdge::Bottom { height } => *height,
        PanelEdge::Top { height } => *height,
        PanelEdge::Right { width } => *width,
        PanelEdge::Left { width } => *width,
      }
    }
  }

  #[derive(Debug, Clone, Default)]
  pub struct PanelPositions {
    entries: HashMap<Ustr, PanelEdge>,
  }

  impl PanelPositions {
    pub fn new() -> Self {
      Self::default()
    }

    pub fn set(&mut self, name: impl Into<Ustr>, edge: PanelEdge) {
      self.entries.insert(name.into(), edge);
    }

    pub fn remove(&mut self, name: &str) {
      self.entries.remove(name);
    }

    pub fn get(&self, name: &str) -> Option<&PanelEdge> {
      self.entries.get(name)
    }

    pub fn keys(&self) -> impl Iterator<Item = &Ustr> + Debug {
      self.entries.keys()
    }

    pub fn for_each(&self, mut f: impl FnMut(&Ustr, &PanelEdge)) {
      for (name, edge) in &self.entries {
        f(name, edge);
      }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Ustr, &PanelEdge)> {
      self.entries.iter()
    }
  }
}

pub mod tray {
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
    pub items: Vec<TrayItem>,
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
    pub menus: Mutex<HashMap<String, system_tray::menu::TrayMenu>>,
    pub nav: Mutex<Vec<NavFrame>>,
    pub cmd_tx: tokio::sync::mpsc::UnboundedSender<TrayCmd>,
  }

  impl TrayState {
    pub fn reset_nav(&self) {
      *self.nav.lock().unwrap() = Vec::new();
    }
  }

  pub type SharedTrayState = Arc<TrayState>;

  #[derive(Clone)]
  pub struct TrayPayload {
    pub address: String,
    pub menu_path: Option<String>,
    pub submenu_id: i32,
  }
}

pub mod desktop {
  use std::{
    cell::RefCell,
    collections::HashMap,
    path::PathBuf,
    sync::{
      Arc, RwLock,
      atomic::{AtomicU64, Ordering},
    },
  };

  use freedesktop_desktop_entry::{DesktopEntry, Iter, default_paths};
  use slowshell_core::{
    Store,
    types::{ToUstr, Ustr, Void},
  };

  static GLOBAL_VERSION: AtomicU64 = AtomicU64::new(0);

  #[derive(Clone, Debug)]
  pub struct DesktopAction {
    pub name: String,
    pub exec: String,
  }

  #[derive(Clone, Debug)]
  pub struct AppEntry {
    pub name: String,
    pub comment: Option<String>,
    pub icon: Option<String>,
    pub exec: String,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
    pub actions: Vec<DesktopAction>,
  }

  #[derive(Clone, Default)]
  pub struct DesktopEntriesData {
    pub all_entries: Vec<PathBuf>,
    pub apps: Arc<Vec<AppEntry>>,
    pub app_id_to_name: Arc<HashMap<String, String>>,
  }

  #[derive(Clone)]
  pub struct DesktopEntries {
    data: Arc<RwLock<DesktopEntriesData>>,
    pub name_cache: RefCell<HashMap<Ustr, Ustr>>,
  }

  fn load_entries() -> DesktopEntriesData {
    let mut apps = Vec::new();
    let mut seen_execs = std::collections::HashSet::new();
    let mut app_id_to_name = HashMap::new();
    let mut all_entries = Vec::new();

    for path in Iter::new(default_paths()) {
      all_entries.push(path.clone());

      if let Ok(entry) = DesktopEntry::from_path::<&str>(&path, None) {
        if let Some(name) = entry.name::<&str>(&[]).map(|n| n.to_string()) {
          if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
            app_id_to_name.insert(file_stem.to_lowercase(), name.clone());
          }

          if let Some(wm_class) = entry.startup_wm_class() {
            app_id_to_name.insert(wm_class.to_string().to_lowercase(), name.clone());
          }

          app_id_to_name.insert(name.to_lowercase(), name.clone());

          if let Some(t) = entry.type_() {
            if !t.eq_ignore_ascii_case("Application") {
              continue;
            }
          }

          let Some(exec) = entry.exec().map(|s| s.to_string()) else {
            continue;
          };

          let name = entry
            .name::<&str>(&[])
            .map(|s| s.into_owned())
            .or_else(|| {
              path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
            })
            .unwrap_or_default();

          if name.is_empty() {
            continue;
          }

          let clean_key = format!("{}:{}", name.to_lowercase(), exec.to_lowercase());
          if !seen_execs.insert(clean_key) {
            continue;
          }

          let comment = entry.comment::<&str>(&[]).map(|s| s.into_owned());
          let icon = entry.icon().map(|s| s.to_string());
          let keywords = entry
            .keywords::<&str>(&[])
            .unwrap_or_default()
            .into_iter()
            .map(|s| s.into_owned())
            .collect();
          let categories = entry
            .categories()
            .unwrap_or_default()
            .into_iter()
            .map(|s| s.to_string())
            .collect();
          let actions = entry
            .actions()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|action| {
              let name = entry
                .action_entry_localized::<&str>(action, "Name", &[])
                .map(|s| s.into_owned())
                .filter(|s| !s.is_empty())?;
              let action_exec = entry.action_entry(action, "Exec").map(|s| s.trim());
              let action_exec = match action_exec.filter(|s| !s.is_empty()) {
                Some(exec) => exec.to_string(),
                None => exec.clone(),
              };
              if action_exec.is_empty() {
                return None;
              }
              Some(DesktopAction {
                name,
                exec: action_exec,
              })
            })
            .collect();

          apps.push(AppEntry {
            name,
            comment,
            icon,
            exec,
            keywords,
            categories,
            actions,
          });
        }
      }
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    DesktopEntriesData {
      all_entries,
      apps: Arc::new(apps),
      app_id_to_name: Arc::new(app_id_to_name),
    }
  }

  impl DesktopEntries {
    pub fn global_version() -> u64 {
      GLOBAL_VERSION.load(Ordering::Relaxed)
    }

    pub fn apps(&self) -> Arc<Vec<AppEntry>> {
      Arc::clone(&self.data.read().unwrap().apps)
    }

    pub fn all_entries(&self) -> Vec<PathBuf> {
      self.data.read().unwrap().all_entries.clone()
    }

    pub fn is_ready(&self) -> bool {
      !self.data.read().unwrap().apps.is_empty()
    }

    pub fn initialize_unless(store: &mut Store) -> Option<Void> {
      if store.borrow::<DesktopEntries>().is_none() {
        let entries = DesktopEntries {
          data: Arc::new(RwLock::new(DesktopEntriesData::default())),
          name_cache: RefCell::new(Default::default()),
        };
        let data_clone = Arc::clone(&entries.data);
        std::thread::Builder::new()
          .name("slowshell-desktop-entries".into())
          .spawn(move || {
            let loaded = load_entries();
            *data_clone.write().unwrap() = loaded;
            GLOBAL_VERSION.fetch_add(1, Ordering::SeqCst);
          })
          .ok();

        store.insert(entries);
        return Some(Void);
      }
      None
    }

    pub fn resolve_name(&self, app_id: &str) -> Ustr {
      if let Some(cached_name) = self.name_cache.borrow().get(app_id) {
        return cached_name.clone();
      }

      let target_id = app_id
        .split(',')
        .last()
        .unwrap_or(app_id)
        .trim()
        .to_lowercase();

      let data = self.data.read().unwrap();
      let is_empty = data.app_id_to_name.is_empty();
      let resolved = data.app_id_to_name.get(&target_id).cloned().or_else(|| {
        target_id
          .split('.')
          .last()
          .and_then(|last_segment| data.app_id_to_name.get(last_segment).cloned())
      });
      drop(data);

      let final_name = resolved
        .unwrap_or_else(|| {
          let mut chars = target_id.chars();
          match chars.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
          }
        })
        .to_ustr();

      if !is_empty {
        self
          .name_cache
          .borrow_mut()
          .insert(app_id.to_ustr(), final_name.clone());
      }

      final_name
    }
  }
}
