use zed::unstable::{
    gpui::{self, Entity},
    ui::{App, IntoElement, RenderOnce, Window, div},
};

use crate::state::profile::Profile;

pub fn init(cx: &mut App) {
    // cx.observe_new(|workspace: &mut Workspace, window, cx| {
    //     let Some(window) = window else {
    //         return;
    //     };
    // })
    // .detach();
}

#[derive(IntoElement)]
pub struct SpaceHeader {
    //
    profile: Entity<Profile>,
}

impl SpaceHeader {
    pub fn new(profile: Entity<Profile>) -> Self {
        SpaceHeader { profile }
    }
}

impl RenderOnce for SpaceHeader {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
    }
}
