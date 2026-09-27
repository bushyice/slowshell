use std::collections::HashMap;

use slowshell_config::Value;
use slowshell_core::types::Ustr;

#[derive(Clone, PartialEq, Debug)]
pub struct ComponentOptions {
  inner: HashMap<Ustr, Value>,
}

impl ComponentOptions {
  pub fn number(&self, id: impl Into<Ustr>) -> Option<f32> {
    self
      .inner
      .get(&id.into())
      .and_then(|v| v.as_float().map(|x| x as f32))
  }

  pub fn int(&self, id: impl Into<Ustr>) -> Option<i32> {
    self
      .inner
      .get(&id.into())
      .and_then(|v| v.as_int().map(|x| x as i32))
  }

  pub fn str(&self, id: impl Into<Ustr>) -> Option<&str> {
    self.inner.get(&id.into()).and_then(|v| v.as_str())
  }

  pub fn bool(&self, id: impl Into<Ustr>) -> Option<bool> {
    self.inner.get(&id.into()).and_then(|v| v.as_bool())
  }

  pub fn table(&self, id: impl Into<Ustr>) -> Option<&[(String, Value)]> {
    self.inner.get(&id.into()).and_then(|v| v.as_table())
  }

  pub fn strings(&self, id: impl Into<Ustr>) -> Option<Vec<&str>> {
    self
      .inner
      .get(&id.into())
      .and_then(|v| v.as_array())
      .map(|array| array.iter().filter_map(|v| v.as_str()).collect())
  }

  pub fn is_empty(&self) -> bool {
    self.inner.is_empty()
  }
}

impl From<HashMap<Ustr, Value>> for ComponentOptions {
  fn from(value: HashMap<Ustr, Value>) -> Self {
    ComponentOptions { inner: value }
  }
}
