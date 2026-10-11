use std::{
  any::Any,
  collections::HashMap,
  sync::{Mutex, OnceLock},
};

use abi_stable::{
  sabi_trait::TD_Opaque,
  std_types::{RBox, ROption, RResult, RStr, RString, RVec},
};

use crate::{
  Args,
  host::{Host, Host_TO},
  payload::{PayloadBox, PayloadParser, PayloadParserBox},
  surfaces::{
    Component, Compositor, ConfigParser, ConfigParserBox, DeployableDesktopItem, DesktopItem,
    Payload, QuickSettings, Renderable, Service, Spotlight, service_factory,
  },
};

pub use kdl;

type ConfigValue = Box<dyn Any + Send + Sync>;

pub(crate) fn config_values() -> &'static Mutex<HashMap<String, ConfigValue>> {
  static VALUES: OnceLock<Mutex<HashMap<String, ConfigValue>>> = OnceLock::new();
  VALUES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn store_config_value<T: Send + Sync + 'static>(key: &str, value: T) {
  config_values()
    .lock()
    .unwrap()
    .insert(key.to_owned(), Box::new(value));
}

struct ParserFn<F> {
  parse: F,
}

impl<F> ConfigParser for ParserFn<F>
where
  F: Fn(Option<&kdl::KdlNode>) -> Result<(), String> + Send + Sync + 'static,
{
  fn parse(&self, block: ROption<RStr<'_>>) -> RResult<(), RString> {
    let node = match block {
      ROption::RSome(text) => text
        .parse::<kdl::KdlDocument>()
        .ok()
        .and_then(|document| document.nodes().first().cloned()),
      ROption::RNone => None,
    };

    match (self.parse)(node.as_ref()) {
      Ok(()) => RResult::ROk(()),
      Err(message) => RResult::RErr(RString::from(message)),
    }
  }
}

pub fn config_parser<T: Send + Sync + 'static>(
  name: &str,
  parse: impl Fn(Option<&kdl::KdlNode>) -> T + Send + Sync + 'static,
) -> ConfigParserBox {
  let key = name.to_owned();
  ConfigParserBox::from_value(
    ParserFn {
      parse: move |node: Option<&kdl::KdlNode>| {
        let value = parse(node);
        store_config_value(&key, value);
        Ok(())
      },
    },
    TD_Opaque,
  )
}

pub fn config_parser_try<T: Send + Sync + 'static>(
  name: &str,
  parse: impl Fn(Option<&kdl::KdlNode>) -> Result<T, String> + Send + Sync + 'static,
) -> ConfigParserBox {
  let key = name.to_owned();
  ConfigParserBox::from_value(
    ParserFn {
      parse: move |node: Option<&kdl::KdlNode>| {
        let value = parse(node)?;
        store_config_value(&key, value);
        Ok(())
      },
    },
    TD_Opaque,
  )
}

struct PayloadParserFn<F> {
  parse: F,
}

impl<F> PayloadParser for PayloadParserFn<F>
where
  F: Fn(&str, &Args) -> Option<PayloadBox> + Send + Sync + 'static,
{
  fn parse(&self, command: RStr<'_>, args: &Args) -> ROption<PayloadBox> {
    (self.parse)(&command, args).into()
  }
}

pub fn payload_parser<T: Clone + Send + Sync + 'static>(
  parse: impl Fn(&str, &Args) -> Option<T> + Send + Sync + 'static,
) -> PayloadParserBox {
  PayloadParserBox::from_value(
    PayloadParserFn {
      parse: move |command: &str, args: &Args| parse(command, args).map(PayloadBox::new),
    },
    TD_Opaque,
  )
}

