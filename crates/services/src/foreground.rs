use std::collections::HashMap;
#[allow(unused_imports)]
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

#[allow(unused_imports)]
use slowshell_core::{
  Store,
  listeners::ListenerAction,
  persistence,
  types::{OptionalPayloadBox, Ustr},
};

#[cfg(feature = "audio")]
use crate::audio::PRIMARY_PLAYER_FILE;
#[cfg(feature = "audio")]
use slowshell_commons::audio::{AudioCmd, MprisSelection};
#[cfg(feature = "bluetooth")]
use slowshell_commons::bluetooth::BluetoothCmd;
#[cfg(feature = "network")]
use slowshell_commons::network::{NetCmd, NetworkPayload};
#[cfg(feature = "notifications")]
use slowshell_commons::notifications::{NotificationCmd, NotificationPayload};
#[cfg(feature = "power")]
use slowshell_commons::power::{PowerCmd, PowerProfile};
#[cfg(feature = "tray")]
use slowshell_commons::tray::{TrayCmd, TrayPayload};

pub trait ForegroundService: Send + Sync {
  fn update(&mut self, store: &mut Store, action: &ListenerAction) -> bool;
}

#[cfg(feature = "audio")]
pub struct AudioForegroundService;

#[cfg(feature = "audio")]
impl ForegroundService for AudioForegroundService {
  fn update(&mut self, store: &mut Store, action: &ListenerAction) -> bool {
    match action {
      ListenerAction::Payload { name, payload } if &**name == "audio.step" => {
        let shared = crate::audio::ensure(store);
        if let Some(delta) = payload.transform::<f32>() {
          let _ = shared.cmd_tx.send(AudioCmd::StepVolume(*delta));
        }
        true
      }
      ListenerAction::Payload { name, .. } if &**name == "audio.toggle_mute" => {
        let shared = crate::audio::ensure(store);
        let _ = shared.cmd_tx.send(AudioCmd::ToggleMute);
        true
      }
      ListenerAction::Payload { name, payload } if &**name == "audio.set_volume" => {
        let shared = crate::audio::ensure(store);
        if let Some(vol) = payload.transform::<f32>() {
          let _ = shared.cmd_tx.send(AudioCmd::SetVolume(*vol));
        }
        true
      }
      ListenerAction::Payload { name, payload } if &**name == "audio.set_sink" => {
        let shared = crate::audio::ensure(store);
        if let Some(id) = payload.transform::<u32>() {
          let _ = shared.cmd_tx.send(AudioCmd::SetDefaultSink(*id));
        }
        true
      }
      ListenerAction::Payload { name, .. } if &**name == "audio.play_pause" => {
        let shared = crate::audio::ensure(store);
        let _ = shared.cmd_tx.send(AudioCmd::PlayPause);
        true
      }
      ListenerAction::Payload { name, .. } if &**name == "audio.next" => {
        let shared = crate::audio::ensure(store);
        let _ = shared.cmd_tx.send(AudioCmd::Next);
        true
      }
      ListenerAction::Payload { name, .. } if &**name == "audio.previous" => {
        let shared = crate::audio::ensure(store);
        let _ = shared.cmd_tx.send(AudioCmd::Previous);
        true
      }
      ListenerAction::Payload { name, payload } if &**name == "audio.select_player" => {
        let shared = crate::audio::ensure(store);
        if let Some(selection) = payload.transform::<MprisSelection>() {
          let selection = selection.clone();
          let _ = shared
            .cmd_tx
            .send(AudioCmd::SelectPlayer(selection.clone()));
          if let Err(e) = persistence::save(
            PRIMARY_PLAYER_FILE,
            &(selection.bus_name, selection.identity),
          ) {
            eprintln!("[audio] failed to persist the primary player: {e}");
          }
        }
        true
      }
      _ => false,
    }
  }
}

#[cfg(feature = "power")]
pub struct PowerForegroundService;

