use std::{
  collections::HashMap,
  io::{BufRead, BufReader},
  os::fd::OwnedFd,
  sync::atomic::Ordering,
  time::Duration,
};

use futures_util::StreamExt;
use slowshell_commons::audio::{AudioCmd, AudioSink, MprisPlayer, SharedAudioState};
use zbus::zvariant::Value;

use crate::util;

pub fn run(
  shared: SharedAudioState,
  mut cmd_rx: tokio::sync::mpsc::UnboundedReceiver<AudioCmd>,
  notify: Option<OwnedFd>,
) {
  crate::shared_runtime().spawn(async move {
    loop {
      match audio_loop(&shared, &mut cmd_rx, &notify).await {
        Ok(()) => break,
        Err(e) => {
          eprintln!("[audio] loop error: {e}");
          tokio::time::sleep(Duration::from_millis(2000)).await;
        }
      }
    }
  });
}

async fn audio_loop(
  shared: &SharedAudioState,
  cmd_rx: &mut tokio::sync::mpsc::UnboundedReceiver<AudioCmd>,
  notify: &Option<OwnedFd>,
) -> miette::Result<()> {
  use miette::IntoDiagnostic;

  let session_conn = zbus::connection::Builder::session()
    .into_diagnostic()?
    .build()
    .await
    .into_diagnostic()?;

  refresh_audio_wpctl(shared);
  refresh_mpris(&session_conn, shared).await.ok();
  bump(shared, notify);

  let dbus_proxy = zbus::fdo::DBusProxy::new(&session_conn)
    .await
    .into_diagnostic()?;
  let mut name_owner_changed = dbus_proxy
    .receive_name_owner_changed()
    .await
    .into_diagnostic()?;

  let (pactl_tx, mut pactl_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
  std::thread::Builder::new()
    .name("slowshell-pactl-sub".into())
    .spawn(move || {
      let Ok(mut child) = std::process::Command::new("pactl")
        .arg("subscribe")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
      else {
        return;
      };

      if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
          match line {
            Ok(l) => {
              if l.contains("sink") || l.contains("server") {
                if pactl_tx.send(()).is_err() {
                  break;
                }
              }
            }
            Err(_) => break,
          }
        }
      }
      let _ = child.kill();
      let _ = child.wait();
    })
    .ok();

  let mut mpris_poll_interval = tokio::time::interval(Duration::from_millis(1500));

  loop {
    tokio::select! {
      Some(cmd) = cmd_rx.recv() => {
        handle_cmd(&session_conn, shared, cmd).await;
        refresh_audio_wpctl(shared);
        bump(shared, notify);
      }

      Some(()) = pactl_rx.recv() => {
        if refresh_audio_wpctl(shared) {
          bump(shared, notify);
        }
      }

      Some(_) = name_owner_changed.next() => {
        refresh_mpris(&session_conn, shared).await.ok();
        bump(shared, notify);
      }

      _ = mpris_poll_interval.tick() => {
        let mpris_changed = refresh_mpris(&session_conn, shared).await.unwrap_or(false);
        if mpris_changed {
          bump(shared, notify);
        }
      }
    }
  }
}

fn query_default_volume_fallback() -> Option<(f32, bool)> {
  let output = std::process::Command::new("wpctl")
    .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
    .output()
    .ok()?;
  let text = String::from_utf8_lossy(&output.stdout);
  let clean = text.trim();
  if let Some(rest) = clean.strip_prefix("Volume:") {
    let rest = rest.trim();
    let muted = rest.contains("MUTED");
    let num_str: String = rest
      .chars()
      .take_while(|c| c.is_ascii_digit() || *c == '.')
      .collect();
    if let Ok(v) = num_str.parse::<f32>() {
      return Some((v, muted));
    }
  }
  None
}