pub trait HostExt {
  fn component<T: Component>(&self, name: &str);
  fn renderable<T: Renderable>(&self, name: &str);
  fn payload<T: Payload>(&self, command: &str);
  fn desktop_item<T: DesktopItem>(&self, name: &str);
  fn desktop_item_deployer<T: DeployableDesktopItem>(&self, name: &str);
  fn compositor<T: Compositor>(&self, name: &str);
  fn spotlight<T: Spotlight>(&self, name: &str);
  fn quick_settings<T: QuickSettings>(&self, name: &str);
  fn config_parser<T: Send + Sync + 'static>(
    &self,
    name: &str,
    parse: impl Fn(Option<&kdl::KdlNode>) -> T + Send + Sync + 'static,
  );
  fn config_parser_try<T: Send + Sync + 'static>(
    &self,
    name: &str,
    parse: impl Fn(Option<&kdl::KdlNode>) -> Result<T, String> + Send + Sync + 'static,
  );
  fn service<T: Service>(&self, name: &str);
  fn service_autostart<T: Service>(&self, name: &str);
  fn payload_parser<T: Clone + Send + Sync + 'static>(
    &self,
    commands: &[&str],
    parse: impl Fn(&str, &Args) -> Option<T> + Send + Sync + 'static,
  );
  fn command(&self, name: &str, title: &str);
  fn command_with(&self, name: &str, title: &str, description: &str);
  fn style(&self, css: &str);
}

impl HostExt for Host_TO<RBox<()>> {
  fn component<T: Component>(&self, name: &str) {
    Host::register_component(
      self,
      RString::from(name),
      crate::surfaces::component_factory::<T>(),
    );
  }

  fn renderable<T: Renderable>(&self, name: &str) {
    Host::register_renderable(
      self,
      RString::from(name),
      crate::surfaces::renderable_factory::<T>(),
    );
  }

  fn payload<T: Payload>(&self, command: &str) {
    Host::register_payload(
      self,
      RString::from(command),
      crate::surfaces::payload_factory::<T>(),
    );
  }

  fn desktop_item<T: DesktopItem>(&self, name: &str) {
    Host::register_desktop_item(
      self,
      RString::from(name),
      crate::surfaces::desktop_item_factory::<T>(),
    );
  }

  fn desktop_item_deployer<T: DeployableDesktopItem>(&self, name: &str) {
    Host::register_desktop_item_deployer(
      self,
      RString::from(name),
      crate::surfaces::deployable_desktop_item_factory::<T>(),
    );
  }

  fn compositor<T: Compositor>(&self, name: &str) {
    Host::register_compositor(
      self,
      RString::from(name),
      crate::surfaces::compositor_factory::<T>(),
    );
  }

  fn spotlight<T: Spotlight>(&self, name: &str) {
    Host::register_spotlight(
      self,
      RString::from(name),
      crate::surfaces::spotlight_factory::<T>(),
    );
  }

  fn quick_settings<T: QuickSettings>(&self, name: &str) {
    Host::register_quick_settings(
      self,
      RString::from(name),
      crate::surfaces::quick_settings_factory::<T>(),
    );
  }

  fn config_parser<T: Send + Sync + 'static>(
    &self,
    name: &str,
    parse: impl Fn(Option<&kdl::KdlNode>) -> T + Send + Sync + 'static,
  ) {
    Host::register_config_parser(self, RString::from(name), config_parser(name, parse));
  }

  fn config_parser_try<T: Send + Sync + 'static>(
    &self,
    name: &str,
    parse: impl Fn(Option<&kdl::KdlNode>) -> Result<T, String> + Send + Sync + 'static,
  ) {
    Host::register_config_parser(self, RString::from(name), config_parser_try(name, parse));
  }

  fn service<T: Service>(&self, name: &str) {
    Host::register_service(self, RString::from(name), service_factory::<T>(), false);
  }

  fn service_autostart<T: Service>(&self, name: &str) {
    Host::register_service(self, RString::from(name), service_factory::<T>(), true);
  }

  fn payload_parser<T: Clone + Send + Sync + 'static>(
    &self,
    commands: &[&str],
    parse: impl Fn(&str, &Args) -> Option<T> + Send + Sync + 'static,
  ) {
    let commands: RVec<RString> = commands
      .iter()
      .map(|command| RString::from(*command))
      .collect();
    Host::register_payload_parser(self, commands, payload_parser(parse));
  }

  fn command(&self, name: &str, title: &str) {
    Host::register_command(
      self,
      RString::from(name),
      RString::from(title),
      RString::new(),
    );
  }

  fn command_with(&self, name: &str, title: &str, description: &str) {
    Host::register_command(
      self,
      RString::from(name),
      RString::from(title),
      RString::from(description),
    );
  }

  fn style(&self, css: &str) {
    Host::register_style(self, RStr::from_str(css));
  }
}
