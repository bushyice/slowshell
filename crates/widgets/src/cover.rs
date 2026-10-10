use std::{
  collections::{HashMap, HashSet},
  hash::{Hash, Hasher},
  path::PathBuf,
  sync::{Mutex, OnceLock},
  time::SystemTime,
};

use iced::widget::image;

struct CachedCover {
  modified: Option<SystemTime>,
  handle: image::Handle,
}

static COVERS: OnceLock<Mutex<HashMap<String, CachedCover>>> = OnceLock::new();
static DOWNLOADS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

pub fn mpris_cover_handle(art_url: &str) -> Option<image::Handle> {
  let url = art_url.trim();

  if url.is_empty() {
    return None;
  }

  let path = match resolve_cover(url) {
    Some(path) => path,
    None => return None,
  };

  let modified = std::fs::metadata(&path).ok()?.modified().ok();
  let cache = COVERS.get_or_init(|| Mutex::new(HashMap::new()));

  if let Ok(guard) = cache.lock()
    && let Some(cached) = guard.get(url)
    && cached.modified == modified
  {
    return Some(cached.handle.clone());
  }

  let handle = image::Handle::from_path(&path);

  if let Ok(mut guard) = cache.lock() {
    guard.insert(
      url.to_string(),
      CachedCover {
        modified,
        handle: handle.clone(),
      },
    );
  }

  Some(handle)
}

fn resolve_cover(url: &str) -> Option<PathBuf> {
  if let Some(path) = local_path(url) {
    return path.exists().then_some(path);
  }

  if url.starts_with("http://") || url.starts_with("https://") {
    let path = remote_cache_path(url);

    if path.exists() {
      return Some(path);
    }

    download(url, path);
  }

  None
}

fn local_path(url: &str) -> Option<PathBuf> {
  let raw = url.strip_prefix("file://").unwrap_or(url);

  if raw.starts_with("http://")
    || raw.starts_with("https://")
    || raw.starts_with("data:")
    || raw.starts_with("//")
  {
    return None;
  }

  let path = PathBuf::from(percent_decode(raw));

  path.is_absolute().then_some(path)
}

fn remote_cache_path(url: &str) -> PathBuf {
  let base = std::env::var_os("XDG_CACHE_HOME")
    .map(PathBuf::from)
    .filter(|path| path.is_absolute())
    .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
    .unwrap_or_else(std::env::temp_dir)
    .join("slowshell/covers");

  let mut hasher = std::collections::hash_map::DefaultHasher::new();
  url.hash(&mut hasher);

  base.join(format!("{:016x}.img", hasher.finish()))
}

fn download(url: &str, path: PathBuf) {
  let in_flight = DOWNLOADS.get_or_init(|| Mutex::new(HashSet::new()));

  if let Ok(mut guard) = in_flight.lock() {
    if !guard.insert(url.to_string()) {
      return;
    }
  } else {
    return;
  }

  let url = url.to_string();

  std::thread::spawn(move || {
    if let Some(dir) = path.parent() {
      let _ = std::fs::create_dir_all(dir);
    }

    let tmp = path.with_extension("tmp");
    let status = std::process::Command::new("curl")
      .args(["-fsSL", "--max-time", "10", "-o"])
      .arg(&tmp)
      .arg(&url)
      .status();

    let ok = status.map(|status| status.success()).unwrap_or(false);

    if ok {
      let _ = std::fs::rename(&tmp, &path);
    } else {
      let _ = std::fs::remove_file(&tmp);
    }
  });
}

fn percent_decode(input: &str) -> String {
  let bytes = input.as_bytes();
  let mut out = Vec::with_capacity(bytes.len());
  let mut index = 0;

  while index < bytes.len() {
    if bytes[index] == b'%'
      && index + 2 < bytes.len()
      && let (Some(high), Some(low)) = (hex(bytes[index + 1]), hex(bytes[index + 2]))
    {
      out.push(high * 16 + low);
      index += 3;
      continue;
    }

    out.push(bytes[index]);
    index += 1;
  }

  String::from_utf8_lossy(&out).into_owned()
}

fn hex(byte: u8) -> Option<u8> {
  match byte {
    b'0'..=b'9' => Some(byte - b'0'),
    b'a'..=b'f' => Some(byte - b'a' + 10),
    b'A'..=b'F' => Some(byte - b'A' + 10),
    _ => None,
  }
}
