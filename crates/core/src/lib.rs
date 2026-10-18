// TODO: Add comments for api-ref

use std::{
  any::{Any, TypeId},
  collections::HashMap,
  ffi::c_void,
};

use crate::types::{PayloadBox, PayloadBuilderRegistry, Ustr};

pub mod animations;
pub mod commands;
pub mod listeners;
pub mod message;
pub mod monitors;
pub mod persistence;
pub mod types;

#[derive(Clone)]
pub struct ActionDispatcher(pub futures_channel::mpsc::UnboundedSender<crate::message::Message>);

impl ActionDispatcher {
  pub fn dispatch(&self, command: &str) -> bool {
    self.dispatch_payload(command, None)
  }

  pub fn dispatch_named(&self, name: &str) -> bool {
    self.send(crate::listeners::ListenerAction::Named(Ustr::from(name)))
  }

  pub fn dispatch_payload(&self, name: &str, payload: Option<PayloadBox>) -> bool {
    self.send(crate::listeners::ListenerAction::Payload {
      name: Ustr::from(name),
      payload,
    })
  }

  pub fn dispatch_with_args(
    &self,
    name: &str,
    args: &[Ustr],
    builders: &PayloadBuilderRegistry,
  ) -> bool {
    match builders.build(name, args) {
      Some(payload) => self.dispatch_payload(name, Some(payload)),
      None => self.dispatch_named(name),
    }
  }

  pub fn dispatch_action(&self, action: crate::listeners::ListenerAction) -> bool {
    self.send(action)
  }

  fn send(&self, action: crate::listeners::ListenerAction) -> bool {
    self
      .0
      .unbounded_send(crate::message::Message::FdUpdate(action))
      .is_ok()
  }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Workspace {
  pub id: u64,
  pub idx: u8,
  pub name: Option<String>,
  pub output: Option<String>,
  pub is_urgent: bool,
  pub is_active: bool,
  pub is_focused: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Monitor {
  pub name: Ustr,
  pub active_workspace: u32,
  pub width: u32,
  pub height: u32,
  pub scale: f64,
}

impl Monitor {
  pub fn named(name: impl Into<Ustr>) -> Self {
    Self {
      name: name.into(),
      active_workspace: 0,
      width: 0,
      height: 0,
      scale: 1.0,
    }
  }
}

#[derive(Debug, Clone)]
pub struct Window {
  pub id: u64,
  pub title: String,
  pub class: String,
  pub is_active: bool,
  // TODO: Make metadata into a proper map
  pub metadata: HashMap<String, String>,
}

#[doc(hidden)]
trait HandleStateTuple: Sized {
  fn run<R>(
    state: &mut Store,
    f: impl FnOnce(&mut Store, Self) -> miette::Result<R>,
  ) -> miette::Result<R>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StoreKey {
  pub type_name: &'static str,
  pub size: usize,
  pub align: usize,
}

impl StoreKey {
  pub fn of<T: 'static>() -> Self {
    Self {
      type_name: std::any::type_name::<T>(),
      size: core::mem::size_of::<T>(),
      align: core::mem::align_of::<T>(),
    }
  }

  pub fn matches<T: 'static>(&self) -> bool {
    self == &Self::of::<T>()
  }
}

pub struct ErasedEntry {
  pub key: StoreKey,
  pub ptr: *mut c_void,
  pub drop_fn: extern "C" fn(*mut c_void),
}

impl ErasedEntry {
  pub fn new<T: 'static>(value: T) -> Self {
    extern "C" fn drop_it<T>(ptr: *mut c_void) {
      unsafe { drop(Box::from_raw(ptr as *mut T)) };
    }
    Self {
      key: StoreKey::of::<T>(),
      ptr: Box::into_raw(Box::new(value)) as *mut c_void,
      drop_fn: drop_it::<T>,
    }
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

impl Drop for ErasedEntry {
  fn drop(&mut self) {
    if !self.ptr.is_null() {
      (self.drop_fn)(self.ptr);
      self.ptr = core::ptr::null_mut();
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum StoreId {
  TypeId(TypeId),
  Erased(StoreKey),
}

enum Entry {
  Any(Box<dyn Any>),
  Erased(ErasedEntry),
}

impl Entry {
  fn ptr_mut<T: 'static>(&mut self) -> Option<*mut T> {
    match self {
      Entry::Any(boxed) => boxed.downcast_mut::<T>().map(|value| value as *mut T),
      Entry::Erased(entry) => {
        if entry.key.matches::<T>() {
          Some(entry.ptr as *mut T)
        } else {
          None
        }
      }
    }
  }
}

pub struct Store {
  contents: HashMap<StoreId, Entry>,
}

impl Store {
  pub fn new() -> Self {
    Self {
      contents: Default::default(),
    }
  }

  pub fn insert_raw(&mut self, type_id: TypeId, val: Box<dyn Any>) -> bool {
    let id = StoreId::TypeId(type_id);
    if self.contents.contains_key(&id) {
      return false;
    }

    self.contents.insert(id, Entry::Any(val));
    true
  }

  pub fn insert<T: 'static>(&mut self, val: T) -> bool {
    let id = StoreId::Erased(StoreKey::of::<T>());
    if self.contents.contains_key(&id) {
      return false;
    }

    self
      .contents
      .insert(id, Entry::Erased(ErasedEntry::new(val)));
    true
  }

  fn lookup_ids<T: 'static>() -> [StoreId; 2] {
    [
      StoreId::Erased(StoreKey::of::<T>()),
      StoreId::TypeId(TypeId::of::<T>()),
    ]
  }

