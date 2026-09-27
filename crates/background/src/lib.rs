pub mod wallpaper;
#[cfg(feature = "widgets")]
pub mod widgets;

#[cfg(feature = "widgets")]
slowshell_registry::register_resources!(
  renderables: Renderable {
    name: "widgets/icon".into(),
    create: |_, _| Some(Box::new(widgets::DesktopIconWidget))
  },
  renderables: Renderable {
    name: "widgets/clock".into(),
    create: |_, _| Some(Box::new(widgets::ClockWidget))
  },
  renderables: Renderable {
    name: "widgets/nowplaying".into(),
    create: |_, _| Some(Box::new(widgets::NowplayingWidget))
  },
  renderables: Renderable {
    name: "widgets/sysmon".into(),
    create: |_, _| Some(Box::new(widgets::SysmonWidget))
  },
);

#[cfg(not(feature = "widgets"))]
pub fn include(_registry: &mut ::slowshell_registry::GlobalRegistry) {}
