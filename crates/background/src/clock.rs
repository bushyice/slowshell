use std::{cell::RefCell, collections::HashMap, path::Path};

use chrono::{Local, Timelike};
use iced::{
  ContentFit, Element, Length, Radians, Rotation,
  widget::{Space, container, image, stack},
};
use slowshell_config::Config;
use slowshell_core::message::ItemMessage;
use slowshell_widgets::ComponentOptions;

const DEFAULT_DIAL: f32 = 96.0;
const FACE_BORDER: f32 = 2.0;

const HAND_PAD: f32 = 1.5;

const HOUR_LENGTH: f32 = 0.28;
const MINUTE_LENGTH: f32 = 0.42;
const HOUR_THICKNESS: f32 = 0.06;
const MINUTE_THICKNESS: f32 = 0.04;

const FACE_CLASSES: &[&str] = &["clock", "analog", "face"];
const HOUR_CLASSES: &[&str] = &["clock-hand", "hour"];
const MINUTE_CLASSES: &[&str] = &["clock-hand", "minute"];

pub type HandCache = RefCell<HashMap<String, image::Handle>>;

pub fn view(
  config: &Config,
  options: Option<&ComponentOptions>,
  cache: &HandCache,
) -> Element<'static, ItemMessage> {
  let face = config.computed(FACE_CLASSES);
  let dial = face
    .length("width")
    .or_else(|| face.length("height"))
    .or_else(|| option_number(options, "size"))
    .unwrap_or(DEFAULT_DIAL)
    .clamp(8.0, 1024.0);
  let radius = dial / 2.0;

  let background = config
    .color(FACE_CLASSES, "background-color")
    .unwrap_or(config.palette().mantle);
  let border_width = face.length("border-width").unwrap_or(FACE_BORDER).max(0.0);
  let border_color = config
    .color(FACE_CLASSES, "border-color")
    .unwrap_or(config.palette().overlay);
  let shadow = face
    .shadow("box-shadow")
    .map(|(x, y, blur, color)| iced::Shadow {
      color: color.to_iced(),
      offset: iced::Vector::new(x, y),
      blur_radius: blur,
    })
    .unwrap_or_default();

  let base_style = container::Style {
    text_color: None,
    background: Some(background.into()),
    border: iced::Border {
      color: border_color,
      width: 0.0,
      radius: radius.into(),
    },
    shadow,
    snap: false,
  };

  let mut layers: Vec<Element<'static, ItemMessage>> = Vec::with_capacity(5);
  layers.push(
    container(Space::new())
      .width(dial)
      .height(dial)
      .style(move |_| base_style)
      .into(),
  );

  if let Some(path) = config.background_image(FACE_CLASSES) {
    layers.push(
      image(image::Handle::from_path(path))
        .width(Length::Fixed(dial))
        .height(Length::Fixed(dial))
        .content_fit(ContentFit::Cover)
        .border_radius(radius)
        .into(),
    );
  }

  if border_width > 0.0 {
    let ring_style = container::Style {
      text_color: None,
      background: None,
      border: iced::Border {
        color: border_color,
        width: border_width,
        radius: radius.into(),
      },
      shadow: iced::Shadow::default(),
      snap: false,
    };

    layers.push(
      container(Space::new())
        .width(dial)
        .height(dial)
        .style(move |_| ring_style)
        .into(),
    );
  }

  let now = Local::now();
  let (hour, minute) = hand_angles(now.hour(), now.minute(), now.second());

  let fallback_color = options
    .and_then(|options| options.str("color"))
    .and_then(|name| config.palette().color(name))
    .unwrap_or(config.palette().text);

  layers.push(hand(
    config,
    cache,
    HOUR_CLASSES,
    fallback_color,
    hour,
    dial,
    HOUR_LENGTH,
    HOUR_THICKNESS,
  ));
  layers.push(hand(
    config,
    cache,
    MINUTE_CLASSES,
    fallback_color,
    minute,
    dial,
    MINUTE_LENGTH,
    MINUTE_THICKNESS,
  ));

  stack(layers)
    .width(Length::Fixed(dial))
    .height(Length::Fixed(dial))
    .into()
}

