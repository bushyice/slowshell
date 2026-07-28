use std::{
  any::{Any, TypeId},
  collections::HashMap,
};

use crate::types::Ustr;

pub mod listeners;
pub mod message;
pub mod types;

#[derive(Debug, PartialEq, Clone)]
pub struct Monitor {
  pub name: Ustr,
  pub active_workspace: u32,
}

#[derive(Debug, Clone)]
pub struct Window {
  pub title: String,
  pub class: String,
  // TODO: Make metadata into a proper map
  pub metadata: HashMap<String, String>,
}

#[doc(hidden)]
trait HandleStateTuple: Sized {
  fn run<R>(
    state: &mut Store,
    f: impl FnOnce(&mut Store, Self) -> anyhow::Result<R>,
  ) -> anyhow::Result<R>;
}

pub struct Store {
  contents: HashMap<TypeId, Box<dyn Any>>,
}

impl Store {
  pub fn new() -> Self {
    Self {
      contents: Default::default(),
    }
  }

  pub fn insert<T: 'static>(&mut self, val: T) -> bool {
    let type_id = TypeId::of::<T>();

    if self.contents.contains_key(&type_id) {
      return false;
    }

    self.contents.insert(type_id, Box::new(val));
    true
  }

  pub fn borrow<T: 'static>(&self) -> Option<&T> {
    self
      .contents
      .get(&TypeId::of::<T>())
      .and_then(|x| x.downcast_ref::<T>())
  }

  pub fn borrow_mut<T: 'static>(&mut self) -> Option<&mut T> {
    self
      .contents
      .get_mut(&TypeId::of::<T>())
      .and_then(|x| x.downcast_mut::<T>())
  }

  pub fn remove<T: 'static>(&mut self) -> Option<T> {
    self
      .contents
      .remove(&TypeId::of::<T>())
      .and_then(|x| x.downcast::<T>().ok())
      .map(|x| *x)
  }

  #[allow(warnings)]
  pub fn handle<T, R>(
    &mut self,
    f: impl FnOnce(&mut Store, T) -> anyhow::Result<R>,
  ) -> anyhow::Result<R>
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
        f: impl FnOnce(&mut Store, Self) -> anyhow::Result<R>,
      ) -> anyhow::Result<R> {
        {
          let keys = [$(TypeId::of::<$T>()),+];

          for i in 0..keys.len() {
            for j in (i + 1)..keys.len() {
              if keys[i] == keys[j] {
                anyhow::bail!("Duplicate type requested");
              }
            }
          }
        }

        $(
          #[allow(non_snake_case)]
          let mut $T = {
            let key = TypeId::of::<$T>();

            let val = store
              .contents
              .remove(&key)
              .ok_or_else(|| anyhow::anyhow!("Missing instance"))?;

            (key, val)
          };
        )+

        let result = {
          let tuple = (
            $(
              {
                let downcasted = $T
                  .1
                  .downcast_mut::<$T>()
                  .ok_or_else(|| anyhow::anyhow!("Type mismatch"))?;

                unsafe { &mut *(downcasted as *mut $T) }
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
