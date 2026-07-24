use gpui::App;

pub mod observability;

pub fn init(cx: &mut App) {
    observability::init(cx);
    // zed::init(cx);
    gpui_component::init(cx);
    plugin_galvanized::init(cx);
    galvanized_core::init(cx);

    plugin_vault::init(cx);
    plugin_p2p::init(cx);
    plugin_willow::init(cx);
    plugin_automerge::init(cx);
    plugin_calendar::init(cx);
    plugin_chat::init(cx);
    plugin_contacts::init(cx);
    plugin_files::init(cx);

    plugin_theme_palette::init(cx);
}
