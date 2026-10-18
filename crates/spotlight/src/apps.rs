use std::{
  collections::HashMap,
  sync::{
    LazyLock, Mutex, RwLock,
    atomic::{AtomicU64, Ordering},
  },
};

use slowshell_commons::desktop::{AppEntry, DesktopEntries};
use slowshell_core::{Store, persistence};

use crate::{SpotlightAction, SpotlightActionDef, SpotlightItem};

pub const USAGE_FILE: &str = "spotlight-app-usage";

static USAGE: LazyLock<Mutex<HashMap<String, u64>>> =
  LazyLock::new(|| Mutex::new(persistence::load(USAGE_FILE).unwrap_or_default()));

static USAGE_VERSION: AtomicU64 = AtomicU64::new(0);

static EXEC_INDEX: LazyLock<RwLock<HashMap<String, String>>> =
  LazyLock::new(|| RwLock::new(HashMap::new()));

static EXEC_INDEX_VERSION: AtomicU64 = AtomicU64::new(u64::MAX);

pub fn applications_version() -> u64 {
  DesktopEntries::global_version().wrapping_add(USAGE_VERSION.load(Ordering::Relaxed))
}

pub fn spawn_app(exec_line: &str) {
  record_launch(exec_line);
  slowshell_commons::desktop::spawn_exec(exec_line);
}

fn record_launch(exec_line: &str) {
  let Some(id) = EXEC_INDEX.read().unwrap().get(exec_line).cloned() else {
    return;
  };

  let counts = {
    let mut usage = USAGE.lock().unwrap();
    let count = usage.entry(id).or_insert(0);
    *count = count.saturating_add(1);
    usage.clone()
  };

  USAGE_VERSION.fetch_add(1, Ordering::Relaxed);

  if let Err(error) = persistence::save(USAGE_FILE, &counts) {
    eprintln!("[spotlight] failed to persist application usage: {error}");
  }
}

pub fn search_applications(query: &str, store: &Store) -> Vec<SpotlightItem> {
  let Some(entries) = store.borrow::<DesktopEntries>() else {
    return Vec::new();
  };
  let apps = entries.apps();
  refresh_index(&apps);
  let usage = USAGE.lock().unwrap();
  search(&apps, query, &usage)
}

fn refresh_index(apps: &[AppEntry]) {
  let version = DesktopEntries::global_version();
  let mut index = EXEC_INDEX.write().unwrap();

  if EXEC_INDEX_VERSION.load(Ordering::Relaxed) == version && !index.is_empty() {
    return;
  }

  index.clear();
  for app in apps {
    index.insert(app.exec.clone(), app.id_lower.clone());
    for action in &app.actions {
      index.insert(action.exec.clone(), app.id_lower.clone());
    }
  }

  EXEC_INDEX_VERSION.store(version, Ordering::Relaxed);
}

fn search(apps: &[AppEntry], query: &str, usage: &HashMap<String, u64>) -> Vec<SpotlightItem> {
  let q = query.trim().to_lowercase();
  let empty_query = q.is_empty();

  let mut scored_matches: Vec<(u32, u64, SpotlightItem)> = Vec::new();

  for app in apps.iter() {
    let name_lower = &app.name_lower;
    let score = if empty_query {
      Some(100)
    } else if name_lower == &q {
      Some(0)
    } else if name_lower.starts_with(&q) {
      Some(10)
    } else if name_lower
      .split_whitespace()
      .any(|word| word.starts_with(&q))
    {
      Some(20)
    } else if name_lower.contains(&q) {
      Some(30)
    } else if app.keywords_lower.iter().any(|kw| kw.contains(&q)) {
      Some(40)
    } else if app.exec_lower.contains(&q) {
      Some(50)
    } else if let Some(c) = &app.comment_lower {
      if c.contains(&q) { Some(60) } else { None }
    } else {
      None
    };

    if let Some(score) = score {
      let opened = if empty_query {
        usage.get(&app.id_lower).copied().unwrap_or(0)
      } else {
        0
      };

      let exec = app.exec.clone();
      let mut action_defs = vec![SpotlightActionDef {
        title: Some("Open".into()),
        action: SpotlightAction::Exec(exec.clone()),
      }];
      action_defs.extend(app.actions.iter().map(|act| SpotlightActionDef {
        title: Some(act.name.clone()),
        action: SpotlightAction::Exec(act.exec.clone()),
      }));
      action_defs.push(SpotlightActionDef {
        title: Some("Copy command".into()),
        action: SpotlightAction::Copy(exec),
      });
      let item = SpotlightItem {
        image: None,
        image_bytes: None,
        image_icon: app.icon.clone(),
        title: app.name.clone(),
        subtitle: app.comment.clone(),
        subtext: None,
        tags: if app.categories.is_empty() {
          None
        } else {
          Some(app.categories.clone())
        },
        actions: action_defs,
      };
      scored_matches.push((score, opened, item));
    }
  }

  scored_matches.sort_by(|a, b| {
    a.0
      .cmp(&b.0)
      .then_with(|| b.1.cmp(&a.1))
      .then_with(|| a.2.title.cmp(&b.2.title))
  });
  scored_matches
    .into_iter()
    .map(|(_, _, item)| item)
    .collect()
}