fn option_number(options: Option<&ComponentOptions>, key: &str) -> Option<f32> {
  let options = options?;
  options
    .number(key)
    .or_else(|| options.int(key).map(|value| value as f32))
}

fn hand_angles(hour: u32, minute: u32, second: u32) -> (f32, f32) {
  let minute = minute as f32 + second as f32 / 60.0;
  let hour = (hour % 12) as f32 + minute / 60.0;
  (hour * 30.0, minute * 6.0)
}

fn hand(
  config: &Config,
  cache: &HandCache,
  classes: &[&str],
  fallback_color: iced::Color,
  angle: f32,
  dial: f32,
  default_length: f32,
  default_thickness: f32,
) -> Element<'static, ItemMessage> {
  let computed = config.computed(classes);
  let thickness = computed
    .length("width")
    .unwrap_or((dial * default_thickness).max(1.5))
    .round()
    .clamp(1.0, 128.0);
  let length = computed
    .length("height")
    .unwrap_or(dial * default_length)
    .round()
    .clamp(2.0, 2048.0);
  let color = config
    .color(classes, "background-color")
    .unwrap_or(fallback_color);
  let border_radius = computed.length("border-radius").unwrap_or(0.0);
  let path = config.background_image(classes);

  let sprite = hand_sprite(
    cache,
    path.as_deref(),
    color,
    thickness,
    length,
    border_radius,
  );

  let sprite_size = iced::Size::new(thickness + HAND_PAD * 2.0, length * 2.0 + HAND_PAD * 2.0);
  let radians = angle.to_radians();
  let box_size = Rotation::Solid(Radians(radians)).apply(sprite_size);

  let sprite_image = image(sprite)
    .width(Length::Fixed(box_size.width))
    .height(Length::Fixed(box_size.height))
    .content_fit(ContentFit::Fill)
    .filter_method(image::FilterMethod::Linear)
    .rotation(Rotation::Solid(Radians(radians)));

  container(sprite_image)
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

fn hand_sprite(
  cache: &HandCache,
  path: Option<&Path>,
  color: iced::Color,
  thickness: f32,
  length: f32,
  border_radius: f32,
) -> image::Handle {
  let width = (thickness.round().max(1.0) + HAND_PAD * 2.0) as u32;
  let height = (length.round().max(2.0) * 2.0 + HAND_PAD * 2.0) as u32;
  let [red, green, blue, alpha] = color.into_rgba8();
  let key = format!(
    "{path:?}|{width}x{height}|{red},{green},{blue},{alpha}|{border_radius}|{}",
    path.map(file_stamp).unwrap_or_default()
  );

  if let Some(handle) = cache.borrow().get(&key) {
    return handle.clone();
  }

  let sprite = build_hand_sprite(path, color, width, height, border_radius);
  let handle = image::Handle::from_rgba(width, height, sprite.into_raw());
  cache.borrow_mut().insert(key, handle.clone());
  handle
}