fn refresh_audio_wpctl(shared: &SharedAudioState) -> bool {
  let output = match std::process::Command::new("wpctl").arg("status").output() {
    Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
    Err(_) => return false,
  };

  let mut sinks = Vec::new();
  let mut in_sinks = false;
  let mut default_vol = 1.0;
  let mut default_muted = false;
  let mut default_name = None;
  let mut found_default_sink = false;

  for line in output.lines() {
    let clean_line: String = line
      .chars()
      .filter(|c| !matches!(*c, '│' | '├' | '─' | '└'))
      .collect();
    let trimmed = clean_line.trim();

    if trimmed.starts_with("Sinks:") {
      in_sinks = true;
      continue;
    } else if in_sinks
      && (trimmed.starts_with("Sources:")
        || trimmed.starts_with("Filters:")
        || trimmed.starts_with("Streams:")
        || trimmed.is_empty())
    {
      in_sinks = false;
    }

    if in_sinks {
      let is_default = line.contains('*');
      let clean = clean_line.replace('*', "").trim().to_string();

      if let Some((id_part, rest)) = clean.split_once('.') {
        if let Ok(id) = id_part.trim().parse::<u32>() {
          let mut description = rest.trim().to_string();
          let mut volume = 1.0;
          let mut muted = false;

          if let Some(bracket_start) = description.find('[') {
            let meta = &description[bracket_start..];
            if meta.contains("MUTED") {
              muted = true;
            }
            if let Some(vol_idx) = meta.find("vol:") {
              let vol_str = meta[vol_idx + 4..].trim_start();
              let vol_num_str: String = vol_str
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
              if let Ok(v) = vol_num_str.parse::<f32>() {
                volume = v;
              }
            }
            description = description[..bracket_start].trim().to_string();
          }

          if is_default {
            default_vol = volume;
            default_muted = muted;
            default_name = Some(description.clone());
            found_default_sink = true;
          }

          sinks.push(AudioSink {
            id,
            name: format!("{id}"),
            description,
            volume,
            muted,
            is_default,
          });
        }
      }
    }
  }

  if !found_default_sink {
    if let Some((vol, muted)) = query_default_volume_fallback() {
      default_vol = vol;
      default_muted = muted;
    }
  }

  let mut state = shared.state.lock().unwrap();
  let changed = (state.volume - default_vol).abs() > 0.005
    || state.muted != default_muted
    || state.sinks.len() != sinks.len()
    || state.default_sink_name != default_name;

  state.volume = default_vol;
  state.muted = default_muted;
  state.default_sink_name = default_name;
  state.sinks = sinks;

  changed
}

async fn refresh_mpris(conn: &zbus::Connection, shared: &SharedAudioState) -> miette::Result<bool> {
  use miette::IntoDiagnostic;

  let dbus_proxy = zbus::fdo::DBusProxy::new(conn).await.into_diagnostic()?;
  let names = dbus_proxy.list_names().await.into_diagnostic()?;

  let mut mpris_buses = Vec::new();
  for name in names {
    if name.starts_with("org.mpris.MediaPlayer2.") {
      mpris_buses.push(name.to_string());
    }
  }

  if mpris_buses.is_empty() {
    let mut cur = shared.player.lock().unwrap();
    let changed = cur.is_some();
    *cur = None;
    return Ok(changed);
  }

  let mut chosen_player: Option<MprisPlayer> = None;

  for bus in mpris_buses {
    let Ok(player_proxy) = zbus::Proxy::new_owned(
      conn.clone(),
      bus.clone(),
      "/org/mpris/MediaPlayer2",
      "org.mpris.MediaPlayer2.Player",
    )
    .await
    else {
      continue;
    };

    let Ok(root_proxy) = zbus::Proxy::new_owned(
      conn.clone(),
      bus.clone(),
      "/org/mpris/MediaPlayer2",
      "org.mpris.MediaPlayer2",
    )
    .await
    else {
      continue;
    };

    let identity: String = root_proxy
      .get_property("Identity")
      .await
      .unwrap_or_else(|_| bus.replace("org.mpris.MediaPlayer2.", ""));

    let playback_status: String = player_proxy
      .get_property("PlaybackStatus")
      .await
      .unwrap_or_else(|_| "Stopped".to_string());

    let metadata: HashMap<String, Value> = player_proxy
      .get_property("Metadata")
      .await
      .unwrap_or_default();

    let title = metadata
      .get("xesam:title")
      .and_then(|v| match v {
        Value::Str(s) => Some(s.to_string()),
        _ => None,
      })
      .unwrap_or_default();

    let artist = metadata
      .get("xesam:artist")
      .and_then(|v| match v {
        Value::Array(arr) => {
          let artists: Vec<String> = arr
            .iter()
            .filter_map(|x| match x {
              Value::Str(s) => Some(s.to_string()),
              _ => None,
            })
            .collect();
          Some(artists.join(", "))
        }
        Value::Str(s) => Some(s.to_string()),
        _ => None,
      })
      .unwrap_or_default();

    let album = metadata
      .get("xesam:album")
      .and_then(|v| match v {
        Value::Str(s) => Some(s.to_string()),
        _ => None,
      })
      .unwrap_or_default();

    let art_url = metadata.get("mpris:artUrl").and_then(|v| match v {
      Value::Str(s) => Some(s.to_string()),
      _ => None,
    });

    let can_play_pause: bool = player_proxy
      .get_property("CanControl")
      .await
      .unwrap_or(true);
    let can_go_next: bool = player_proxy.get_property("CanGoNext").await.unwrap_or(true);
    let can_go_previous: bool = player_proxy
      .get_property("CanGoPrevious")
      .await
      .unwrap_or(true);

    let player = MprisPlayer {
      bus_name: bus,
      identity,
      title,
      artist,
      album,
      art_url,
      playback_status,
      can_play_pause,
      can_go_next,
      can_go_previous,
    };

    let is_playing = player.playback_status == "Playing";
    let has_title = !player.title.is_empty();

    if is_playing {
      chosen_player = Some(player);
      break;
    } else if has_title && chosen_player.is_none() {
      chosen_player = Some(player);
    } else if chosen_player.is_none() {
      chosen_player = Some(player);
    }
  }

  let mut cur = shared.player.lock().unwrap();
  let changed = match (&*cur, &chosen_player) {
    (Some(old), Some(new_p)) => {
      old.title != new_p.title
        || old.artist != new_p.artist
        || old.playback_status != new_p.playback_status
        || old.bus_name != new_p.bus_name
    }
    (None, None) => false,
    _ => true,
  };

  *cur = chosen_player;
  Ok(changed)
}

