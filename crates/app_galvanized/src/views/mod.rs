use zed::unstable::ui::App;

pub mod connections;
pub mod panel_root;
pub mod profile_login;
pub mod vault_login_item;

pub fn init(cx: &mut App) {
    connections::init(cx);
    panel_root::init(cx);
    vault_login_item::init(cx);
}