#[cfg(feature = "power")]
impl ForegroundService for PowerForegroundService {
  fn update(&mut self, store: &mut Store, action: &ListenerAction) -> bool {
    match action {
      ListenerAction::Payload { name, payload } if &**name == "power.set_profile" => {
        let shared = crate::power::ensure(store);
        if let Some(profile) = payload.transform::<PowerProfile>() {
          let _ = shared.cmd_tx.send(PowerCmd::SetProfile(profile.clone()));
        }
        true
      }
      ListenerAction::Payload { name, payload } if &**name == "power.set_brightness" => {
        let shared = crate::power::ensure(store);
        if let Some(pct) = payload.transform::<u8>() {
          let _ = shared.cmd_tx.send(PowerCmd::SetBrightness(*pct));
        }
        true
      }
      ListenerAction::Payload { name, payload } if &**name == "power.step_brightness" => {
        let shared = crate::power::ensure(store);
        if let Some(delta) = payload.transform::<i32>() {
          let _ = shared.cmd_tx.send(PowerCmd::StepBrightness(*delta));
        }
        true
      }
      _ => false,
    }
  }
}

#[cfg(feature = "bluetooth")]
pub struct BluetoothForegroundService;

#[cfg(feature = "bluetooth")]
impl ForegroundService for BluetoothForegroundService {
  fn update(&mut self, store: &mut Store, action: &ListenerAction) -> bool {
    match action {
      ListenerAction::Payload { name, .. } if &**name == "bluetooth.toggle" => {
        let shared = crate::bluetooth::ensure(store);
        let _ = shared.cmd_tx.send(BluetoothCmd::TogglePower);
        true
      }
      ListenerAction::Payload { name, .. } if &**name == "bluetooth.scan" => {
        let shared = crate::bluetooth::ensure(store);
        let discovering = shared.discovering.load(Ordering::Relaxed);
        if discovering {
          let _ = shared.cmd_tx.send(BluetoothCmd::StopDiscovery);
        } else {
          let _ = shared.cmd_tx.send(BluetoothCmd::StartDiscovery);
        }
        true
      }
      ListenerAction::Payload { name, payload } if &**name == "bluetooth.connect" => {
        let shared = crate::bluetooth::ensure(store);
        if let Some(addr) = payload
          .transform::<Ustr>()
          .map(|s| s.to_string())
          .or_else(|| payload.transform::<String>().cloned())
        {
          let _ = shared.cmd_tx.send(BluetoothCmd::Connect(addr));
        }
        true
      }
      ListenerAction::Payload { name, payload } if &**name == "bluetooth.disconnect" => {
        let shared = crate::bluetooth::ensure(store);
        if let Some(addr) = payload
          .transform::<Ustr>()
          .map(|s| s.to_string())
          .or_else(|| payload.transform::<String>().cloned())
        {
          let _ = shared.cmd_tx.send(BluetoothCmd::Disconnect(addr));
        }
        true
      }
      ListenerAction::Payload { name, payload } if &**name == "bluetooth.pair" => {
        let shared = crate::bluetooth::ensure(store);
        if let Some(addr) = payload
          .transform::<Ustr>()
          .map(|s| s.to_string())
          .or_else(|| payload.transform::<String>().cloned())
        {
          let _ = shared.cmd_tx.send(BluetoothCmd::Pair(addr));
        }
        true
      }
      ListenerAction::Payload { name, payload } if &**name == "bluetooth.remove" => {
        let shared = crate::bluetooth::ensure(store);
        if let Some(addr) = payload
          .transform::<Ustr>()
          .map(|s| s.to_string())
          .or_else(|| payload.transform::<String>().cloned())
        {
          let _ = shared.cmd_tx.send(BluetoothCmd::Remove(addr));
        }
        let mut menu = shared.menu.lock().unwrap();
        menu.confirming_remove = None;
        true
      }
      ListenerAction::Payload { name, payload } if &**name == "bluetooth.confirm_remove" => {
        let shared = crate::bluetooth::ensure(store);
        let mut menu = shared.menu.lock().unwrap();
        if let Some(addr) = payload
          .transform::<Ustr>()
          .map(|s| s.to_string())
          .or_else(|| payload.transform::<String>().cloned())
        {
          menu.confirming_remove = if menu.confirming_remove.as_deref() == Some(&addr) {
            None
          } else {
            Some(addr)
          };
        }
        true
      }
      ListenerAction::Payload { name, payload } if &**name == "bluetooth.trust" => {
        let shared = crate::bluetooth::ensure(store);
        if let Some((addr, trust)) = payload.transform::<(Ustr, bool)>() {
          let _ = shared
            .cmd_tx
            .send(BluetoothCmd::Trust(addr.to_string(), *trust));
        }
        true
      }
      _ => false,
    }
  }
}

