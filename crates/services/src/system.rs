use std::{sync::Arc, time::Duration};

use slowshell_commons::system::{ProcessInfo, SharedSystemState, SystemSnapshot};
use sysinfo::{
  Components, CpuRefreshKind, MemoryRefreshKind, Networks, ProcessRefreshKind, RefreshKind, System,
  UpdateKind,
};

// TODO: Add sort-by
pub fn run(shared: SharedSystemState, update_time: Duration) {
  std::thread::spawn(move || {
    let mut sys = System::new_with_specifics(
      RefreshKind::nothing()
        .with_cpu(CpuRefreshKind::everything())
        .with_memory(MemoryRefreshKind::everything())
        .with_processes(ProcessRefreshKind::everything()),
    );
    let mut components = Components::new_with_refreshed_list();
    let mut networks = Networks::new_with_refreshed_list();

    sample(&shared, &mut sys, &mut components, &mut networks);

    loop {
      std::thread::sleep(update_time);
      sample(&shared, &mut sys, &mut components, &mut networks);
    }
  });
}

fn sample(
  shared: &SharedSystemState,
  sys: &mut System,
  components: &mut Components,
  networks: &mut Networks,
) {
  sys.refresh_cpu_all();
  let cpu_usage = sys.global_cpu_usage();

  sys.refresh_memory();
  let total = sys.total_memory();
  let used = sys.used_memory();
  let mem_usage = if total > 0 {
    (used as f32 / total as f32 * 100.0).clamp(0.0, 100.0)
  } else {
    0.0
  };

  let processes: Vec<ProcessInfo> = if shared
    .sample_processes
    .load(std::sync::atomic::Ordering::Relaxed)
  {
    sys.refresh_processes_specifics(
      sysinfo::ProcessesToUpdate::All,
      true,
      ProcessRefreshKind::nothing()
        .with_cpu()
        .with_memory()
        .with_exe(UpdateKind::OnlyIfNotSet),
    );
    let mut procs: Vec<ProcessInfo> = sys
      .processes()
      .iter()
      .filter_map(|(pid, proc_info)| {
        let name = proc_info.exe()?.file_stem()?.to_string_lossy().into_owned();
        let name_display = if name.chars().count() > 14 {
          format!("{}…", name.chars().take(13).collect::<String>())
        } else {
          name.clone()
        };
        let memory = proc_info.memory();
        let cpu_usage = proc_info.cpu_usage();

        Some(ProcessInfo {
          pid: pid.as_u32(),
          name,
          name_display,
          cpu_usage,
          cpu_display: format!("{cpu_usage:.0}%"),
          memory,
          memory_display: format_memory(memory),
        })
      })
      .collect();

    procs.sort_by(|a, b| {
      b.cpu_usage
        .partial_cmp(&a.cpu_usage)
        .unwrap_or(std::cmp::Ordering::Equal)
    });

    procs.truncate(10);
    procs
  } else {
    Vec::new()
  };

  components.refresh(false);
  let temperature = components.iter().find_map(|c| c.temperature());

  let load = System::load_average();
  let load = [load.one as f32, load.five as f32, load.fifteen as f32];

  networks.refresh(true);

  let mut rx = 0;
  let mut tx = 0;
  let mut current_state = 0;

  for (name, net) in &*networks {
    if name == "lo" {
      continue;
    }
    rx += net.received();
    tx += net.transmitted();

    if current_state < 2 {
      if name.starts_with('w') {
        current_state = 1;
      } else if name.starts_with('e') || name.starts_with("eth") {
        current_state = 2;
      }
    }
  }

  let mut guard = shared.snapshot.lock().unwrap();
  *guard = Arc::new(SystemSnapshot {
    cpu_usage,
    mem_usage,
    mem_total: total,
    mem_used: used,
    temperature,
    load,
    network_state: current_state,
    network_rx: rx,
    network_tx: tx,
    top_processes: processes,
  });
}

fn format_memory(bytes: u64) -> String {
  let mb = bytes as f64 / 1024.0 / 1024.0;
  if mb >= 1024.0 {
    format!("{:.1}G", mb / 1024.0)
  } else {
    format!("{:.0}M", mb)
  }
}
