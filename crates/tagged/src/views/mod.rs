use zed::unstable::ui::App;

pub mod create_space_modal;
pub mod onboarding_item;
pub mod tagged_panel;

pub fn init(cx: &mut App) {
    onboarding_item::init(cx);
    tagged_panel::init(cx);
}
