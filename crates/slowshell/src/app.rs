use std::pin::Pin;
use std::sync::{Mutex, OnceLock};
use std::task::{Context, Poll};

use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use iced::{Element, Task};
use iced_layershell::reexport::{Anchor, IcedId};
use slowshell_compositor::CompositorStore;
use slowshell_config::Config;
use slowshell_core::listeners::{FdHandle, Listeners};
use slowshell_desktop::DesktopItems;
use slowshell_desktop::wallpaper::Wallpaper;

use slowshell_core::Store;
use slowshell_core::message::Message;
use slowshell_ipc::IpcListener;
use slowshell_notifications::NotificationManager;
use slowshell_panels::{Panel, PanelDeloyer, Position};
use slowshell_spotlight::Spotlight;

static EPOLL_RX: OnceLock<Mutex<Option<UnboundedReceiver<Message>>>> = OnceLock::new();

pub struct App {
  store: Store,
  items: DesktopItems,
  _tx: UnboundedSender<Message>,
}

impl App {
  pub fn new(config: Config, tx: UnboundedSender<Message>) -> (Self, Task<Message>) {
    let mut store = Store::new();

    let mut listeners = Listeners::default();
    let mut tasks = Vec::new();
    let mut cs = CompositorStore::new();

    match cs.initialize(&config, &mut listeners) {
      Err(e) => eprintln!("Failed to initialize compositor: {e}"),
      Ok(_) => {}
    }

    store.insert(cs);
    store.insert(Config);

    let mut items = DesktopItems::new();

    if let Some(cs) = store.borrow::<CompositorStore>() {
      if let Ok(state) = cs.state() {
        tasks.push(items.sync_monitors(&config, state.monitors.keys().cloned().collect()));
      }
    }

    tasks.push(items.register(
      &config,
      Box::new(NotificationManager::new(Anchor::Top | Anchor::Right)),
    ));
    tasks.push(items.register(
      &config,
      Box::new(Wallpaper::new("/home/makano/Pictures/bg/1387138.png")),
    ));
    tasks.push(items.register(&config, Box::new(Spotlight::new())));

    items.deployable(Box::new(PanelDeloyer));

    let main_bar = Panel::new("Main", Position::Top)
      .with_height(32)
      .with_item("left", "Workspaces", Some("core/workspaces".into()))
      .with_item("center", "Clock", Some("core/clock".into()))
      .with_item("right", "Battery", Some("core/battery".into()));
    tasks.push(items.register(&config, Box::new(main_bar)));

    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
    let eloop_tx = tx.clone();
    let ipc_tx = tx.clone();
    let (mut eloop, wake_write) = crate::eloop::EventLoop::new(listeners, eloop_tx, 3, cmd_rx)
      .expect("failed to create event loop");
    store.insert(FdHandle::new(cmd_tx, wake_write));
    store.insert(IpcListener::new(ipc_tx));
    std::thread::spawn(move || {
      eloop.run();
    });

    (
      App {
        store,
        items,
        _tx: tx,
      },
      Task::batch(tasks),
    )
  }

  pub fn update(&mut self, message: Message) -> Task<Message> {
    match message {
      Message::Tick => Task::none(),
      Message::FdUpdate(action) => {
        let mut tasks = Vec::new();

        match &action {
          slowshell_core::listeners::ListenerAction::UpdateCompositor => {
            let Some(mut compositor) = self.store.remove::<CompositorStore>() else {
              return Task::none();
            };

            match compositor.update_state(Some(&self.store), self._tx.clone()) {
              Err(e) => eprintln!("Failed to update compositor state: {e}"),
              Ok(_) => {}
            }

            self.store.insert(compositor);
          }
          _ => {}
        }

        tasks.push(self.items.check_deployables(&self.store, &action));
        tasks.push(self.items.update(&Config, &self.store, &action));
        Task::batch(tasks)
      }
      Message::Item(msg) => self.items.handle_message(&Config, &msg),
      Message::UpdateMonitors => {
        let monitors = {
          let Some(compositor) = self.store.borrow::<CompositorStore>() else {
            return Task::none();
          };
          match compositor.state() {
            Ok(state) => state.monitors.keys().cloned().collect(),
            Err(_) => return Task::none(),
          }
        };
        self.items.sync_monitors(&Config, monitors)
      }
      Message::Noop => Task::none(),
      _ => Task::none(),
    }
  }

  pub fn subscription(&self) -> iced::Subscription<Message> {
    iced::Subscription::run(epoll_stream)
  }
}

pub fn init_epoll_rx(rx: UnboundedReceiver<Message>) {
  let _ = EPOLL_RX.set(Mutex::new(Some(rx)));
}

struct EpollStream {
  rx: Option<UnboundedReceiver<Message>>,
}

impl iced::futures::Stream for EpollStream {
  type Item = Message;

  fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    match self.rx.as_mut() {
      Some(rx) => Pin::new(rx).poll_next(cx),
      None => Poll::Ready(None),
    }
  }
}

fn epoll_stream() -> EpollStream {
  let rx = EPOLL_RX
    .get()
    .and_then(|m| m.lock().ok())
    .and_then(|mut guard| guard.take());
  EpollStream { rx }
}

pub fn view(state: &App, window_id: IcedId) -> Element<'_, Message> {
  if let Some(item_view) = state.items.view(window_id, &state.store) {
    return item_view.map(Message::Item);
  }

  iced::widget::space().into()
}
