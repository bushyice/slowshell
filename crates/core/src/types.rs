use smol_str::SmolStr;
use std::borrow::Borrow;
use std::fmt::Display;
use std::ops::Deref;

#[derive(
  Clone, serde::Serialize, serde::Deserialize, Hash, PartialEq, Eq, Debug, PartialOrd, Ord,
)]
pub struct Ustr(SmolStr);

pub trait ToUstr {
  fn to_ustr(&self) -> Ustr;
}

impl ToUstr for str {
  fn to_ustr(&self) -> Ustr {
    Ustr(SmolStr::new(self))
  }
}

impl ToUstr for String {
  fn to_ustr(&self) -> Ustr {
    Ustr(SmolStr::new(self))
  }
}

impl Deref for Ustr {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    self.0.as_str()
  }
}

impl From<String> for Ustr {
  fn from(value: String) -> Self {
    Ustr(SmolStr::new(value))
  }
}

impl From<&str> for Ustr {
  fn from(value: &str) -> Self {
    Ustr(SmolStr::new(value))
  }
}

impl From<&String> for Ustr {
  fn from(value: &String) -> Self {
    Ustr(SmolStr::new(value))
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
    Ustr(SmolStr::new(""))
  }
}

#[allow(non_camel_case_types)]
pub type Void = ();
#[allow(non_upper_case_globals)]
pub const Void: Void = ();
