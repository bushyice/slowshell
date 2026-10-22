// TODO: Add comments for api-ref

#![allow(non_local_definitions)]
#![allow(non_camel_case_types)]
#![allow(improper_ctypes_definitions)]

pub use abi_stable::StableAbi;
pub use abi_stable::export_root_module;
pub use abi_stable::prefix_type::PrefixTypeTrait;
pub use abi_stable::std_types::{RBox, ROption, RResult, RSlice, RStr, RString, RVec};

pub mod event;
pub mod host;
pub mod payload;
pub mod root;
pub mod sdk;
pub mod store;
pub mod surfaces;
pub mod types;

pub use event::*;
pub use host::*;
pub use payload::*;
pub use root::*;
pub use sdk::*;
pub use store::*;
pub use surfaces::*;
pub use types::*;

pub mod prelude {
  pub use crate::event::{
    Args, CompositorEvent, Event, EventFilter, ItemEffect, Message, WidgetEvent, WidgetEventKind,
  };
  pub use crate::export_plugin;
  pub use crate::host::{Context, Host, HostRef, Options, ServiceContext, StyleRef};
  pub use crate::payload::{PayloadBox, PayloadParser, PayloadParserBox};
  pub use crate::root::{PluginDependency, PluginMod, PluginMod_Ref};
  pub use crate::sdk::HostExt;
  pub use crate::store::{Store, StoreBorrow};
  pub use crate::surfaces::{
    Component, Compositor, DeployResult, DeployableDesktopItem, DeployedItem, DesktopItem, Payload,
    QuickSettings, Renderable, Service, Spotlight,
  };
  pub use crate::types::*;
  pub use abi_stable::std_types::{RBox, ROption, RResult, RSlice, RStr, RString, RVec};
}

pub mod reexport {
  pub use abi_stable;
  pub use iced;
  pub use iced_anim;
  pub use icss;
  pub use kdl;
  pub use ron;
  pub use serde;
  pub use slowshell_core as core;
}
