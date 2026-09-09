use std::os::fd::OwnedFd;

use slowshell_commons::tray::{SharedTrayState, TrayCmd};

use crate::util;

pub fn run(
  shared: SharedTrayState,
  mut cmd_rx: tokio::sync::mpsc::UnboundedReceiver<TrayCmd>,
  notify: Option<OwnedFd>,
) {
  crate::shared_runtime().spawn(async move {
    let client = match system_tray::client::Client::new().await {
      Ok(c) => std::sync::Arc::new(c),
      Err(e) => {
        eprintln!("[tray] failed to start client: {e}");
        return;
      }
    };

    sync(&client, &shared, notify.as_ref());

    let mut rx = client.subscribe();
    loop {
      tokio::select! {
        event = rx.recv() => {
          match event {
            Ok(_) => sync(&client, &shared, notify.as_ref()),
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => sync(&client, &shared, notify.as_ref()),
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
              eprintln!("[tray] subscription closed");
              break;
            }
          }
        }
        cmd = cmd_rx.recv() => {
          match cmd {
            Some(TrayCmd::Activate { address, menu_path, submenu_id }) => {
              let client = client.clone();
              tokio::spawn(async move {
                if let Err(e) = client.activate(
                  system_tray::client::ActivateRequest::MenuItem { address, menu_path, submenu_id },
                ).await {
                  eprintln!("[tray] activate failed: {e}");
                }
              });
            }
            Some(TrayCmd::AboutToShow { address, menu_path, submenu_id }) => {
              let client = client.clone();
              tokio::spawn(async move {
                let fut = client.about_to_show_menuitem(address, menu_path, submenu_id);
                match tokio::time::timeout(std::time::Duration::from_millis(500), fut).await {
                  Ok(Err(e)) => eprintln!("[tray] about_to_show failed: {e}"),
                  Err(_) => eprintln!("[tray] about_to_show timed out after 500ms"),
                  _ => {}
                }
              });
            }
            None => break,
          }
        }
      }
    }
  });
}

fn sync(client: &system_tray::client::Client, shared: &SharedTrayState, notify: Option<&OwnedFd>) {
  use std::sync::atomic::Ordering;

  let items = client.items();
  let map = match items.lock() {
    Ok(m) => m,
    Err(_) => return,
  };

  let mut state = shared.state.lock().unwrap();
  let mut menus = shared.menus.lock().unwrap();
  state.items.clear();
  menus.clear();
  for (address, (item, menu)) in map.iter() {
    let pixmap = item.icon_pixmap.as_ref().and_then(|pm| {
      pm.iter()
        .max_by_key(|p| p.width * p.height)
        .map(|p| (p.width as u32, p.height as u32, argb_to_rgba(&p.pixels)))
    });
    let title = item.title.clone().unwrap_or_else(|| item.id.clone());
    state.items.push(slowshell_commons::tray::TrayItem {
      address: address.clone(),
      title,
      icon_name: item.icon_name.clone(),
      pixmap,
      menu_path: item.menu.clone(),
    });
    if let Some(menu) = menu {
      menus.insert(address.clone(), menu.clone());
    }
  }
  drop(map);

  shared.revision.fetch_add(1, Ordering::Relaxed);
  if let Some(fd) = notify {
    util::notify_signal(fd);
  }
}

// util
fn argb_to_rgba(argb: &[u8]) -> Vec<u8> {
  let mut out = Vec::with_capacity(argb.len());
  for px in argb.chunks_exact(4) {
    out.push(px[1]);
    out.push(px[2]);
    out.push(px[3]);
    out.push(px[0]);
  }
  out
}
