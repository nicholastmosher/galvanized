use gpui::App;

mod willow_panel;
mod willow_ui;

pub use zed::unstable::{db, gpui, settings, util, workspace};

pub fn init(cx: &mut App) {
    willow_ui::init(cx);
    willow_panel::init(cx);
}
