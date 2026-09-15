use std::pin::Pin;
use std::sync::{Mutex, OnceLock};
use std::task::{Context, Poll};

use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender};
use iced::{Element, Task};
use iced_layershell::reexport::IcedId;
use slowshell_components::{ComponentRegistration, Components};
use slowshell_compositor::{CompositorRegistration, CompositorStore};
use slowshell_config::Config;
use slowshell_core::listeners::{FdHandle, ListenerAction, Listeners};
use slowshell_core::types::{PayloadBuilder, PayloadBuilderRegistry};
use slowshell_desktop::DesktopItems;
use slowshell_registry::{GlobalRegistry, ResourceRegistration};

use slowshell_core::Store;
use slowshell_core::message::Message;
use slowshell_ipc::IpcListener;
use slowshell_panels::{PanelDeloyer, PanelPositions};
use slowshell_popups::Popup;
use slowshell_widgets::Renderables;

static EPOLL_RX: OnceLock<Mutex<Option<UnboundedReceiver<Message>>>> = OnceLock::new();

pub(crate) fn register_plugin_components(
  registry: &mut GlobalRegistry,
  components: &mut Components,
) {
  for resource in registry.inside("components") {
    if let ResourceRegistration::Unknown(registration) = resource
      && let Ok(registration) = registration.downcast::<ComponentRegistration>()
    {
      components.insert(registration.name.clone(), registration.factory);
    }
  }
}

fn register_plugin_compositors(registry: &mut GlobalRegistry, compositors: &mut CompositorStore) {
  for resource in registry.inside("compositor") {
    if let ResourceRegistration::Unknown(registration) = resource
      && let Ok(registration) = registration.downcast::<CompositorRegistration>()
    {
      compositors.register(*registration);
    }
  }
}

pub struct App {
  config: Config,
  store: Store,
  items: DesktopItems,
  _tx: UnboundedSender<Message>,
  config_watcher_fd: Option<i32>,
}

impl App {
  pub fn new(
    registry: &mut GlobalRegistry,
    config: Config,
    tx: UnboundedSender<Message>,
  ) -> (Self, Task<Message>) {
    let mut store = Store::new();

    let mut listeners = Listeners::default();
    let mut tasks = Vec::new();
    let mut cs = CompositorStore::new();
    register_plugin_compositors(registry, &mut cs);

    match cs.initialize(&config, &mut listeners) {
      Err(e) => eprintln!("Failed to initialize compositor: {e}"),
      Ok(_) => {}
    }

    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
    let eloop_tx = tx.clone();
    let ipc_tx = tx.clone();
    let (mut eloop, wake_write) =
      crate::eloop::EventLoop::new(listeners, eloop_tx, config.tick_interval, cmd_rx)
        .expect("failed to create event loop");
    let fd_handle = FdHandle::new(cmd_tx, wake_write);

    let config_watcher_fd = if let Some(watch_dir) = config.current_dir() {
      match crate::watcher::ConfigWatcher::watch(&watch_dir) {
        Ok(watcher) => {
          let owned = watcher.into_owned_fd();
          let raw_fd = std::os::fd::AsRawFd::as_raw_fd(&owned);
          fd_handle.watch_with_flags(
            owned,
            nix::sys::epoll::EpollFlags::EPOLLIN | nix::sys::epoll::EpollFlags::EPOLLET,
            ListenerAction::Signal {
              name: "config.reload".into(),
              fd: raw_fd,
            },
          );
          Some(raw_fd)
        }
        Err(e) => {
          eprintln!("[config] failed to watch config directory: {e}");
          None
        }
      }
    } else {
      None
    };

    store.insert(fd_handle);

    let mut preg = PayloadBuilderRegistry::new();

    for payload in registry.inside("payload") {
      if let ResourceRegistration::Unknown(payload) = payload {
        if let Ok(builder) = payload.downcast::<PayloadBuilder>() {
          preg.register(*builder);
        }
      }
    }
    store.insert(IpcListener::new(ipc_tx, preg));

    store.insert(cs);
    store.insert(slowshell_core::ActionDispatcher(tx.clone()));

    // TODO: Move registration into a global registerar
    // start of registeration
    store.insert(PanelPositions::default());
    let mut renderables = Renderables::default();

    for renderable in registry.inside("renderables") {
      if let Some((name, renderable)) = renderable.as_renderable(&config, &mut store) {
        renderables.insert(name.into(), renderable);
      }
    }

    store.insert(renderables);
    let mut components = Components::default();
    slowshell_components::register_all(&mut components);
    register_plugin_components(registry, &mut components);
    store.insert(components);

    let mut items = DesktopItems::new();

    if let Some(cs) = store.borrow::<CompositorStore>() {
      if let Ok(state) = cs.state() {
        tasks.push(items.sync_monitors(&config, state.monitors.keys().cloned().collect()));
      }
    }

    for resource in registry.inside("app") {
      // TODO: handle err
      match resource.create(&config, &mut store, &mut items, registry) {
        Ok(Some(task)) => tasks.push(task),
        Err(e) => eprintln!("{e}"),
        _ => {}
      }
    }

    items.deployable(Box::new(PanelDeloyer::new(&config)));

    tasks.push(items.register(&config, Box::new(Popup::default())));

    // end of registration

    std::thread::spawn(move || {
      eloop.run();
    });

    (
      App {
        store,
        items,
        config,
        _tx: tx,
        config_watcher_fd,
      },
      Task::batch(tasks),
    )
  }

