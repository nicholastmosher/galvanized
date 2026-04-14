use zed::unstable::ui::App;

pub mod connections;
pub mod create_profile_modal;
pub mod create_space_modal;
pub mod panel_root;

pub fn init(cx: &mut App) {
    connections::init(cx);
    panel_root::init(cx);
}
