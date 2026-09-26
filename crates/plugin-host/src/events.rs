use slowshell_core::{
  listeners::{CompositorEvent as CoreCompositorEvent, IpcCommand, ListenerAction},
  message::{EventFilter as CoreEventFilter, ItemEffect as CoreEffect, ItemMessage as CoreMessage},
  types::{PayloadBox as CorePayloadBox, Ustr},
};
use slowshell_plugin_abi::{
  Args, CompositorEvent as AbiCompositorEvent, Event as AbiEvent, EventFilter as AbiEventFilter,
  ItemEffect as AbiEffect, Message as AbiMessage, PayloadBox, RString,
  WidgetEvent as AbiWidgetEvent, WidgetEventKind,
};

#[derive(Clone, Debug)]
pub(crate) struct WidgetEvent {
  pub id: String,
  pub kind: u32,
  pub value: String,
  pub scroll: [f32; 2],
}

impl WidgetEvent {
  pub fn input(id: String, value: String) -> Self {
    Self {
      id,
      kind: WidgetEventKind::Input as u32,
      value,
      scroll: [0.0, 0.0],
    }
  }

  pub fn submit(id: String, value: String) -> Self {
    Self {
      id,
      kind: WidgetEventKind::Submit as u32,
      value,
      scroll: [0.0, 0.0],
    }
  }

  pub fn custom(id: String, value: String) -> Self {
    Self {
      id,
      kind: WidgetEventKind::Custom as u32,
      value,
      scroll: [0.0, 0.0],
    }
  }

  pub fn scroll(id: String, scroll: [f32; 2]) -> Self {
    Self {
      id,
      kind: WidgetEventKind::Scroll as u32,
      value: String::new(),
      scroll,
    }
  }

  pub fn to_abi(&self) -> AbiWidgetEvent {
    AbiWidgetEvent {
      id: RString::from(self.id.as_str()),
      kind: match self.kind {
        1 => WidgetEventKind::Input,
        2 => WidgetEventKind::Submit,
        3 => WidgetEventKind::Custom,
        4 => WidgetEventKind::Scroll,
        _ => WidgetEventKind::Custom,
      },
      value: RString::from(self.value.as_str()),
      scroll: self.scroll,
    }
  }
}

pub(crate) fn filters_to_core(
  filters: &[AbiEventFilter],
  surface_name: &Ustr,
) -> Vec<CoreEventFilter> {
  filters
    .iter()
    .map(|filter| match filter {
      AbiEventFilter::Tick | AbiEventFilter::Fd => CoreEventFilter::Named(surface_name.clone()),
      AbiEventFilter::Widget => CoreEventFilter::Payload(surface_name.clone()),
      AbiEventFilter::ConfigReload => CoreEventFilter::Named("config.reload".into()),
      AbiEventFilter::CompositorUpdate => CoreEventFilter::UpdateCompositor,
      AbiEventFilter::Compositor(events) => {
        CoreEventFilter::Compositor(CoreCompositorEvent::from_bits(events.bits()))
      }
      AbiEventFilter::Frame => CoreEventFilter::Frame,
      AbiEventFilter::Named(name) => CoreEventFilter::Named(name.as_str().into()),
      AbiEventFilter::Payload(name) => CoreEventFilter::Payload(name.as_str().into()),
      AbiEventFilter::Ipc(name) => CoreEventFilter::Ipc(IpcCommand {
        name: name.as_str().into(),
        command: name.as_str().into(),
        args: Vec::new(),
      }),
      AbiEventFilter::StartUp => CoreEventFilter::StartUp,
      AbiEventFilter::All => CoreEventFilter::All,
    })
    .collect()
}

pub(crate) fn widget_from_payload(payload: &Option<CorePayloadBox>) -> Option<AbiWidgetEvent> {
  let payload = payload.as_ref()?;
  if let Some(widget) = payload.as_this::<WidgetEvent>() {
    return Some(widget.to_abi());
  }
  payload
    .as_this::<PayloadBox>()
    .and_then(|payload| payload.as_this::<WidgetEvent>())
    .map(WidgetEvent::to_abi)
}

pub(crate) fn payload_box(payload: &Option<CorePayloadBox>) -> PayloadBox {
  payload
    .as_ref()
    .and_then(|payload| payload.as_this::<PayloadBox>())
    .cloned()
    .unwrap_or_else(|| PayloadBox::new(Args::default()))
}

