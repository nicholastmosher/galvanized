use zed::unstable::gpui::App;

mod codec;
mod iroh_panel_ui;
pub mod iroh_repo;

pub fn init(cx: &mut App) {
    iroh_repo::init(cx);
    iroh_panel_ui::init(cx);
}
