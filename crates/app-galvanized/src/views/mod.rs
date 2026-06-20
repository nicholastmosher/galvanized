use zed::unstable::ui::App;

pub mod vault_login_item;

pub fn init(cx: &mut App) {
    vault_login_item::init(cx);
}