  pub fn borrow<T: 'static>(&self) -> Option<&T> {
    for id in Self::lookup_ids::<T>() {
      match self.contents.get(&id) {
        Some(Entry::Any(boxed)) => {
          if let Some(value) = boxed.downcast_ref::<T>() {
            return Some(value);
          }
        }
        Some(Entry::Erased(entry)) if entry.key.matches::<T>() => {
          return Some(unsafe { &*(entry.ptr as *const T) });
        }
        _ => {}
      }
    }
    None
  }

  pub fn borrow_mut<T: 'static>(&mut self) -> Option<&mut T> {
    for id in Self::lookup_ids::<T>() {
      let matches = match self.contents.get_mut(&id) {
        Some(entry) => entry.ptr_mut::<T>(),
        None => None,
      };
      if let Some(ptr) = matches {
        return Some(unsafe { &mut *ptr });
      }
    }
    None
  }

  pub fn remove<T: 'static>(&mut self) -> Option<T> {
    for id in Self::lookup_ids::<T>() {
      let matches = matches!(self.contents.get(&id), Some(Entry::Erased(entry)) if entry.key.matches::<T>())
        || matches!(self.contents.get(&id), Some(Entry::Any(boxed)) if boxed.is::<T>());
      if !matches {
        continue;
      }
      return match self.contents.remove(&id) {
        Some(Entry::Any(boxed)) => boxed.downcast::<T>().ok().map(|boxed| *boxed),
        Some(Entry::Erased(entry)) => entry.take::<T>(),
        None => None,
      };
    }
    None
  }

  pub fn erased_insert(&mut self, entry: ErasedEntry) -> bool {
    let id = StoreId::Erased(entry.key);
    if self.contents.contains_key(&id) {
      return false;
    }
    self.contents.insert(id, Entry::Erased(entry));
    true
  }

  pub fn erased_contains(&self, key: StoreKey) -> bool {
    matches!(
      self.contents.get(&StoreId::Erased(key)),
      Some(Entry::Erased(entry)) if entry.key == key
    )
  }

  pub fn erased_get(&self, key: StoreKey) -> Option<*mut c_void> {
    match self.contents.get(&StoreId::Erased(key)) {
      Some(Entry::Erased(entry)) if entry.key == key => Some(entry.ptr),
      _ => None,
    }
  }

  pub fn erased_remove(&mut self, key: StoreKey) -> Option<ErasedEntry> {
    let id = StoreId::Erased(key);
    let matches = matches!(self.contents.get(&id), Some(Entry::Erased(entry)) if entry.key == key);
    if !matches {
      return None;
    }
    match self.contents.remove(&id) {
      Some(Entry::Erased(entry)) => Some(entry),
      _ => None,
    }
  }

  #[allow(warnings)]
  pub fn handle<T, R>(
    &mut self,
    f: impl FnOnce(&mut Store, T) -> miette::Result<R>,
  ) -> miette::Result<R>
  where
    T: HandleStateTuple,
  {
    T::run(self, f)
  }
}

macro_rules! impl_handle_tuple {
  ($($T:ident),+) => {
    impl<$($T: 'static),+> HandleStateTuple for ($(&mut $T,)+) {
      fn run<R>(
        store: &mut Store,
        f: impl FnOnce(&mut Store, Self) -> miette::Result<R>,
      ) -> miette::Result<R> {
        {
          let keys = [$(StoreKey::of::<$T>()),+];

          for i in 0..keys.len() {
            for j in (i + 1)..keys.len() {
              if keys[i] == keys[j] {
                miette::bail!("Duplicate type requested");
              }
            }
          }
        }

        $(
          #[allow(non_snake_case)]
          let mut $T = {
            let mut taken = None;
            for id in Store::lookup_ids::<$T>() {
              if let Some(entry) = store.contents.remove(&id) {
                taken = Some((id, entry));
                break;
              }
            }

            taken.ok_or_else(|| miette::miette!("Missing instance"))?
          };
        )+

        let result = {
          let tuple = (
            $(
              {
                let ptr = $T
                  .1
                  .ptr_mut::<$T>()
                  .ok_or_else(|| miette::miette!("Type mismatch"))?;

                unsafe { &mut *ptr }
              },
            )+
          );

          f(store, tuple)
        };

        $(
          store.contents.insert($T.0, $T.1);
        )+

        result
      }
    }
  };
}

impl_handle_tuple!(A, B);
impl_handle_tuple!(A, B, C);
impl_handle_tuple!(A, B, C, D);
impl_handle_tuple!(A, B, C, D, E);
impl_handle_tuple!(A, B, C, D, E, F);
