use image::imageops::FilterType;
use zed::unstable::gpui::{App, Image, ImageFormat};

pub mod observability;
mod users;
mod views;

pub fn init(cx: &mut App) {
    // observability::init(cx);
    zed::init(cx);
    plugin_vault::init(cx);
    plugin_willow::init(cx);
    plugin_p2p::init(cx);
    plugin_calendar::init(cx);
    plugin_chat::init(cx);
    plugin_theme_palette::init(cx);
    users::init(cx);
    views::init(cx);
}

pub fn identicon(bytes: &[u8]) -> Image {
    let identicon =
        plot_icon::generate_png_scaled_custom(bytes, 127, 4, FilterType::Triangle).unwrap();
    Image::from_bytes(ImageFormat::Png, identicon)
}