#[cfg(feature = "network")]
pub struct NetworkForegroundService;

#[cfg(feature = "network")]
impl ForegroundService for NetworkForegroundService {
  fn update(&mut self, store: &mut Store, action: &ListenerAction) -> bool {
    match action {
      ListenerAction::Payload { name, payload } if name.as_ref() == "network.connect" => {
        let shared = crate::network::ensure(store);
        if let Some(payload) = payload.transform::<NetworkPayload>() {
          let _ = shared.cmd_tx.send(NetCmd::Connect {
            ssid: payload.ssid.clone(),
            password: payload.password.clone(),
          });
        }
        true
      }
      ListenerAction::Payload { name, .. } if name.as_ref() == "network.disconnect" => {
        let shared = crate::network::ensure(store);
        let _ = shared.cmd_tx.send(NetCmd::DisconnectWifi);
        let mut menu = shared.menu.lock().unwrap();
        menu.selected_ssid = None;
        menu.password.clear();
        menu.info_ssid = None;
        true
      }
      ListenerAction::Payload { name, .. } if name.as_ref() == "network.wired" => {
        let shared = crate::network::ensure(store);
        let _ = shared.cmd_tx.send(NetCmd::WiredConnect);
        let mut menu = shared.menu.lock().unwrap();
        menu.selected_ssid = None;
        menu.password.clear();
        menu.info_ssid = None;
        true
      }
      ListenerAction::Payload { name, .. } if name.as_ref() == "network.toggle" => {
        let shared = crate::network::ensure(store);
        let _ = shared.cmd_tx.send(NetCmd::ToggleWifi);
        true
      }
      ListenerAction::Payload { name, .. } if name.as_ref() == "network.scan" => {
        let shared = crate::network::ensure(store);
        let _ = shared.cmd_tx.send(NetCmd::Scan);
        true
      }
      ListenerAction::Payload { name, payload } if name.as_ref() == "network.info" => {
        let shared = crate::network::ensure(store);
        if let Some(payload) = payload.transform::<NetworkPayload>() {
          let mut menu = shared.menu.lock().unwrap();
          menu.info_ssid = Some(payload.ssid.clone());
          menu.selected_ssid = None;
          menu.password.clear();
        }
        true
      }
      ListenerAction::Payload { name, .. } if name.as_ref() == "network.info.back" => {
        let shared = crate::network::ensure(store);
        shared.menu.lock().unwrap().info_ssid = None;
        true
      }
      ListenerAction::Payload { name, .. } if name.as_ref() == "network.edit" => {
        match std::process::Command::new("nm-connection-editor").spawn() {
          Ok(_) => {}
          Err(e) => eprintln!("[network] failed to launch nm-connection-editor: {e}"),
        }
        true
      }
      ListenerAction::Payload { name, payload } if name.as_ref() == "network.select" => {
        let shared = crate::network::ensure(store);
        if let Some(payload) = payload.transform::<NetworkPayload>() {
          let mut menu = shared.menu.lock().unwrap();
          menu.selected_ssid = Some(payload.ssid.clone());
          menu.password.clear();
        }
        true
      }
      ListenerAction::Payload { name, payload } if name.as_ref() == "network.password" => {
        let shared = crate::network::ensure(store);
        if let Some(payload) = payload
          .transform::<NetworkPayload>()
          .and_then(|x| x.value.clone())
        {
          shared.menu.lock().unwrap().password = payload;
        }
        true
      }
      _ => false,
    }
  }
}

