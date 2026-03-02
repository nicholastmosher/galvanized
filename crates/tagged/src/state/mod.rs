use zed::unstable::ui::App;

pub mod profile;

pub fn init(cx: &mut App) {
    profile::init(cx);
}
