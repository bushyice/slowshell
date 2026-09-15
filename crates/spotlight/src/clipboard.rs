use std::{
  path::PathBuf,
  process::{Command, Stdio},
  sync::{Mutex, OnceLock},
  time::{Duration, Instant},
};

use slowshell_core::Store;

use crate::{SpotlightAction, SpotlightActionDef, SpotlightItem};

const MAX_TITLE: usize = 72;

#[derive(Clone)]
struct Row {
  line: String,
  preview: String,
  is_image: bool,
}

type RowsCache = Option<(Instant, Vec<Row>)>;

static ROWS: OnceLock<Mutex<RowsCache>> = OnceLock::new();

pub fn search_clipboard(query: &str, _store: &Store) -> Vec<SpotlightItem> {
  let q = query.trim().to_lowercase();

  rows()
    .into_iter()
    .filter(|row| {
      q.is_empty()
        || row.preview.to_lowercase().contains(&q)
        || row.line.to_lowercase().contains(&q)
    })
    .map(|row| {
      let (title, subtitle) = if row.is_image {
        let dims = row
          .preview
          .split_whitespace()
          .find(|t| t.contains('x') && t.bytes().all(|b| b.is_ascii_digit() || b == b'x'))
          .map(ToString::to_string);
        let mime = row
          .preview
          .split_whitespace()
          .filter(|t| t.bytes().all(|b| b.is_ascii_alphanumeric()))
          .find(|t| {
            matches!(
              t.to_lowercase().as_str(),
              "png" | "jpg" | "jpeg" | "gif" | "webp"
            )
          })
          .map(ToString::to_string);
        (
          "Image".to_string(),
          dims
            .map(|d| match &mime {
              Some(m) => format!("{m} · {d}"), // cool dot: ·
              None => d,
            })
            .unwrap_or_else(|| mime.unwrap_or_else(|| "image".to_string())),
        )
      } else {
        (
          truncate(&row.preview),
          format!("{} chars", row.preview.chars().count()),
        )
      };

      SpotlightItem {
        image: None,
        image_bytes: None,
        image_icon: Some(
          if row.is_image {
            "image-x-generic-symbolic"
          } else {
            "edit-paste-symbolic"
          }
          .into(),
        ),
        title,
        subtitle: Some(subtitle),
        subtext: Some("[Enter] Copy".into()),
        tags: Some(vec![if row.is_image { "Image" } else { "Text" }.into()]),
        actions: vec![SpotlightActionDef {
          title: Some("Copy".into()),
          action: SpotlightAction::Clip(row.line),
        }],
      }
    })
    .collect()
}

pub fn copy(line: &str) {
  let pipeline = if line.contains("[[ binary data") {
    "cliphist decode | wl-copy -t image/png"
  } else {
    "cliphist decode | wl-copy"
  };

  let Ok(mut child) = Command::new("sh")
    .arg("-c")
    .arg(pipeline)
    .stdin(Stdio::piped())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .spawn()
  else {
    return;
  };

  if let Some(mut stdin) = child.stdin.take() {
    use std::io::Write;
    let _ = writeln!(stdin, "{line}");
  }
  let _ = child.wait();
}

fn rows() -> Vec<Row> {
  const TTL: Duration = Duration::from_secs(1);

  let cache = ROWS.get_or_init(|| Mutex::new(None));
  let now = Instant::now();

  if let Some((at, rows)) = cache.lock().unwrap().as_ref()
    && now.duration_since(*at) < TTL
  {
    return rows.clone();
  }

  let fresh = load_rows();
  *cache.lock().unwrap() = Some((now, fresh.clone()));
  fresh
}

fn load_rows() -> Vec<Row> {
  let output = match Command::new("cliphist")
    .arg("list")
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .output()
  {
    Ok(out) if out.status.success() => out.stdout,
    _ => return Vec::new(),
  };

  let mut rows = Vec::new();
  for line in String::from_utf8_lossy(&output).lines() {
    match line.split_once('\t') {
      Some((id, preview)) if !id.is_empty() && !preview.is_empty() => {
        rows.push(Row {
          line: line.to_string(),
          preview: preview.to_string(),
          is_image: preview.starts_with("[[ binary data"),
        });
      }
      _ => {}
    }
  }
  rows
}

fn truncate(s: &str) -> String {
  if s.chars().count() <= MAX_TITLE {
    s.to_string()
  } else {
    let cut: String = s.chars().take(MAX_TITLE).collect();
    format!("{cut}…")
  }
}

pub fn version() -> u64 {
  db_paths()
    .find_map(|p| std::fs::metadata(&p).and_then(|m| m.modified()).ok())
    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
    .map(|d| d.as_nanos() as u64)
    .unwrap_or(0)
}

fn db_paths() -> impl Iterator<Item = PathBuf> {
  let cache_dir = std::env::var_os("XDG_CACHE_HOME")
    .map(PathBuf::from)
    .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")));
  std::iter::once(cache_dir.map(|c| c.join("cliphist").join("db"))).flatten()
}
