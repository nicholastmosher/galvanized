use zed::unstable::gpui::App;

mod iroh_panel_ui;

pub fn init(cx: &mut App) {
    iroh_panel_ui::init(cx);
}
