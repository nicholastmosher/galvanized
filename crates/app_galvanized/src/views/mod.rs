use zed::unstable::ui::App;

pub mod connections;
pub mod create_profile_item;
// pub mod create_profile_modal;
// pub mod create_space_modal;
pub mod panel_root;
pub mod vault_login_item;

pub fn init(cx: &mut App) {
    connections::init(cx);
    panel_root::init(cx);
    // create_profile_item::init(cx);
    vault_login_item::init(cx);
}
