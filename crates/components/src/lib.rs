pub mod wifi;

use slowshell_widgets::Renderables;

pub fn register_all(renderables: &mut Renderables) {
  renderables.insert("wifi".into(), Box::new(wifi::WifiRenderable));
}
