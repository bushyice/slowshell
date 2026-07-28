use iced::{
  Color, Element, Length,
  widget::{container, image},
};
use iced_layershell::reexport::{
  Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption,
};
use slowshell_config::Config;
use slowshell_core::{Store, listeners::ListenerAction};

use crate::{
  DesktopItem, EventFilter, ItemEffect, ItemMessage, MonitorScope, UpdateWhen, Visibility,
};

pub struct Wallpaper {
  path: &'static str,
}

impl Wallpaper {
  pub fn new(path: &'static str) -> Self {
    Self { path }
  }
}

impl DesktopItem for Wallpaper {
  fn id(&self) -> &str {
    "wallpaper"
  }

  fn layer(&self, _: &Config, monitor: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
      layer: Layer::Background,
      anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
      exclusive_zone: Some(-1),
      size: None,
      margin: Some((0, 0, 0, 0)),
      keyboard_interactivity: KeyboardInteractivity::None,
      events_transparent: true,
      namespace: Some(format!("slowshell-wallpaper")),
      output_option: OutputOption::OutputName(monitor.to_string()),
    }
  }

  fn visibility(&self) -> Visibility {
    Visibility::Visible
  }

  fn update_strategy(&self) -> UpdateWhen {
    UpdateWhen::OnDemand
  }

  fn monitor(&self, _: &Config) -> MonitorScope {
    MonitorScope::PerMonitor
  }

  // fn clone_box(&self) -> Box<dyn DesktopItem> {
  //   Box::new(Self { path: self.path })
  // }

  fn init_events(&self) -> Vec<EventFilter> {
    vec![]
  }

  fn update(&mut self, _store: &Store, event: &ListenerAction) -> anyhow::Result<ItemEffect> {
    match event {
      _ => Ok(ItemEffect::None),
    }
  }

  fn view(
    &self,
    _store: &Store,
    _id: iced_layershell::reexport::IcedId,
  ) -> Element<'_, ItemMessage> {
    container(
      image(self.path)
        .content_fit(iced::ContentFit::Cover)
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_theme: &iced::Theme| container::Style {
      background: Some(Color::from_rgb(0.1, 0.1, 0.15).into()),
      ..Default::default()
    })
    .into()
  }
}