#[cfg(feature = "notifications")]
pub struct NotificationsForegroundService;

#[cfg(feature = "notifications")]
impl ForegroundService for NotificationsForegroundService {
  fn update(&mut self, store: &mut Store, action: &ListenerAction) -> bool {
    match action {
      ListenerAction::Payload { name, .. } if &**name == "notifications.toggle_dnd" => {
        let shared = crate::notifications::ensure(store);
        let _ = shared.cmd_tx.send(NotificationCmd::ToggleDnd);
        true
      }
      ListenerAction::Payload { name, .. } if &**name == "notifications.clear_all" => {
        let shared = crate::notifications::ensure(store);
        let _ = shared.cmd_tx.send(NotificationCmd::ClearAll);
        true
      }
      ListenerAction::Payload { name, payload } if &**name == "notifications.dismiss" => {
        let shared = crate::notifications::ensure(store);
        if let Some(payload) = payload.transform::<NotificationPayload>() {
          let _ = shared
            .cmd_tx
            .send(NotificationCmd::DismissHistory(payload.id));
          let mut ui = shared.ui.lock().unwrap();
          std::sync::Arc::make_mut(&mut ui)
            .replies
            .remove(&payload.id);
        }
        true
      }
      ListenerAction::Payload { name, payload } if &**name == "notifications.invoke" => {
        let shared = crate::notifications::ensure(store);
        if let Some((id, key)) = payload
          .transform::<NotificationPayload>()
          .and_then(|p| Some((p.id, p.key.clone()?)))
        {
          let _ = shared.cmd_tx.send(NotificationCmd::InvokeAction(id, key));
          let mut ui = shared.ui.lock().unwrap();
          std::sync::Arc::make_mut(&mut ui).replies.remove(&id);
        }
        true
      }
      ListenerAction::Payload { name, payload } if &**name == "notifications.reply" => {
        let shared = crate::notifications::ensure(store);
        if let Some((id, key, text)) = payload
          .transform::<NotificationPayload>()
          .and_then(|p| Some((p.id, p.key.clone()?, p.text.clone()?)))
        {
          let _ = shared
            .cmd_tx
            .send(NotificationCmd::InvokeActionWithText(id, key, text));
          let mut ui = shared.ui.lock().unwrap();
          std::sync::Arc::make_mut(&mut ui).replies.remove(&id);
        }
        true
      }
      ListenerAction::Payload { name, payload } if &**name == "notifications.set_reply" => {
        let shared = crate::notifications::ensure(store);
        if let Some((id, text)) = payload
          .transform::<NotificationPayload>()
          .and_then(|p| Some((p.id, p.text.clone()?)))
        {
          let mut ui = shared.ui.lock().unwrap();
          std::sync::Arc::make_mut(&mut ui).replies.insert(id, text);
        }
        true
      }
      _ => false,
    }
  }
}

#[cfg(feature = "tray")]
pub struct TrayForegroundService;

#[cfg(feature = "tray")]
impl ForegroundService for TrayForegroundService {
  fn update(&mut self, store: &mut Store, action: &ListenerAction) -> bool {
    match action {
      ListenerAction::Payload { name, payload } if name.as_ref() == "tray.activate" => {
        let _ = crate::tray::ensure(store);
        if let Some(payload) = payload.transform::<TrayPayload>() {
          if let Some(menu_path) = payload.menu_path.clone() {
            let _ = crate::tray::shared().cmd_tx.send(TrayCmd::Activate {
              address: payload.address.to_string(),
              menu_path: menu_path.to_string(),
              submenu_id: payload.submenu_id,
            });
          }
        }
        true
      }
      _ => false,
    }
  }
}