  pub fn update(&mut self, message: Message) -> Task<Message> {
    match message {
      Message::Tick => {
        self.items.intialize(&mut self.store);
        Task::none()
      }
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
          slowshell_core::listeners::ListenerAction::Named(name)
          | slowshell_core::listeners::ListenerAction::Signal { name, .. }
          | slowshell_core::listeners::ListenerAction::Timer { name, .. }
            if name.as_ref() == "config.reload" =>
          {
            if let Some(fd) = self.config_watcher_fd {
              crate::watcher::drain_inotify_fd(fd);
            }
            match self.config.reload() {
              Ok(()) => {
                println!("[config] Reloaded configuration");
              }
              Err(e) => {
                eprintln!("[config] Failed to reload configuration:\n{e:?}");
              }
            }
          }
          _ => {}
        }

        tasks.push(
          self
            .items
            .check_deployables(&self.config, &mut self.store, &action),
        );
        tasks.push(self.items.update(&self.config, &mut self.store, &action));
        self.items.intialize(&mut self.store);
        Task::batch(tasks)
      }
      Message::Item(msg) => match msg {
        slowshell_core::message::ItemMessage::Action(
          slowshell_core::listeners::ListenerAction::FocusWorkspace(idx),
        ) => {
          let task = {
            let Some(compositor) = self.store.borrow_mut::<CompositorStore>() else {
              return Task::none();
            };
            match compositor.send_cmd(slowshell_compositor::CompositorCommand::FocusWorkspace(
              idx as i32,
            )) {
              Err(e) => {
                eprintln!("Failed to focus workspace: {e}");
                Task::none()
              }
              Ok(_) => Task::none(),
            }
          };
          task
        }
        slowshell_core::message::ItemMessage::Action(action) => self.update_items(&action),
        slowshell_core::message::ItemMessage::EffectAction(id, effect, action) => Task::batch([
          self.items.handle_message(
            &self.config,
            &slowshell_core::message::ItemMessage::Effect(id, effect),
            Some(&mut self.store),
          ),
          self.update_items(&action),
        ]),
        slowshell_core::message::ItemMessage::Task(task) => (task)(),
        other => self
          .items
          .handle_message(&self.config, &other, Some(&mut self.store)),
      },
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
        let task = self.items.sync_monitors(&self.config, monitors);
        self.items.intialize(&mut self.store);
        task
      }
      Message::Noop => Task::none(),
      _ => Task::none(),
    }
  }

  pub fn subscription(&self) -> iced::Subscription<Message> {
    let events = iced::Subscription::run(epoll_stream);

    // bad
    if self.items.wants_frames() {
      iced::Subscription::batch([
        events,
        iced::window::frames()
          .map(|_| Message::FdUpdate(slowshell_core::listeners::ListenerAction::Frame)),
      ])
    } else {
      events
    }
  }

  fn update_items(&mut self, action: &ListenerAction) -> Task<Message> {
    let mut tasks = vec![
      self
        .items
        .check_deployables(&self.config, &mut self.store, action),
    ];
    tasks.push(self.items.update(&self.config, &mut self.store, action));
    self.items.intialize(&mut self.store);

    Task::batch(tasks)
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
  if let Some(item_view) = state.items.view(window_id, &state.config, &state.store) {
    return item_view.map(Message::Item);
  }

  iced::widget::space().into()
}