fn named_event(name: &Ustr) -> AbiEvent {
  if &**name == "config.reload" {
    AbiEvent::ConfigReload
  } else {
    AbiEvent::Named(RString::from(&**name))
  }
}

pub(crate) fn payload_event(name: &Ustr, payload: &Option<CorePayloadBox>) -> AbiEvent {
  match payload {
    Some(payload) => AbiEvent::Payload {
      name: RString::from(&**name),
      payload: payload_box(&Some(payload.clone())),
    },
    None => named_event(name),
  }
}

pub(crate) fn event_from_action(event: &ListenerAction) -> AbiEvent {
  match event {
    ListenerAction::UpdateCompositor(events) => {
      AbiEvent::CompositorUpdate(AbiCompositorEvent::from_bits(events.bits()))
    }
    ListenerAction::Frame => AbiEvent::Frame,
    ListenerAction::StartUp => AbiEvent::StartUp,
    ListenerAction::Named(name) => named_event(name),
    ListenerAction::Signal { name, .. } | ListenerAction::Timer { name, .. } => named_event(name),
    ListenerAction::Payload { name, payload } => payload_event(name, payload),
    ListenerAction::Ipc(command) => AbiEvent::Ipc {
      name: RString::from(&*command.name),
      command: RString::from(&*command.command),
      args: command
        .args
        .iter()
        .map(|arg| RString::from(&**arg))
        .collect(),
    },
    _ => AbiEvent::Other(0),
  }
}

pub(crate) fn effect_from_abi(effect: AbiEffect) -> CoreEffect {
  match effect {
    AbiEffect::None => CoreEffect::None,
    AbiEffect::Redraw => CoreEffect::Redraw,
    AbiEffect::Subscribe => CoreEffect::None,
    AbiEffect::Hide => CoreEffect::Hide,
    AbiEffect::Show => CoreEffect::Show,
    AbiEffect::ReallyHide => CoreEffect::ReallyHide,
    AbiEffect::Destroy => CoreEffect::Destroy,
    AbiEffect::ReallyDestroy => CoreEffect::ReallyDestroy,
    AbiEffect::Custom(custom) => CoreEffect::Custom(custom),
  }
}

pub(crate) fn effect_to_abi(effect: &CoreEffect) -> AbiEffect {
  match effect {
    CoreEffect::None => AbiEffect::None,
    CoreEffect::Redraw => AbiEffect::Redraw,
    CoreEffect::Subscribe(_) | CoreEffect::ShowAndSubscribe(_) => AbiEffect::Subscribe,
    CoreEffect::Hide => AbiEffect::Hide,
    CoreEffect::Show => AbiEffect::Show,
    CoreEffect::ReallyHide => AbiEffect::ReallyHide,
    CoreEffect::Destroy => AbiEffect::Destroy,
    CoreEffect::ReallyDestroy => AbiEffect::ReallyDestroy,
    CoreEffect::Custom(custom) => AbiEffect::Custom(*custom),
    CoreEffect::UpdateWindow(_) => AbiEffect::None,
  }
}

#[allow(dead_code)]
pub(crate) fn message_from_abi(message: &AbiMessage) -> CoreMessage {
  match message {
    AbiMessage::Noop => CoreMessage::Noop,
    AbiMessage::Effect(effect) => CoreMessage::Effect(
      iced_layershell::reexport::IcedId::unique(),
      effect_from_abi(*effect),
    ),
    AbiMessage::Action(action) => CoreMessage::Action(ListenerAction::Payload {
      name: action.as_str().into(),
      payload: None,
    }),
    AbiMessage::EffectAction(effect, action) => CoreMessage::EffectAction(
      iced_layershell::reexport::IcedId::unique(),
      effect_from_abi(*effect),
      ListenerAction::Payload {
        name: action.as_str().into(),
        payload: None,
      },
    ),
  }
}

pub(crate) fn message_to_abi(message: &CoreMessage) -> AbiMessage {
  match message {
    CoreMessage::Effect(_, effect) => AbiMessage::Effect(effect_to_abi(effect)),
    CoreMessage::Action(action) => AbiMessage::Action(action_name(action)),
    CoreMessage::EffectAction(_, effect, action) => {
      AbiMessage::EffectAction(effect_to_abi(effect), action_name(action))
    }
    _ => AbiMessage::Noop,
  }
}

pub(crate) fn action_name(action: &ListenerAction) -> RString {
  match action {
    ListenerAction::Payload { name, .. } | ListenerAction::Named(name) => RString::from(&**name),
    _ => RString::new(),
  }
}
