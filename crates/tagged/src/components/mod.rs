use zed::unstable::{
    ui::App,
    workspace::{OpenComponentPreview, Workspace},
};

pub mod profile_switcher;

pub fn init(cx: &mut App) {
    profile_switcher::init(cx);

    // cx.observe_new(|workspace: &mut Workspace, window, cx| {
    //     let Some(window) = window else {
    //         return;
    //     };
    // })
    // .detach();
}