pub type ForegroundServiceFactory = Box<dyn Fn() -> Box<dyn ForegroundService> + Send + Sync>;

pub struct ForegroundServices {
  available: HashMap<String, ForegroundServiceFactory>,
  active: HashMap<String, Box<dyn ForegroundService>>,
}

impl Default for ForegroundServices {
  fn default() -> Self {
    Self::new()
  }
}

impl ForegroundServices {
  pub fn new() -> Self {
    #[allow(unused_mut)]
    let mut available: HashMap<String, ForegroundServiceFactory> = HashMap::new();

    #[cfg(feature = "audio")]
    {
      available.insert(
        "audio".into(),
        Box::new(|| Box::new(AudioForegroundService)),
      );
      available.insert(
        "sound".into(),
        Box::new(|| Box::new(AudioForegroundService)),
      );
    }

    #[cfg(feature = "power")]
    {
      available.insert(
        "power".into(),
        Box::new(|| Box::new(PowerForegroundService)),
      );
      available.insert(
        "brightness".into(),
        Box::new(|| Box::new(PowerForegroundService)),
      );
    }

    #[cfg(feature = "bluetooth")]
    available.insert(
      "bluetooth".into(),
      Box::new(|| Box::new(BluetoothForegroundService)),
    );

    #[cfg(feature = "network")]
    available.insert(
      "network".into(),
      Box::new(|| Box::new(NetworkForegroundService)),
    );

    #[cfg(feature = "notifications")]
    available.insert(
      "notifications".into(),
      Box::new(|| Box::new(NotificationsForegroundService)),
    );

    #[cfg(feature = "tray")]
    available.insert("tray".into(), Box::new(|| Box::new(TrayForegroundService)));

    Self {
      available,
      active: HashMap::new(),
    }
  }

  pub fn register_factory(&mut self, name: impl Into<String>, factory: ForegroundServiceFactory) {
    self.available.insert(name.into(), factory);
  }

  pub fn register(&mut self, name: impl Into<String>, service: Box<dyn ForegroundService>) {
    self.active.insert(name.into(), service);
  }

  pub fn start(&mut self, name: &str) -> bool {
    if self.active.contains_key(name) {
      return true;
    }
    if let Some(factory) = self.available.get(name) {
      let service = factory();
      self.active.insert(name.to_string(), service);
      return true;
    }
    false
  }

  pub fn is_active(&self, name: &str) -> bool {
    self.active.contains_key(name)
  }

  pub fn active_names(&self) -> Vec<String> {
    self.active.keys().cloned().collect()
  }

  pub fn available_names(&self) -> Vec<String> {
    self.available.keys().cloned().collect()
  }

  pub fn update(&mut self, store: &mut Store, action: &ListenerAction) {
    for service in self.active.values_mut() {
      if service.update(store, action) {
        break;
      }
    }
  }
}

#[derive(Clone)]
pub struct ForegroundServicesHandle(pub Arc<Mutex<ForegroundServices>>);

impl ForegroundServicesHandle {
  pub fn new(services: Arc<Mutex<ForegroundServices>>) -> Self {
    Self(services)
  }

  pub fn start(&self, name: &str) -> bool {
    if let Ok(mut lock) = self.0.lock() {
      lock.start(name)
    } else {
      false
    }
  }

  pub fn is_active(&self, name: &str) -> bool {
    if let Ok(lock) = self.0.lock() {
      lock.is_active(name)
    } else {
      false
    }
  }
}

pub fn start_foreground(store: &Store, name: &str) -> bool {
  if let Some(handle) = store.borrow::<ForegroundServicesHandle>() {
    handle.start(name)
  } else {
    false
  }
}
