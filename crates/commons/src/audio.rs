use std::sync::{Arc, Mutex, atomic::AtomicU64};

pub const AUDIO_CHANGED: &str = "component/audio.changed";
pub const MPRIS_CHANGED: &str = "component/mpris.changed";

#[derive(Debug, Clone, Default)]
pub struct AudioSink {
  pub id: u32,
  pub name: String,
  pub description: String,
  pub volume: f32,
  pub muted: bool,
  pub is_default: bool,
}

#[derive(Debug, Clone, Default)]
pub struct AudioInner {
  pub volume: f32,
  pub muted: bool,
  pub default_sink_name: Option<String>,
  pub sinks: Vec<AudioSink>,
}

#[derive(Debug, Clone, Default)]
pub struct MprisPlayer {
  pub bus_name: String,
  pub identity: String,
  pub title: String,
  pub artist: String,
  pub album: String,
  pub art_url: Option<String>,
  pub playback_status: String,
  pub can_play_pause: bool,
  pub can_go_next: bool,
  pub can_go_previous: bool,
}

#[derive(Debug, Clone)]
pub enum AudioCmd {
  SetVolume(f32),
  StepVolume(f32),
  ToggleMute,
  SetDefaultSink(u32),
  PlayPause,
  Next,
  Previous,
}

pub struct AudioState {
  pub state: Mutex<AudioInner>,
  pub player: Mutex<Option<MprisPlayer>>,
  pub revision: AtomicU64,
  pub cmd_tx: tokio::sync::mpsc::UnboundedSender<AudioCmd>,
}

pub type SharedAudioState = Arc<AudioState>;

pub fn volume_icon(volume: f32, muted: bool) -> &'static str {
  if muted || volume <= 0.001 {
    "audio-volume-muted-symbolic"
  } else if volume < 0.33 {
    "audio-volume-low-symbolic"
  } else if volume < 0.66 {
    "audio-volume-medium-symbolic"
  } else {
    "audio-volume-high-symbolic"
  }
}
