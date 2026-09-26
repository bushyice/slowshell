use core::{ffi::c_void, marker::PhantomData};

use abi_stable::{
  StableAbi, sabi_trait,
  std_types::{RBox, ROption},
};

use crate::host::Context;

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypeKey {
  pub name: abi_stable::std_types::RStr<'static>,
  pub size: usize,
  pub align: usize,
}

impl TypeKey {
  pub fn of<T: 'static>() -> Self {
    Self {
      name: abi_stable::std_types::RStr::from_str(std::any::type_name::<T>()),
      size: core::mem::size_of::<T>(),
      align: core::mem::align_of::<T>(),
    }
  }

  pub fn matches<T: 'static>(&self) -> bool {
    self == &Self::of::<T>()
  }
}

#[repr(C)]
#[derive(StableAbi)]
pub struct ErasedValue {
  pub key: TypeKey,
  pub ptr: *mut c_void,
  pub drop_fn: extern "C" fn(*mut c_void),
}

impl ErasedValue {
  pub fn new<T: 'static>(value: T) -> Self {
    extern "C" fn drop_it<T>(ptr: *mut c_void) {
      unsafe { drop(Box::from_raw(ptr as *mut T)) };
    }
    Self {
      key: TypeKey::of::<T>(),
      ptr: Box::into_raw(Box::new(value)) as *mut c_void,
      drop_fn: drop_it::<T>,
    }
  }

  pub fn key(&self) -> TypeKey {
    self.key
  }

  pub fn take<T: 'static>(mut self) -> Option<T> {
    if !self.key.matches::<T>() {
      return None;
    }
    let ptr = self.ptr;
    self.ptr = core::ptr::null_mut();
    Some(unsafe { *Box::from_raw(ptr as *mut T) })
  }
}

impl Drop for ErasedValue {
  fn drop(&mut self) {
    if !self.ptr.is_null() {
      (self.drop_fn)(self.ptr);
      self.ptr = core::ptr::null_mut();
    }
  }
}

#[repr(C)]
#[derive(StableAbi, Clone, Copy)]
pub struct ErasedRef<'a> {
  key: TypeKey,
  ptr: *mut c_void,
  marker: PhantomData<&'a mut ()>,
}

impl<'a> ErasedRef<'a> {
  pub fn new(key: TypeKey, ptr: *mut c_void) -> Self {
    Self {
      key,
      ptr,
      marker: PhantomData,
    }
  }

  pub fn key(&self) -> TypeKey {
    self.key
  }

  pub fn cast<T: 'static>(&self) -> Option<&T> {
    if self.key.matches::<T>() {
      Some(unsafe { &*(self.ptr as *const T) })
    } else {
      None
    }
  }

  pub fn cast_mut<T: 'static>(&self) -> Option<&mut T> {
    if self.key.matches::<T>() {
      Some(unsafe { &mut *(self.ptr as *mut T) })
    } else {
      None
    }
  }
}

#[sabi_trait]
pub trait StoreCallback: Send + Sync + 'static {
  fn notify(&self, ctx: &Context<'_>);
}

pub type StoreCallbackBox = StoreCallback_TO<RBox<()>>;

struct CallbackFn<F>(F);

impl<F> StoreCallback for CallbackFn<F>
where
  F: Fn(&Context<'_>) + Send + Sync + 'static,
{
  fn notify(&self, ctx: &Context<'_>) {
    (self.0)(ctx)
  }
}

#[derive(Clone, Copy)]
pub struct Store<'a> {
  ctx: Context<'a>,
}

impl<'a> Store<'a> {
  pub fn new(ctx: Context<'a>) -> Self {
    Self { ctx }
  }

  pub fn insert<T: 'static>(&self, value: T) -> bool {
    self
      .ctx
      .host()
      .store_insert(TypeKey::of::<T>(), ErasedValue::new(value))
  }

  pub fn contains<T: 'static>(&self) -> bool {
    self.ctx.host().store_contains(TypeKey::of::<T>())
  }

  pub fn with<T: 'static, R>(&self, f: impl FnOnce(&mut T) -> R) -> Option<R> {
    match self.ctx.host().store_get(&self.ctx, TypeKey::of::<T>()) {
      ROption::RSome(value) => value.cast_mut::<T>().map(f),
      ROption::RNone => None,
    }
  }

  pub fn borrow<T: 'static>(&self) -> Option<StoreBorrow<'a, T>> {
    match self.ctx.host().store_get(&self.ctx, TypeKey::of::<T>()) {
      ROption::RSome(value) => value.cast::<T>().map(|ptr| StoreBorrow {
        ptr: ptr as *const T,
        marker: PhantomData,
      }),
      ROption::RNone => None,
    }
  }

  pub fn touch<T: 'static>(&self) -> bool {
    self.ctx.host().store_touch(TypeKey::of::<T>())
  }

  pub fn remove<T: 'static>(&self) -> Option<T> {
    match self.ctx.host().store_remove(TypeKey::of::<T>()) {
      ROption::RSome(value) => value.take::<T>(),
      ROption::RNone => None,
    }
  }

  pub fn subscribe<T: 'static, F>(&self, callback: F)
  where
    F: Fn(&Context<'_>) + Send + Sync + 'static,
  {
    self.ctx.host().store_subscribe(
      TypeKey::of::<T>(),
      StoreCallbackBox::from_value(CallbackFn(callback), abi_stable::sabi_trait::TD_Opaque),
    );
  }
}

pub struct StoreBorrow<'a, T> {
  ptr: *const T,
  marker: PhantomData<&'a T>,
}

impl<T> core::ops::Deref for StoreBorrow<'_, T> {
  type Target = T;

  fn deref(&self) -> &T {
    unsafe { &*self.ptr }
  }
}
