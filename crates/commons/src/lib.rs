use std::collections::HashMap;

use slowshell_core::types::Ustr;

pub mod audio;
pub mod bluetooth;
pub mod network;
pub mod notifications;
pub mod power;
pub mod quicksettings;
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
    pub insets: bool,
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
}

pub mod desktop {
  use std::{
    cell::RefCell,
    collections::HashMap,
    io::Write,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
      Arc, RwLock,
      atomic::{AtomicU64, Ordering},
    },
  };

  use freedesktop_desktop_entry::{DesktopEntry, Iter, default_paths};
  use slowshell_core::{
    Store,
    listeners::FdHandle,
    types::{ToUstr, Ustr, Void},
  };

  static GLOBAL_VERSION: AtomicU64 = AtomicU64::new(0);

  pub const DESKTOP_ENTRIES_CHANGED: &str = "desktop-entries.changed";

  #[derive(Clone, Debug)]
  pub struct DesktopAction {
    pub name: String,
    pub exec: String,
  }

  #[derive(Clone, Debug)]
  pub struct AppEntry {
    pub name: String,
    pub name_lower: String,
    pub id_lower: String,
    pub comment: Option<String>,
    pub comment_lower: Option<String>,
    pub icon: Option<String>,
    pub exec: String,
    pub exec_lower: String,
    pub keywords: Vec<String>,
    pub keywords_lower: Vec<String>,
    pub categories: Vec<String>,
    pub actions: Vec<DesktopAction>,
  }

  #[derive(Clone, Default)]
  pub struct DesktopEntriesData {
    pub all_entries: Vec<PathBuf>,
    pub apps: Arc<Vec<AppEntry>>,
    pub app_id_to_name: Arc<HashMap<String, String>>,
    pub app_id_to_exec: Arc<HashMap<String, String>>,
  }

  #[derive(Clone)]
  pub struct DesktopEntries {
    data: Arc<RwLock<DesktopEntriesData>>,
    pub name_cache: RefCell<HashMap<Ustr, Ustr>>,
  }

  fn parse_entry(path: &Path) -> Option<(AppEntry, Vec<String>)> {
    let entry = DesktopEntry::from_path::<&str>(path, None).ok()?;
    let name = entry.name::<&str>(&[]).map(|n| n.to_string())?;

    if let Some(t) = entry.type_() {
      if !t.eq_ignore_ascii_case("Application") {
        return None;
      }
    }

    let exec = entry.exec().map(|s| s.to_string())?;

    let mut keys = Vec::new();
    let id_lower = path
      .file_stem()
      .and_then(|s| s.to_str())
      .map(|s| s.to_lowercase())
      .unwrap_or_default();

    if !id_lower.is_empty() {
      keys.push(id_lower.clone());
    }

    if let Some(wm_class) = entry.startup_wm_class() {
      keys.push(wm_class.to_lowercase());
    }

    let name_lower = name.to_lowercase();
    keys.push(name_lower.clone());

    let comment = entry.comment::<&str>(&[]).map(|s| s.into_owned());
    let icon = entry.icon().map(|s| s.to_string());
    let keywords: Vec<String> = entry
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

    let comment_lower = comment.as_ref().map(|comment| comment.to_lowercase());
    let exec_lower = exec.to_lowercase();
    let keywords_lower = keywords
      .iter()
      .map(|keyword| keyword.to_lowercase())
      .collect();

    Some((
      AppEntry {
        name,
        name_lower,
        id_lower,
        comment,
        comment_lower,
        icon,
        exec,
        exec_lower,
        keywords,
        keywords_lower,
        categories,
        actions,
      },
      keys,
    ))
  }

  fn load_entries() -> DesktopEntriesData {
    let mut apps = Vec::new();
    let mut seen_execs = std::collections::HashSet::new();
    let mut app_id_to_name = HashMap::new();
    let mut app_id_to_exec = HashMap::new();
    let mut all_entries = Vec::new();

    for path in Iter::new(default_paths()) {
      all_entries.push(path.clone());

      let Some((app, keys)) = parse_entry(&path) else {
        continue;
      };

      for key in keys {
        app_id_to_name.insert(key.clone(), app.name.clone());
        app_id_to_exec.insert(key, app.exec.clone());
      }

      let clean_key = format!("{}:{}", app.name_lower, app.exec_lower);
      if !seen_execs.insert(clean_key) {
        continue;
      }

      apps.push(app);
    }

    apps.sort_by(|a, b| a.name_lower.cmp(&b.name_lower));

    DesktopEntriesData {
      all_entries,
      apps: Arc::new(apps),
      app_id_to_name: Arc::new(app_id_to_name),
      app_id_to_exec: Arc::new(app_id_to_exec),
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
        let notify = store
          .borrow::<FdHandle>()
          .and_then(|handle| handle.signal_pipe(DESKTOP_ENTRIES_CHANGED));

        std::thread::Builder::new()
          .name("slowshell-desktop-entries".into())
          .spawn(move || {
            let loaded = load_entries();
            *data_clone.write().unwrap() = loaded;
            GLOBAL_VERSION.fetch_add(1, Ordering::SeqCst);

            if let Some(notify) = notify {
              let mut file = std::fs::File::from(notify);
              let _ = file.write_all(&[1u8]);
            }
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

    pub fn resolve_exec(&self, app_id: &str) -> Option<String> {
      let target_id = app_id
        .split(',')
        .last()
        .unwrap_or(app_id)
        .trim()
        .to_lowercase();

      let data = self.data.read().unwrap();

      data.app_id_to_exec.get(&target_id).cloned().or_else(|| {
        target_id
          .split('.')
          .last()
          .and_then(|last_segment| data.app_id_to_exec.get(last_segment).cloned())
      })
    }

    pub fn find(&self, name: &str) -> Option<AppEntry> {
      let name = name.trim();

      if name.is_empty() {
        return None;
      }

      if name.ends_with(".desktop") {
        return self.register_path(Path::new(name));
      }

      let target = name.to_lowercase();
      let data = self.data.read().unwrap();

      data
        .apps
        .iter()
        .find(|app| app.id_lower == target || app.name_lower == target)
        .cloned()
    }

    pub fn register_path(&self, path: &Path) -> Option<AppEntry> {
      let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
      let (app, keys) = parse_entry(&path)?;

      let mut data = self.data.write().unwrap();

      for key in keys {
        Arc::make_mut(&mut data.app_id_to_name).insert(key.clone(), app.name.clone());
        Arc::make_mut(&mut data.app_id_to_exec).insert(key, app.exec.clone());
      }

      if !data.all_entries.contains(&path) {
        data.all_entries.push(path);
      }

      let duplicate = data.apps.iter().any(|existing| {
        existing.name_lower == app.name_lower && existing.exec_lower == app.exec_lower
      });

      if !duplicate {
        Arc::make_mut(&mut data.apps).push(app.clone());
        Arc::make_mut(&mut data.apps).sort_by(|a, b| a.name_lower.cmp(&b.name_lower));
      }

      drop(data);
      GLOBAL_VERSION.fetch_add(1, Ordering::SeqCst);

      Some(app)
    }
  }

  pub fn spawn_exec(exec_line: &str) {
    let clean_cmd = clean_exec(exec_line);
    if clean_cmd.is_empty() {
      return;
    }

    let mut parts = shlex_split(&clean_cmd);
    if parts.is_empty() {
      return;
    }

    let program = parts.remove(0);

    let mut cmd = Command::new(program);

    cmd
      .args(parts)
      .stdin(Stdio::null())
      .stdout(Stdio::null())
      .stderr(Stdio::null());

    unsafe {
      cmd.pre_exec(|| {
        libc::setsid();
        Ok(())
      });
    }

    let _ = cmd.spawn();
  }

  fn clean_exec(exec: &str) -> String {
    let mut result = Vec::new();
    for word in exec.split_whitespace() {
      if word.starts_with('%') {
        continue;
      }
      result.push(word);
    }
    result.join(" ")
  }

  fn shlex_split(cmd: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = ' ';

    for c in cmd.chars() {
      match c {
        '"' | '\'' if !in_quotes => {
          in_quotes = true;
          quote_char = c;
        }
        c if in_quotes && c == quote_char => {
          in_quotes = false;
        }
        c if c.is_whitespace() && !in_quotes => {
          if !current.is_empty() {
            args.push(current);
            current = String::new();
          }
        }
        _ => {
          current.push(c);
        }
      }
    }

    if !current.is_empty() {
      args.push(current);
    }

    args
  }

  #[cfg(test)]
  mod tests {
    use super::*;

    fn empty_entries() -> DesktopEntries {
      DesktopEntries {
        data: Arc::new(RwLock::new(DesktopEntriesData::default())),
        name_cache: RefCell::new(Default::default()),
      }
    }

    #[test]
    fn registers_and_finds_desktop_files() {
      let dir = std::env::temp_dir().join(format!("slowshell-desktop-test-{}", std::process::id()));
      std::fs::create_dir_all(&dir).expect("temp dir");

      let path = dir.join("SlowShellTest.desktop");
      std::fs::write(
        &path,
        "[Desktop Entry]\nType=Application\nName=SlowShell Test\n\
         Exec=/bin/true\nIcon=utilities-terminal\n",
      )
      .expect("desktop file");

      let entries = empty_entries();

      let found = entries
        .find(path.to_str().expect("utf-8 path"))
        .expect("path should be registered");
      assert_eq!(found.exec, "/bin/true");
      assert_eq!(found.icon.as_deref(), Some("utilities-terminal"));

      assert!(entries.find("slowshelltest").is_some());
      assert!(entries.find("SlowShell Test").is_some());
      assert!(entries.find("missing-app").is_none());

      std::fs::remove_dir_all(&dir).ok();
    }
  }
}
