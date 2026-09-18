use slowshell_commons::desktop::DesktopEntries;
use slowshell_core::Store;

use crate::{SpotlightAction, SpotlightActionDef, SpotlightItem};

pub use slowshell_commons::desktop::spawn_exec as spawn_app;

pub fn search_applications(query: &str, store: &Store) -> Vec<SpotlightItem> {
  let Some(entries) = store.borrow::<DesktopEntries>() else {
    return Vec::new();
  };
  let apps = entries.apps();

  let q = query.trim().to_lowercase();

  let mut scored_matches: Vec<(u32, SpotlightItem)> = Vec::new();

  for app in apps.iter() {
    let name_lower = app.name.to_lowercase();
    let score = if q.is_empty() {
      Some(100)
    } else if name_lower == q {
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
    } else if app.keywords.iter().any(|kw| kw.to_lowercase().contains(&q)) {
      Some(40)
    } else if app.exec.to_lowercase().contains(&q) {
      Some(50)
    } else if let Some(ref c) = app.comment {
      if c.to_lowercase().contains(&q) {
        Some(60)
      } else {
        None
      }
    } else {
      None
    };

    if let Some(score) = score {
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
      scored_matches.push((score, item));
    }
  }

  scored_matches.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.title.cmp(&b.1.title)));
  scored_matches.into_iter().map(|(_, item)| item).collect()
}
