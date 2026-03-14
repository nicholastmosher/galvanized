use zed::unstable::ui::App;

pub mod create_profile_modal;
pub mod create_space_modal;
pub mod tagged_panel;

pub fn init(cx: &mut App) {
    tagged_panel::init(cx);
}
