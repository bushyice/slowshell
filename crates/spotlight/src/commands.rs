use slowshell_core::{Store, commands::commands, types::Ustr};

use crate::{SpotlightAction, SpotlightActionDef, SpotlightItem};

pub fn search_commands(query: &str, _store: &Store) -> Vec<SpotlightItem> {
  let query = query.trim_start_matches("/");

  let head = query.split_whitespace().next().unwrap_or("");
  let needle = head.to_lowercase();

  commands()
    .list()
    .into_iter()
    .filter(|command| {
      if needle.is_empty() {
        return true;
      }

      let name = command.name.to_lowercase();
      let title = command.title.as_deref().unwrap_or("").to_lowercase();
      name.contains(&needle) || title.contains(&needle)
    })
    .map(|command| {
      let title = command.display();
      let subtitle = match (&command.description, command.title.is_some()) {
        (Some(description), _) => Some(description.clone()),
        (None, true) => Some(command.name.to_string()),
        (None, false) => None,
      };

      SpotlightItem {
        image: None,
        image_bytes: None,
        image_icon: command
          .icon
          .clone()
          .or_else(|| Some("utilities-terminal-symbolic".into())),
        title,
        subtitle,
        subtext: None,
        tags: Some(vec!["Command".into()]),
        actions: vec![SpotlightActionDef {
          title: Some("Execute".into()),
          action: SpotlightAction::Execute(command.name),
        }],
      }
    })
    .collect()
}

pub fn command_args(query: &str, name: &str) -> Vec<Ustr> {
  let trimmed = query.trim_start();

  let rest = if let Some(rest) = trimmed.strip_prefix(name) {
    rest
  } else {
    match trimmed.split_once(char::is_whitespace) {
      Some((head, tail)) if !head.is_empty() && name.starts_with(head) => tail,
      _ => trimmed,
    }
  };

  slowshell_core::types::split_args(rest.trim_start()).unwrap_or_default()
}
