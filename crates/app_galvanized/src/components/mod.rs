use zed::unstable::ui::App;

// pub mod onboarding_button;
pub mod profile_bar;
pub mod space_dropdown;
pub mod space_header;
// pub mod space_icon;

pub fn init(cx: &mut App) {
    profile_bar::init(cx);
    // space_header::init(cx);
    // space_icon::init(cx);
}