fn build_hand_sprite(
  path: Option<&Path>,
  color: iced::Color,
  width: u32,
  height: u32,
  border_radius: f32,
) -> ::image::RgbaImage {
  let [red, green, blue, alpha] = color.into_rgba8();
  let mut sprite = ::image::RgbaImage::from_pixel(width, height, ::image::Rgba([0, 0, 0, 0]));

  let left = HAND_PAD;
  let top = HAND_PAD;
  let right = width as f32 - HAND_PAD;
  let base = top + (height as f32 - HAND_PAD * 2.0) / 2.0;
  let radius = border_radius
    .round()
    .clamp(0.0, ((right - left).min(base - top) / 2.0).max(0.0));

  for y in 0..height {
    for x in 0..width {
      let coverage = hand_coverage(x, y, left, top, right, base, radius);
      if coverage <= 0.0 {
        continue;
      }

      sprite.put_pixel(
        x,
        y,
        ::image::Rgba([red, green, blue, (coverage * alpha as f32).round() as u8]),
      );
    }
  }

  if let Some(path) = path {
    match ::image::open(path) {
      Ok(source) => {
        let hand_width = (right - left).round().max(1.0) as u32;
        let hand_height = (base - top).round().max(1.0) as u32;
        let scaled = ::image::imageops::resize(
          &source.to_rgba8(),
          hand_width,
          hand_height,
          ::image::imageops::FilterType::Lanczos3,
        );
        let offset_x = left.round() as u32;
        let offset_y = top.round() as u32;

        for (x, y, pixel) in scaled.enumerate_pixels() {
          let (target_x, target_y) = (offset_x + x, offset_y + y);
          if target_x >= width || target_y >= height {
            continue;
          }

          let coverage = hand_coverage(target_x, target_y, left, top, right, base, radius);
          let source_alpha = pixel[3] as f32 / 255.0 * coverage;
          if source_alpha <= 0.0 {
            continue;
          }

          blend_pixel(
            sprite.get_pixel_mut(target_x, target_y),
            pixel,
            source_alpha,
          );
        }
      }
      Err(error) => {
        eprintln!(
          "[clock] failed to open hand image '{}': {error}",
          path.display()
        );
      }
    }
  }

  sprite
}

fn hand_sdf(px: f32, py: f32, left: f32, top: f32, right: f32, base: f32, radius: f32) -> f32 {
  let half_width = (right - left) / 2.0;
  let half_height = (base - top) / 2.0;
  let qx = (px - (left + half_width)).abs() - half_width;
  let qy = (py - (top + half_height)).abs() - half_height;
  let rectangle = qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0);

  if radius <= 0.0 || py >= top + radius {
    return rectangle;
  }

  let corner = if px < left + radius {
    let dx = left + radius - px;
    let dy = top + radius - py;

    (dx > 0.0 && dy > 0.0).then(|| dx.hypot(dy) - radius)
  } else if px > right - radius {
    let dx = px - (right - radius);
    let dy = top + radius - py;

    (dx > 0.0 && dy > 0.0).then(|| dx.hypot(dy) - radius)
  } else {
    None
  };

  match corner {
    Some(corner) => rectangle.max(corner),
    None => rectangle,
  }
}

fn hand_coverage(x: u32, y: u32, left: f32, top: f32, right: f32, base: f32, radius: f32) -> f32 {
  let sdf = hand_sdf(
    x as f32 + 0.5,
    y as f32 + 0.5,
    left,
    top,
    right,
    base,
    radius,
  );

  (0.5 - sdf).clamp(0.0, 1.0)
}

fn blend_pixel(target: &mut ::image::Rgba<u8>, source: &::image::Rgba<u8>, source_alpha: f32) {
  let target_alpha = target[3] as f32 / 255.0;
  let out_alpha = source_alpha + target_alpha * (1.0 - source_alpha);

  if out_alpha <= 0.0 {
    *target = ::image::Rgba([0, 0, 0, 0]);

    return;
  }

  for channel in 0..3 {
    let source_channel = source[channel] as f32 / 255.0;
    let target_channel = target[channel] as f32 / 255.0;
    let out = (source_channel * source_alpha
      + target_channel * target_alpha * (1.0 - source_alpha))
      / out_alpha;

    target[channel] = (out * 255.0).round().clamp(0.0, 255.0) as u8;
  }

  target[3] = (out_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
}

fn file_stamp(path: &Path) -> u64 {
  std::fs::metadata(path)
    .and_then(|metadata| metadata.modified())
    .ok()
    .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
    .map(|duration| duration.as_secs())
    .unwrap_or(0)
}
