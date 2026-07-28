use serde::Deserialize;
use serde::de::Deserializer;
use std::borrow::Borrow;
use std::fmt::Display;
use std::ops::Deref;

use strumbra::UniqueString;

#[derive(
  Clone, serde::Serialize, serde::Deserialize, Hash, PartialEq, Eq, Debug, PartialOrd, Ord,
)]
pub struct Ustr(#[serde(serialize_with = "ser_name", deserialize_with = "de_name")] UniqueString);

pub trait ToUstr {
  fn to_ustr(&self) -> Ustr;
}

impl ToUstr for str {
  fn to_ustr(&self) -> Ustr {
    Ustr::from(self)
  }
}

impl ToUstr for String {
  fn to_ustr(&self) -> Ustr {
    Ustr::from(self.as_str())
  }
}

impl Deref for Ustr {
  type Target = UniqueString;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl From<String> for Ustr {
  fn from(value: String) -> Self {
    Ustr(UniqueString::try_from(value).unwrap())
  }
}

impl From<&str> for Ustr {
  fn from(value: &str) -> Self {
    Ustr(UniqueString::try_from(value).unwrap())
  }
}

impl From<&String> for Ustr {
  fn from(value: &String) -> Self {
    Ustr(UniqueString::try_from(value).unwrap())
  }
}

impl Display for Ustr {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0.as_str())
  }
}

impl Borrow<str> for Ustr {
  fn borrow(&self) -> &str {
    self.0.as_str()
  }
}

impl AsRef<std::ffi::OsStr> for Ustr {
  fn as_ref(&self) -> &std::ffi::OsStr {
    std::ffi::OsStr::new(self.0.as_str())
  }
}

impl Default for Ustr {
  fn default() -> Self {
    Ustr(UniqueString::try_from("").unwrap())
  }
}

fn ser_name<S: serde::Serializer>(f: &UniqueString, serializer: S) -> Result<S::Ok, S::Error> {
  serializer.collect_str(&f.to_string())
}

fn de_name<'de, D: Deserializer<'de>>(deserializer: D) -> Result<UniqueString, D::Error> {
  let s: String = Deserialize::deserialize(deserializer)?;
  Ok(UniqueString::try_from(s).unwrap())
}

#[allow(non_camel_case_types)]
pub type Void = ();
#[allow(non_upper_case_globals)]
pub const Void: Void = ();
