use smol_str::SmolStr;
use std::any::Any;
use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Display};
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

impl From<&&str> for Ustr {
  fn from(value: &&str) -> Self {
    Ustr(SmolStr::new(*value))
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

pub trait CloneAny: Any + Send + Sync {
  fn clone_any(&self) -> Box<dyn CloneAny>;
  fn as_any(&self) -> &dyn Any;
  fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T> CloneAny for T
where
  T: Any + Clone + Send + Sync,
{
  fn clone_any(&self) -> Box<dyn CloneAny> {
    Box::new(self.clone())
  }

  fn as_any(&self) -> &dyn Any {
    self
  }

  fn as_any_mut(&mut self) -> &mut dyn Any {
    self
  }
}

pub struct PayloadBox(Box<dyn CloneAny>);

impl Debug for PayloadBox {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "PayloadBox")
  }
}

impl Clone for PayloadBox {
  fn clone(&self) -> Self {
    Self((*self.0).clone_any())
  }
}

impl PayloadBox {
  pub fn new(item: impl CloneAny) -> Self {
    Self(Box::new(item))
  }

  pub fn enforce<T: 'static>(&self) -> &T {
    (*self.0).as_any().downcast_ref::<T>().unwrap()
  }

  pub fn as_this<T: 'static>(&self) -> Option<&T> {
    (*self.0).as_any().downcast_ref::<T>()
  }

  pub fn enforce_mut<T: 'static>(&mut self) -> &mut T {
    (*self.0).as_any_mut().downcast_mut::<T>().unwrap()
  }

  pub fn as_this_mut<T: 'static>(&mut self) -> Option<&mut T> {
    (*self.0).as_any_mut().downcast_mut::<T>()
  }
}

pub trait OptionalPayloadBox {
  fn transform<T: Clone + Send + Sync + 'static>(&self) -> Option<&T>;
}

impl OptionalPayloadBox for Option<PayloadBox> {
  fn transform<T: Clone + Send + Sync + 'static>(&self) -> Option<&T> {
    self.as_ref().and_then(|x| x.as_this::<T>())
  }
}

pub struct PayloadBuilder {
  pub commands: &'static [&'static str],
  pub build: fn(&str, PayloadBuilderArgs) -> Option<PayloadBox>,
}

impl PayloadBuilder {
  pub fn into_boxed(self) -> Box<PayloadBuilder> {
    Box::new(self)
  }
}

pub struct PayloadBuilderArgs<'a>(pub &'a [Ustr]);

impl<'a> PayloadBuilderArgs<'a> {
  pub fn as_map(&self) -> HashMap<&'a str, &'a str> {
    self
      .0
      .iter()
      .map(|x| x.split_once("=").unwrap_or_else(|| (x, "_")))
      .collect()
  }
}

pub struct PayloadBuilderRegistry {
  builders: Vec<PayloadBuilder>,
  exact: HashMap<Ustr, usize>,
  matches: HashMap<Ustr, HashSet<usize>>,
}

impl PayloadBuilderRegistry {
  pub fn new() -> Self {
    Self {
      builders: Vec::new(),
      exact: HashMap::new(),
      matches: HashMap::new(),
    }
  }

  pub fn register(&mut self, builder: PayloadBuilder) {
    let cmds = builder.commands;
    let index = self.builders.len();
    self.builders.push(builder);

    for cmd in cmds {
      if cmd.ends_with(".") {
        self.matches.entry((*cmd).into()).or_default().insert(index);
      } else {
        self.exact.insert((*cmd).into(), index);
      }
    }
  }

  pub fn build(&self, command: &str, args: &[Ustr]) -> Option<PayloadBox> {
    let args = PayloadBuilderArgs(args);

    if let Some(&index) = self.exact.get(command) {
      return (self.builders.get(index)?.build)(command, args);
    }

    let match_key = if let Some((name, _)) = command.split_once(".") {
      &command[..name.len() + 1]
    } else {
      &command
    };

    for &index in self.matches.get(match_key)? {
      return (self.builders.get(index)?.build)(command, args);
    }

    None
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_downcast_payload() {
    let f = PayloadBox::new(12u32);

    println!("{}", std::any::type_name_of_val(f.0.as_any()));

    assert!(f.as_this::<u32>().copied() == Some(12));
  }
}
