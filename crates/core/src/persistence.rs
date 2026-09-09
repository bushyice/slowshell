use serde::{Serialize, de::DeserializeOwned};
use std::{
  env, fs,
  path::{Path, PathBuf},
};

pub fn persistence_location() -> PathBuf {
  env::var_os("XDG_STATE_HOME")
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from(env::var_os("HOME").expect("HOME not set")).join(".local"))
    .join("slowshell")
}

fn file_for(name: &str) -> Option<PathBuf> {
  if name.is_empty() || name.contains('/') || name.contains('\\') || name == "." || name == ".." {
    return None;
  }
  Some(persistence_location().join(format!("{name}.ron")))
}

pub fn load<T: DeserializeOwned>(name: impl AsRef<str>) -> Option<T> {
  let name = name.as_ref();
  let path = file_for(name)?;
  let content = fs::read_to_string(path).ok()?;
  match ron::from_str(&content) {
    Ok(value) => Some(value),
    Err(e) => {
      eprintln!("[persistence] failed to load {name:?}: {e}");
      None
    }
  }
}

pub fn save<T: Serialize>(name: impl AsRef<str>, value: &T) -> anyhow::Result<()> {
  let name = name.as_ref();
  let path =
    file_for(name).ok_or_else(|| anyhow::anyhow!("[persistence] invalid name {name:?}"))?;
  let dir = path
    .parent()
    .ok_or_else(|| anyhow::anyhow!("invalid path"))?;
  fs::create_dir_all(dir)?;

  let content = ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::default())
    .map_err(|e| anyhow::anyhow!("[persistence] failed to serialize {name:?}: {e}"))?;

  atomic_write(&path, content.as_bytes())?;
  Ok(())
}

pub fn has(name: impl AsRef<str>) -> bool {
  file_for(name.as_ref()).is_some_and(|p| p.exists())
}

pub fn remove(name: impl AsRef<str>) -> anyhow::Result<()> {
  let name = name.as_ref();
  let path =
    file_for(name).ok_or_else(|| anyhow::anyhow!("[persistence] invalid name {name:?}"))?;
  if path.exists() {
    fs::remove_file(&path)?;
  }
  Ok(())
}

fn atomic_write(path: &Path, content: &[u8]) -> anyhow::Result<()> {
  let tmp = path.with_extension("tmp");
  fs::write(&tmp, content)?;
  fs::rename(&tmp, path)?;
  Ok(())
}