async fn handle_cmd(conn: &zbus::Connection, shared: &SharedAudioState, cmd: AudioCmd) {
  match cmd {
    AudioCmd::SetVolume(vol) => {
      let clamped = vol.clamp(0.0, 1.5);
      let _ = std::process::Command::new("wpctl")
        .args([
          "set-volume",
          "-l",
          "1.5",
          "@DEFAULT_AUDIO_SINK@",
          &format!("{clamped:.2}"),
        ])
        .status();
    }
    AudioCmd::StepVolume(delta) => {
      let step_str = if delta >= 0.0 {
        format!("{}%+", (delta * 100.0).round().abs() as u32)
      } else {
        format!("{}%-", (delta * 100.0).round().abs() as u32)
      };
      let _ = std::process::Command::new("wpctl")
        .args(["set-volume", "-l", "1.5", "@DEFAULT_AUDIO_SINK@", &step_str])
        .status();
    }
    AudioCmd::ToggleMute => {
      let _ = std::process::Command::new("wpctl")
        .args(["set-mute", "@DEFAULT_AUDIO_SINK@", "toggle"])
        .status();
    }
    AudioCmd::SetDefaultSink(id) => {
      let _ = std::process::Command::new("wpctl")
        .args(["set-default", &id.to_string()])
        .status();
    }
    AudioCmd::PlayPause => {
      let bus_name = shared
        .player
        .lock()
        .unwrap()
        .as_ref()
        .map(|p| p.bus_name.clone());
      if let Some(bus) = bus_name {
        if let Ok(proxy) = zbus::Proxy::new_owned(
          conn.clone(),
          bus,
          "/org/mpris/MediaPlayer2",
          "org.mpris.MediaPlayer2.Player",
        )
        .await
        {
          let _ = proxy.call_method("PlayPause", &()).await;
        }
      }
    }
    AudioCmd::Next => {
      let bus_name = shared
        .player
        .lock()
        .unwrap()
        .as_ref()
        .map(|p| p.bus_name.clone());
      if let Some(bus) = bus_name {
        if let Ok(proxy) = zbus::Proxy::new_owned(
          conn.clone(),
          bus,
          "/org/mpris/MediaPlayer2",
          "org.mpris.MediaPlayer2.Player",
        )
        .await
        {
          let _ = proxy.call_method("Next", &()).await;
        }
      }
    }
    AudioCmd::Previous => {
      let bus_name = shared
        .player
        .lock()
        .unwrap()
        .as_ref()
        .map(|p| p.bus_name.clone());
      if let Some(bus) = bus_name {
        if let Ok(proxy) = zbus::Proxy::new_owned(
          conn.clone(),
          bus,
          "/org/mpris/MediaPlayer2",
          "org.mpris.MediaPlayer2.Player",
        )
        .await
        {
          let _ = proxy.call_method("Previous", &()).await;
        }
      }
    }
  }
}

fn bump(shared: &SharedAudioState, notify: &Option<OwnedFd>) {
  shared.revision.fetch_add(1, Ordering::SeqCst);
  if let Some(fd) = notify {
    util::notify_signal(fd);
  }
}
