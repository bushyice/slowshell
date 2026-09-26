use abi_stable::{
  StableAbi,
  library::RootModule,
  std_types::{RSlice, RStr},
};

use crate::host::HostRef;

#[repr(C)]
#[derive(StableAbi, Clone, Copy, Debug)]
pub struct PluginDependency {
  pub id: RStr<'static>,
  pub version_req: RStr<'static>,
}

fn no_requirements() -> RSlice<'static, PluginDependency> {
  RSlice::EMPTY
}

#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix(prefix_ref = PluginMod_Ref, prefix_fields = PluginMod_Prefix)))]
#[sabi(missing_field(panic))]
pub struct PluginMod {
  pub id: RStr<'static>,
  pub version: RStr<'static>,

  #[sabi(last_prefix_field)]
  pub register: extern "C" fn(HostRef<'_>),

  #[sabi(missing_field(with = no_requirements))]
  pub requires: RSlice<'static, PluginDependency>,
}

impl RootModule for PluginMod_Ref {
  abi_stable::declare_root_module_statics! { PluginMod_Ref }
  const BASE_NAME: &'static str = "slowshell_plugin";
  const NAME: &'static str = "slowshell_plugin";
  const VERSION_STRINGS: abi_stable::sabi_types::VersionStrings =
    abi_stable::package_version_strings!();
}

/// Exports the plugin stuff.
///
///
/// ```ignore
/// use slowshell_plugin_abi::prelude::*;
///
/// struct Hello;
/// impl Component for Hello {
///   fn new(_ctx: &Context<'_>) -> Self { Hello }
///   fn view(&self, _ctx: &Context<'_>) -> Node { Node::text("hello") }
/// }
///
/// fn register(host: HostRef<'_>) {
///   host.component::<Hello>("example/hello");
/// }
///
/// export_plugin!(register);
/// ```
///
/// ```ignore
/// export_plugin!(register, requires = [("other-plugin", "^0.2")]);
/// ```
#[macro_export]
macro_rules! export_plugin {
  ($register:path) => {
    $crate::export_plugin!($register, requires = []);
  };
  ($register:path, requires = [$(($id:literal, $req:literal)),* $(,)?]) => {
    #[doc(hidden)]
    extern "C" fn __slowshell_plugin_register(host: $crate::HostRef<'_>) {
      $register(host);
    }

    #[doc(hidden)]
    const __SLOWSHELL_REQUIRES: &[$crate::PluginDependency] = &[
      $($crate::PluginDependency {
        id: $crate::RStr::from_str($id),
        version_req: $crate::RStr::from_str($req),
      }),*
    ];

    #[doc(hidden)]
    #[$crate::export_root_module]
    pub fn __slowshell_plugin_root_module() -> $crate::PluginMod_Ref {
      $crate::PrefixTypeTrait::leak_into_prefix($crate::PluginMod {
        id: $crate::RStr::from_str(env!("CARGO_PKG_NAME")),
        version: $crate::RStr::from_str(env!("CARGO_PKG_VERSION")),
        register: __slowshell_plugin_register,
        requires: $crate::RSlice::from_slice(__SLOWSHELL_REQUIRES),
      })
    }
  };
}
