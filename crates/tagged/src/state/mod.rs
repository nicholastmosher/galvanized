use zed::unstable::ui::App;

pub mod profile;
pub mod space;

pub fn init(cx: &mut App) {
    profile::init(cx);
    space::init(cx);
}
