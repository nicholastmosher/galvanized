use zed::unstable::ui::{App, Context, SharedString};

pub fn init(cx: &mut App) {
    // cx.observe_new(|workspace: &mut Workspace, window, cx| {
    //     let Some(window) = window else {
    //         return;
    //     };
    // })
    // .detach();
}

pub struct Space {
    //
    name: SharedString,
}

impl Space {
    pub fn new(name: impl Into<SharedString>, _cx: &mut Context<Self>) -> Self {
        Space { name: name.into() }
    }

    pub fn name(&self) -> SharedString {
        self.name.clone()
    }
}
