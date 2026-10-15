use iced::{
  Element, Length,
  widget::{column, container},
};
use iced_exwlshell::reexport::IcedId;
use slowshell_config::Config;
use slowshell_core::{
  Store,
  message::{EventFilter, ItemEffect, ItemMessage},
};
use slowshell_popups::PopupSettings;
use slowshell_widgets::{
  Backdrop, Renderable, SizedPopup, SystemMonOptions, render_system_mon_widget,
  set_sample_processes,
};

const TICK: &str = "component/system-mon.tick";

pub struct SystemMonRenderable;

impl Renderable for SystemMonRenderable {
  fn events(&self, _store: &Store) -> Vec<EventFilter> {
    vec![EventFilter::Named(TICK.into())]
  }

  fn initialize(&self, _config: &Config, store: &mut Store) -> Box<dyn std::any::Any + Send> {
    set_sample_processes(store, true);
    Box::new(())
  }

  fn close(&mut self, store: &mut Store) {
    set_sample_processes(store, false);
  }

  fn view<'a>(
    &self,
    config: &'a Config,
    store: &'a Store,
    id: IcedId,
    data: &'a dyn std::any::Any,
  ) -> Element<'a, ItemMessage> {
    let data = data.downcast_ref::<PopupSettings>().unwrap();

    let header = config.text("System Monitor", &["menu", "text-header"]);

    let options = SystemMonOptions {
      show_processes: true,
      max_processes: 10,
      show_temp: true,
    };

    let widget = render_system_mon_widget(config, store, options);

    let body = column![header, widget,]
      .spacing(config.sizing(&["menu"]).gap)
      .width(Length::Fixed(
        config
          .computed(&["menu", "list", "large"])
          .length("list-large-width")
          .unwrap_or(320.0),
      ));

    let inner_padding = config.sizing(&["menu", "inner"]).padding();
    let popup_style = config.container_cached(&["menu"]);

    SizedPopup::new(container(
      container(body).padding(inner_padding).style(popup_style),
    ))
    .with_width(280.)
    .with_backdrop(Backdrop::transparent().with_close_on_click(true))
    .with_position(data.position.0.get(store), data.position.1.get(store))
    .with_panel_edge(data.panel_edge)
    .with_panel_insets(store, data.monitor.as_deref())
    .with_on_close(ItemMessage::Effect(id, ItemEffect::Hide))
    .into()
  }
}
