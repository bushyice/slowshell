use core::{ffi::c_void, fmt};

use abi_stable::{
  StableAbi, sabi_trait,
  std_types::{RBox, ROption, RStr},
};

use crate::{Args, TypeKey};

#[repr(C)]
#[derive(StableAbi)]
pub struct PayloadValue {
  key: TypeKey,
  ptr: *mut c_void,
  clone_fn: extern "C" fn(*const c_void) -> *mut c_void,
  drop_fn: extern "C" fn(*mut c_void),
}

unsafe impl Send for PayloadValue {}
unsafe impl Sync for PayloadValue {}

impl PayloadValue {
  fn new<T: Clone + Send + Sync + 'static>(value: T) -> Self {
    extern "C" fn clone_it<T: Clone + Send + Sync + 'static>(ptr: *const c_void) -> *mut c_void {
      let value = unsafe { &*(ptr as *const T) };
      Box::into_raw(Box::new(value.clone())) as *mut c_void
    }

    extern "C" fn drop_it<T>(ptr: *mut c_void) {
      unsafe { drop(Box::from_raw(ptr as *mut T)) };
    }

    Self {
      key: TypeKey::of::<T>(),
      ptr: Box::into_raw(Box::new(value)) as *mut c_void,
      clone_fn: clone_it::<T>,
      drop_fn: drop_it::<T>,
    }
  }

  fn clone_value(&self) -> Self {
    Self {
      key: self.key,
      ptr: (self.clone_fn)(self.ptr as *const c_void),
      clone_fn: self.clone_fn,
      drop_fn: self.drop_fn,
    }
  }

  fn is<T: 'static>(&self) -> bool {
    self.key.matches::<T>()
  }

  fn as_ref<T: 'static>(&self) -> Option<&T> {
    if self.is::<T>() {
      Some(unsafe { &*(self.ptr as *const T) })
    } else {
      None
    }
  }

  fn as_mut<T: 'static>(&mut self) -> Option<&mut T> {
    if self.is::<T>() {
      Some(unsafe { &mut *(self.ptr as *mut T) })
    } else {
      None
    }
  }
}

impl Clone for PayloadValue {
  fn clone(&self) -> Self {
    self.clone_value()
  }
}

impl Drop for PayloadValue {
  fn drop(&mut self) {
    if !self.ptr.is_null() {
      (self.drop_fn)(self.ptr);
      self.ptr = core::ptr::null_mut();
    }
  }
}

#[repr(C)]
#[derive(StableAbi)]
pub struct PayloadBox {
  value: PayloadValue,
}

impl PayloadBox {
  pub fn new<T: Clone + Send + Sync + 'static>(value: T) -> Self {
    Self {
      value: PayloadValue::new(value),
    }
  }

  pub fn is<T: 'static>(&self) -> bool {
    self.value.is::<T>()
  }

  pub fn as_this<T: 'static>(&self) -> Option<&T> {
    self.value.as_ref::<T>()
  }

  pub fn as_this_mut<T: 'static>(&mut self) -> Option<&mut T> {
    self.value.as_mut::<T>()
  }

  pub fn type_key(&self) -> TypeKey {
    self.value.key
  }
}

impl Clone for PayloadBox {
  fn clone(&self) -> Self {
    Self {
      value: self.value.clone_value(),
    }
  }
}

impl fmt::Debug for PayloadBox {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "PayloadBox<{}>", self.value.key.name)
  }
}

#[sabi_trait]
pub trait PayloadParser: Send + Sync + 'static {
  fn parse(&self, command: RStr<'_>, args: &Args) -> ROption<PayloadBox>;
}

pub type PayloadParserBox = PayloadParser_TO<RBox<()>>;
