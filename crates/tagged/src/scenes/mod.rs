use zed::unstable::ui::App;

pub mod tagged_panel;

pub fn init(cx: &mut App) {
    tagged_panel::init(cx);
}
