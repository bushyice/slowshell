use std::{
  any::{Any, TypeId},
  collections::HashMap,
};

use iced::Task;
use slowshell_config::{
  Config,
  style::{Style, new_default_style},
};
use slowshell_core::{Store, message::Message};
use slowshell_desktop::{DeployableDesktopItem, DesktopItem, DesktopItems};
use slowshell_widgets::Renderable;

#[repr(C)]
pub enum ResourceRegistration {
  Item(fn(&Config, &Store) -> miette::Result<Vec<Box<dyn DesktopItem + Send>>>),
  Renderable {
    name: String,
    create: fn(&Config, &Store) -> Option<Box<dyn Renderable + Send>>,
  },
  Store {
    type_id: TypeId,
    create: fn(&Config, &Store) -> miette::Result<Box<dyn Any + Send>>,
  },
  Deployable(fn(&Config, &Store) -> miette::Result<Vec<Box<dyn DeployableDesktopItem>>>),
  Custom(fn(store: &mut Store)),
  CustomConfig(fn(&Config, store: &mut Store)),
  CustomRegistry(fn(reg: &mut GlobalRegistry, store: &mut Store)),
  Unknown(Box<dyn Any + Send + Sync>),
  Style(Style),
}

impl ResourceRegistration {
  pub fn create(
    &self,
    config: &Config,
    store: &mut Store,
    items: &mut DesktopItems,
    reg: &mut GlobalRegistry,
  ) -> miette::Result<Option<Task<Message>>> {
    match self {
      ResourceRegistration::Custom(f) => {
        (f)(store);
        Ok(None)
      }
      ResourceRegistration::CustomConfig(f) => {
        (f)(config, store);
        Ok(None)
      }
      ResourceRegistration::CustomRegistry(f) => {
        (f)(reg, store);
        Ok(None)
      }
      ResourceRegistration::Item(f) => {
        let mut tasks = Vec::new();

        for item in (f)(config, store)? {
          tasks.push(items.register(&config, item));
        }

        if tasks.len() < 1 {
          return Ok(None);
        }

        Ok(Some(if tasks.len() > 1 {
          Task::batch(tasks)
        } else {
          tasks.remove(0)
        }))
      }
      ResourceRegistration::Store { type_id, create } => {
        store.insert_raw(*type_id, (create)(config, store)?);

        Ok(None)
      }
      ResourceRegistration::Deployable(f) => {
        for deployer in (f)(config, store)? {
          items.deployable(deployer);
        }
        Ok(None)
      }
      _ => Ok(None),
    }
  }

  pub fn as_unknown(self) -> Option<Box<dyn Any + Send + Sync>> {
    match self {
      // ResourceRegistration::Unknown(f) => Some((f)()),
      ResourceRegistration::Unknown(f) => Some(f),
      _ => None,
    }
  }

  pub fn as_renderable(
    self,
    config: &Config,
    store: &mut Store,
  ) -> Option<(String, Box<dyn Renderable + Send>)> {
    match self {
      ResourceRegistration::Renderable { name, create } => {
        (create)(config, store).map(|x| (name, x))
      }
      _ => None,
    }
  }
}

#[derive(Default)]
pub struct GlobalRegistry {
  items: HashMap<&'static str, Vec<ResourceRegistration>>,
}

impl GlobalRegistry {
  pub fn include_in(&mut self, group: &'static str, res: ResourceRegistration) {
    match res {
      ResourceRegistration::Style(style) => {
        new_default_style(group.into(), style);
      }
      _ => self.items.entry(group).or_default().push(res),
    }
  }

  pub fn inside(&mut self, group: &'static str) -> Vec<ResourceRegistration> {
    self.items.remove(group).unwrap_or_default()
  }
}

#[macro_export]
macro_rules! register_resources {
  (
    $(
      $group:ident:
      $kind:ident $body:tt
    ),*
    $(,)?
  ) => {
    #[doc(hidden)]
    pub fn include(
      registry: &mut ::slowshell_registry::GlobalRegistry,
    ) {
      $(
        registry.include_in(
          stringify!($group),
          ::slowshell_registry::ResourceRegistration::$kind $body,
        );
      )*
    }
  };
}

#[doc(hidden)]
pub fn include(registry: &mut GlobalRegistry) {
  let _ = registry;
  // registry.include_in(
  //   "app",
  //   ResourceRegistration::CustomRegistry(|reg, store| {
  //     let mut preg = PayloadBuilderRegistry::new();

  //     for payload in reg.inside("payload") {
  //       if let ResourceRegistration::Unknown(payload) = payload {
  //         if let Ok(builder) = payload.downcast::<PayloadBuilder>() {
  //           preg.register(*builder);
  //         }
  //       }
  //     }

  //     store.insert(preg);
  //   }),
  // );
}
