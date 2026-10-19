use std::marker::PhantomData;

use abi_stable::{
  StableAbi, sabi_trait,
  sabi_trait::TD_Opaque,
  std_types::{RBox, ROption, RResult, RStr, RString, RVec},
};

use crate::{
  event::{Args, Event, EventFilter, ItemEffect, Message},
  host::Context,
  payload::PayloadBox,
  types::{
    DesktopSettings, DisplayStyle, Node, QuickSettingsTile, RenderableSettings, SpotlightResult,
  },
};

pub trait Component: Send + 'static {
  fn new(ctx: &Context<'_>) -> Self;
  fn events(&self) -> RVec<EventFilter> {
    RVec::new()
  }
  fn watch(&mut self, _ctx: &Context<'_>) {}
  fn update(&mut self, _ctx: &Context<'_>, _event: &Event) -> ItemEffect {
    ItemEffect::None
  }
  fn check_view(&self, _ctx: &Context<'_>) -> bool {
    true
  }
  fn hoverable() -> bool {
    true
  }
  fn stop(&mut self, _ctx: &Context<'_>) {}
  fn view(&self, ctx: &Context<'_>) -> Node;
}

#[sabi_trait]
pub trait ComponentObject: Send + 'static {
  fn events(&self) -> RVec<EventFilter>;
  fn watch(&mut self, ctx: &Context<'_>);
  fn update(&mut self, ctx: &Context<'_>, event: &Event) -> ItemEffect;
  fn check_view(&self, ctx: &Context<'_>) -> bool;
  fn stop(&mut self, ctx: &Context<'_>);
  fn view(&self, ctx: &Context<'_>) -> Node;
}

pub type ComponentObjectBox = ComponentObject_TO<RBox<()>>;

#[sabi_trait]
pub trait ComponentFactory: Send + Sync + 'static {
  fn create(&self, ctx: &Context<'_>) -> ComponentObjectBox;
  fn hoverable(&self) -> bool;
}

pub type ComponentFactoryBox = ComponentFactory_TO<RBox<()>>;

struct ComponentAdapter<T>(T);

impl<T: Component> ComponentObject for ComponentAdapter<T> {
  fn events(&self) -> RVec<EventFilter> {
    self.0.events()
  }

  fn watch(&mut self, ctx: &Context<'_>) {
    self.0.watch(ctx);
  }

  fn update(&mut self, ctx: &Context<'_>, event: &Event) -> ItemEffect {
    self.0.update(ctx, event)
  }

  fn check_view(&self, ctx: &Context<'_>) -> bool {
    self.0.check_view(ctx)
  }

  fn stop(&mut self, ctx: &Context<'_>) {
    self.0.stop(ctx);
  }

  fn view(&self, ctx: &Context<'_>) -> Node {
    self.0.view(ctx)
  }
}

struct ComponentFactoryImpl<T>(PhantomData<fn() -> T>);

impl<T: Component> ComponentFactory for ComponentFactoryImpl<T> {
  fn create(&self, ctx: &Context<'_>) -> ComponentObjectBox {
    ComponentObjectBox::from_value(ComponentAdapter(T::new(ctx)), TD_Opaque)
  }

  fn hoverable(&self) -> bool {
    T::hoverable()
  }
}

pub fn component_factory<T: Component>() -> ComponentFactoryBox {
  ComponentFactoryBox::from_value(ComponentFactoryImpl::<T>(PhantomData), TD_Opaque)
}

pub trait Renderable: Send + 'static {
  fn new(ctx: &Context<'_>) -> Self;
  fn settings(&self) -> RenderableSettings {
    RenderableSettings::default()
  }
  fn events(&self) -> RVec<EventFilter> {
    RVec::new()
  }
  fn initialize(&mut self, _ctx: &Context<'_>) {}
  fn update(&mut self, _ctx: &Context<'_>) -> ItemEffect {
    ItemEffect::None
  }
  fn on_event(&mut self, _ctx: &Context<'_>, _event: &Event) -> ItemEffect {
    ItemEffect::None
  }
  fn handle_message(&mut self, _ctx: &Context<'_>, _message: &Message) -> ItemEffect {
    ItemEffect::None
  }
  fn view(&self, ctx: &Context<'_>) -> Node;
}

#[sabi_trait]
pub trait RenderableObject: Send + 'static {
  fn settings(&self) -> RenderableSettings;
  fn events(&self) -> RVec<EventFilter>;
  fn initialize(&mut self, ctx: &Context<'_>);
  fn update(&mut self, ctx: &Context<'_>) -> ItemEffect;
  fn on_event(&mut self, ctx: &Context<'_>, event: &Event) -> ItemEffect;
  fn handle_message(&mut self, ctx: &Context<'_>, message: &Message) -> ItemEffect;
  fn view(&self, ctx: &Context<'_>) -> Node;
}

pub type RenderableObjectBox = RenderableObject_TO<RBox<()>>;

#[sabi_trait]
pub trait RenderableFactory: Send + Sync + 'static {
  fn create(&self, ctx: &Context<'_>) -> RenderableObjectBox;
}

pub type RenderableFactoryBox = RenderableFactory_TO<RBox<()>>;

struct RenderableAdapter<T>(T);

impl<T: Renderable> RenderableObject for RenderableAdapter<T> {
  fn settings(&self) -> RenderableSettings {
    self.0.settings()
  }

  fn events(&self) -> RVec<EventFilter> {
    self.0.events()
  }

  fn initialize(&mut self, ctx: &Context<'_>) {
    self.0.initialize(ctx);
  }

  fn update(&mut self, ctx: &Context<'_>) -> ItemEffect {
    self.0.update(ctx)
  }

  fn on_event(&mut self, ctx: &Context<'_>, event: &Event) -> ItemEffect {
    self.0.on_event(ctx, event)
  }

  fn handle_message(&mut self, ctx: &Context<'_>, message: &Message) -> ItemEffect {
    self.0.handle_message(ctx, message)
  }

  fn view(&self, ctx: &Context<'_>) -> Node {
    self.0.view(ctx)
  }
}

struct RenderableFactoryImpl<T>(PhantomData<fn() -> T>);

impl<T: Renderable> RenderableFactory for RenderableFactoryImpl<T> {
  fn create(&self, ctx: &Context<'_>) -> RenderableObjectBox {
    RenderableObjectBox::from_value(RenderableAdapter(T::new(ctx)), TD_Opaque)
  }
}

pub fn renderable_factory<T: Renderable>() -> RenderableFactoryBox {
  RenderableFactoryBox::from_value(RenderableFactoryImpl::<T>(PhantomData), TD_Opaque)
}

pub trait Payload: Send + 'static {
  fn new(ctx: &Context<'_>) -> Self;
  fn invoke(&mut self, ctx: &Context<'_>, args: &Args, payload: ROption<&PayloadBox>);
}

#[sabi_trait]
pub trait PayloadObject: Send + 'static {
  fn invoke(&mut self, ctx: &Context<'_>, args: &Args, payload: ROption<&PayloadBox>);
}

pub type PayloadObjectBox = PayloadObject_TO<RBox<()>>;

#[sabi_trait]
pub trait PayloadFactory: Send + Sync + 'static {
  fn create(&self, ctx: &Context<'_>) -> PayloadObjectBox;
}

pub type PayloadFactoryBox = PayloadFactory_TO<RBox<()>>;

struct PayloadAdapter<T>(T);

impl<T: Payload> PayloadObject for PayloadAdapter<T> {
  fn invoke(&mut self, ctx: &Context<'_>, args: &Args, payload: ROption<&PayloadBox>) {
    self.0.invoke(ctx, args, payload);
  }
}

struct PayloadFactoryImpl<T>(PhantomData<fn() -> T>);

impl<T: Payload> PayloadFactory for PayloadFactoryImpl<T> {
  fn create(&self, ctx: &Context<'_>) -> PayloadObjectBox {
    PayloadObjectBox::from_value(PayloadAdapter(T::new(ctx)), TD_Opaque)
  }
}

pub fn payload_factory<T: Payload>() -> PayloadFactoryBox {
  PayloadFactoryBox::from_value(PayloadFactoryImpl::<T>(PhantomData), TD_Opaque)
}

pub trait DesktopItem: Send + 'static {
  fn new(ctx: &Context<'_>) -> Self;
  fn settings(&self) -> DesktopSettings;
  fn events(&self) -> RVec<EventFilter> {
    RVec::new()
  }
  fn initialize(&mut self, _ctx: &Context<'_>) {}
  fn update(&mut self, _ctx: &Context<'_>, _event: &Event) -> ItemEffect {
    ItemEffect::None
  }
  fn view(&self, ctx: &Context<'_>) -> Node;
  fn handle_message(&mut self, _ctx: &Context<'_>, _message: &Message) -> ItemEffect {
    ItemEffect::None
  }
}

#[sabi_trait]
pub trait DesktopItemObject: Send + 'static {
  fn settings(&self) -> DesktopSettings;
  fn events(&self) -> RVec<EventFilter>;
  fn initialize(&mut self, ctx: &Context<'_>);
  fn update(&mut self, ctx: &Context<'_>, event: &Event) -> ItemEffect;
  fn view(&self, ctx: &Context<'_>) -> Node;
  fn handle_message(&mut self, ctx: &Context<'_>, message: &Message) -> ItemEffect;
}

pub type DesktopItemObjectBox = DesktopItemObject_TO<RBox<()>>;

#[sabi_trait]
pub trait DesktopItemFactory: Send + Sync + 'static {
  fn create(&self, ctx: &Context<'_>) -> DesktopItemObjectBox;
}

pub type DesktopItemFactoryBox = DesktopItemFactory_TO<RBox<()>>;

struct DesktopItemAdapter<T>(T);

impl<T: DesktopItem> DesktopItemObject for DesktopItemAdapter<T> {
  fn settings(&self) -> DesktopSettings {
    self.0.settings()
  }

  fn events(&self) -> RVec<EventFilter> {
    self.0.events()
  }

  fn initialize(&mut self, ctx: &Context<'_>) {
    self.0.initialize(ctx);
  }

  fn update(&mut self, ctx: &Context<'_>, event: &Event) -> ItemEffect {
    self.0.update(ctx, event)
  }

  fn view(&self, ctx: &Context<'_>) -> Node {
    self.0.view(ctx)
  }

  fn handle_message(&mut self, ctx: &Context<'_>, message: &Message) -> ItemEffect {
    self.0.handle_message(ctx, message)
  }
}

struct DesktopItemFactoryImpl<T>(PhantomData<fn() -> T>);

impl<T: DesktopItem> DesktopItemFactory for DesktopItemFactoryImpl<T> {
  fn create(&self, ctx: &Context<'_>) -> DesktopItemObjectBox {
    DesktopItemObjectBox::from_value(DesktopItemAdapter(T::new(ctx)), TD_Opaque)
  }
}

pub fn desktop_item_factory<T: DesktopItem>() -> DesktopItemFactoryBox {
  DesktopItemFactoryBox::from_value(DesktopItemFactoryImpl::<T>(PhantomData), TD_Opaque)
}

#[repr(C)]
#[derive(StableAbi)]
pub struct DeployedItem {
  pub id: RString,
  pub item: DesktopItemObjectBox,
}

impl DeployedItem {
  pub fn new<T: DesktopItem>(ctx: &Context<'_>, id: impl Into<RString>) -> Self {
    Self {
      id: id.into(),
      item: desktop_item_factory::<T>().create(ctx),
    }
  }
}

#[repr(C)]
#[derive(StableAbi)]
pub enum DeployResult {
  None,
  Deploy(RVec<DeployedItem>),
  Destroy(RString),
  Many(RVec<DeployResult>),
}

pub trait DeployableDesktopItem: Send + 'static {
  fn new(ctx: &Context<'_>) -> Self;
  fn events(&self) -> RVec<EventFilter> {
    RVec::new()
  }
  fn deploy(&mut self, _ctx: &Context<'_>, _event: &Event) -> DeployResult {
    DeployResult::None
  }
}

#[sabi_trait]
pub trait DeployableDesktopItemObject: Send + 'static {
  fn events(&self) -> RVec<EventFilter>;
  fn deploy(&mut self, ctx: &Context<'_>, event: &Event) -> DeployResult;
}

pub type DeployableDesktopItemObjectBox = DeployableDesktopItemObject_TO<RBox<()>>;

#[sabi_trait]
pub trait DeployableDesktopItemFactory: Send + Sync + 'static {
  fn create(&self, ctx: &Context<'_>) -> DeployableDesktopItemObjectBox;
}

pub type DeployableDesktopItemFactoryBox = DeployableDesktopItemFactory_TO<RBox<()>>;

struct DeployableDesktopItemAdapter<T>(T);

impl<T: DeployableDesktopItem> DeployableDesktopItemObject for DeployableDesktopItemAdapter<T> {
  fn events(&self) -> RVec<EventFilter> {
    self.0.events()
  }

  fn deploy(&mut self, ctx: &Context<'_>, event: &Event) -> DeployResult {
    self.0.deploy(ctx, event)
  }
}

struct DeployableDesktopItemFactoryImpl<T>(PhantomData<fn() -> T>);

impl<T: DeployableDesktopItem> DeployableDesktopItemFactory
  for DeployableDesktopItemFactoryImpl<T>
{
  fn create(&self, ctx: &Context<'_>) -> DeployableDesktopItemObjectBox {
    DeployableDesktopItemObjectBox::from_value(DeployableDesktopItemAdapter(T::new(ctx)), TD_Opaque)
  }
}

pub fn deployable_desktop_item_factory<T: DeployableDesktopItem>() -> DeployableDesktopItemFactoryBox
{
  DeployableDesktopItemFactoryBox::from_value(
    DeployableDesktopItemFactoryImpl::<T>(PhantomData),
    TD_Opaque,
  )
}

#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub enum CompositorCommand {
  FocusWorkspace(i32),
  FocusWindow(u64),
  CloseWindow(u64),
  KillWindow(u64),
  FocusMonitor(RString),
  Custom(PayloadBox),
}

pub trait Compositor: Send + 'static {
  fn new(ctx: &Context<'_>) -> Self;
  fn is_active(&self) -> bool {
    false
  }
  fn initialize(&mut self, _ctx: &Context<'_>) {}
  fn update_state(&mut self, _ctx: &Context<'_>) {}
  fn send_command(&mut self, _ctx: &Context<'_>, _command: &CompositorCommand) {}
}

#[sabi_trait]
pub trait CompositorObject: Send + 'static {
  fn is_active(&self) -> bool;
  fn initialize(&mut self, ctx: &Context<'_>);
  fn update_state(&mut self, ctx: &Context<'_>);
  fn send_command(&mut self, ctx: &Context<'_>, command: &CompositorCommand);
}

pub type CompositorObjectBox = CompositorObject_TO<RBox<()>>;

#[sabi_trait]
pub trait CompositorFactory: Send + Sync + 'static {
  fn create(&self, ctx: &Context<'_>) -> CompositorObjectBox;
}

pub type CompositorFactoryBox = CompositorFactory_TO<RBox<()>>;

struct CompositorAdapter<T>(T);

impl<T: Compositor> CompositorObject for CompositorAdapter<T> {
  fn is_active(&self) -> bool {
    self.0.is_active()
  }

  fn initialize(&mut self, ctx: &Context<'_>) {
    self.0.initialize(ctx);
  }

  fn update_state(&mut self, ctx: &Context<'_>) {
    self.0.update_state(ctx);
  }

  fn send_command(&mut self, ctx: &Context<'_>, command: &CompositorCommand) {
    self.0.send_command(ctx, command);
  }
}

struct CompositorFactoryImpl<T>(PhantomData<fn() -> T>);

impl<T: Compositor> CompositorFactory for CompositorFactoryImpl<T> {
  fn create(&self, ctx: &Context<'_>) -> CompositorObjectBox {
    CompositorObjectBox::from_value(CompositorAdapter(T::new(ctx)), TD_Opaque)
  }
}

pub fn compositor_factory<T: Compositor>() -> CompositorFactoryBox {
  CompositorFactoryBox::from_value(CompositorFactoryImpl::<T>(PhantomData), TD_Opaque)
}

pub trait Spotlight: Send + 'static {
  fn new(ctx: &Context<'_>) -> Self;
  fn generate(&mut self, _ctx: &Context<'_>, _query: &str) -> Vec<SpotlightResult> {
    Vec::new()
  }
  fn activate(&mut self, _ctx: &Context<'_>, _index: usize) {}
  fn trigger_check(&self, _ctx: &Context<'_>, _query: &str) -> bool {
    false
  }
  fn allows_triggers() -> bool {
    false
  }
  fn display_styles() -> &'static [DisplayStyle] {
    &[DisplayStyle::List]
  }
}

#[sabi_trait]
pub trait SpotlightObject: Send + 'static {
  fn generate(&mut self, ctx: &Context<'_>, query: RStr<'_>) -> RVec<SpotlightResult>;
  fn activate(&mut self, ctx: &Context<'_>, index: usize);
  fn trigger_check(&self, ctx: &Context<'_>, query: RStr<'_>) -> bool;
  fn allows_triggers(&self) -> bool;
  fn display_styles(&self) -> u32;
}

pub type SpotlightObjectBox = SpotlightObject_TO<RBox<()>>;

#[sabi_trait]
pub trait SpotlightFactory: Send + Sync + 'static {
  fn create(&self, ctx: &Context<'_>) -> SpotlightObjectBox;
}

pub type SpotlightFactoryBox = SpotlightFactory_TO<RBox<()>>;

struct SpotlightAdapter<T>(T);

impl<T: Spotlight> SpotlightObject for SpotlightAdapter<T> {
  fn generate(&mut self, ctx: &Context<'_>, query: RStr<'_>) -> RVec<SpotlightResult> {
    self.0.generate(ctx, &query).into()
  }

  fn activate(&mut self, ctx: &Context<'_>, index: usize) {
    self.0.activate(ctx, index);
  }

  fn trigger_check(&self, ctx: &Context<'_>, query: RStr<'_>) -> bool {
    self.0.trigger_check(ctx, &query)
  }

  fn allows_triggers(&self) -> bool {
    T::allows_triggers()
  }

  fn display_styles(&self) -> u32 {
    T::display_styles()
      .iter()
      .fold(0, |bits, style| bits | style.bit())
  }
}

struct SpotlightFactoryImpl<T>(PhantomData<fn() -> T>);

impl<T: Spotlight> SpotlightFactory for SpotlightFactoryImpl<T> {
  fn create(&self, ctx: &Context<'_>) -> SpotlightObjectBox {
    SpotlightObjectBox::from_value(SpotlightAdapter(T::new(ctx)), TD_Opaque)
  }
}

pub fn spotlight_factory<T: Spotlight>() -> SpotlightFactoryBox {
  SpotlightFactoryBox::from_value(SpotlightFactoryImpl::<T>(PhantomData), TD_Opaque)
}

pub trait QuickSettings: Send + 'static {
  fn new(ctx: &Context<'_>) -> Self;
  fn tiles(&self, _ctx: &Context<'_>) -> Vec<QuickSettingsTile> {
    Vec::new()
  }
  fn children(&self, _ctx: &Context<'_>, _id: &str) -> Vec<QuickSettingsTile> {
    Vec::new()
  }
}

#[sabi_trait]
pub trait QuickSettingsObject: Send + 'static {
  fn tiles(&self, ctx: &Context<'_>) -> RVec<QuickSettingsTile>;
  fn children(&self, ctx: &Context<'_>, id: RStr<'_>) -> RVec<QuickSettingsTile>;
}

pub type QuickSettingsObjectBox = QuickSettingsObject_TO<RBox<()>>;

#[sabi_trait]
pub trait QuickSettingsFactory: Send + Sync + 'static {
  fn create(&self, ctx: &Context<'_>) -> QuickSettingsObjectBox;
}

pub type QuickSettingsFactoryBox = QuickSettingsFactory_TO<RBox<()>>;

struct QuickSettingsAdapter<T>(T);

impl<T: QuickSettings> QuickSettingsObject for QuickSettingsAdapter<T> {
  fn tiles(&self, ctx: &Context<'_>) -> RVec<QuickSettingsTile> {
    self.0.tiles(ctx).into()
  }

  fn children(&self, ctx: &Context<'_>, id: RStr<'_>) -> RVec<QuickSettingsTile> {
    self.0.children(ctx, &id).into()
  }
}

struct QuickSettingsFactoryImpl<T>(PhantomData<fn() -> T>);

impl<T: QuickSettings> QuickSettingsFactory for QuickSettingsFactoryImpl<T> {
  fn create(&self, ctx: &Context<'_>) -> QuickSettingsObjectBox {
    QuickSettingsObjectBox::from_value(QuickSettingsAdapter(T::new(ctx)), TD_Opaque)
  }
}

pub fn quick_settings_factory<T: QuickSettings>() -> QuickSettingsFactoryBox {
  QuickSettingsFactoryBox::from_value(QuickSettingsFactoryImpl::<T>(PhantomData), TD_Opaque)
}

#[sabi_trait]
pub trait ConfigParser: Send + Sync + 'static {
  fn parse(
    &self,
    block: abi_stable::std_types::ROption<RStr<'_>>,
  ) -> RResult<(), abi_stable::std_types::RString>;
}

pub type ConfigParserBox = ConfigParser_TO<RBox<()>>;

pub trait Service: Send + 'static {
  fn new(ctx: &Context<'_>) -> Self;
  fn run(&mut self, ctx: &crate::host::ServiceContext<'_>);
}

#[sabi_trait]
pub trait ServiceObject: Send + 'static {
  fn run(&mut self, ctx: &crate::host::ServiceContext<'_>);
}

pub type ServiceObjectBox = ServiceObject_TO<RBox<()>>;

#[sabi_trait]
pub trait ServiceFactory: Send + Sync + 'static {
  fn create(&self, ctx: &Context<'_>) -> ServiceObjectBox;
}

pub type ServiceFactoryBox = ServiceFactory_TO<RBox<()>>;

struct ServiceAdapter<T>(T);

impl<T: Service> ServiceObject for ServiceAdapter<T> {
  fn run(&mut self, ctx: &crate::host::ServiceContext<'_>) {
    self.0.run(ctx);
  }
}

struct ServiceFactoryImpl<T>(PhantomData<fn() -> T>);

impl<T: Service> ServiceFactory for ServiceFactoryImpl<T> {
  fn create(&self, ctx: &Context<'_>) -> ServiceObjectBox {
    ServiceObjectBox::from_value(ServiceAdapter(T::new(ctx)), TD_Opaque)
  }
}

pub fn service_factory<T: Service>() -> ServiceFactoryBox {
  ServiceFactoryBox::from_value(ServiceFactoryImpl::<T>(PhantomData), TD_Opaque)
}
