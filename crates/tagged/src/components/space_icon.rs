use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use zed::unstable::{
    gpui::{self, ObjectFit, img},
    ui::{
        AbsoluteLength, App, InteractiveElement, IntoElement, ParentElement as _, RenderOnce,
        Styled, StyledImage, Window, div, rems,
    },
};

pub fn init(cx: &mut App) {
    // cx.observe_new(|workspace: &mut Workspace, window, cx| {
    //     let Some(window) = window else {
    //         return;
    //     };
    // })
    // .detach();
}

#[derive(IntoElement)]
pub struct SpaceIcon {
    icon_path: Arc<Path>,
    size: Option<AbsoluteLength>,
}

impl SpaceIcon {
    pub fn new(icon_path: impl Into<PathBuf>) -> Self {
        Self {
            icon_path: Arc::from(icon_path.into()),
            size: None,
        }
    }

    pub fn size<L: Into<AbsoluteLength>>(mut self, size: impl Into<Option<L>>) -> Self {
        self.size = size.into().map(Into::into);
        self
    }
}

impl RenderOnce for SpaceIcon {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let image_size = self.size.unwrap_or_else(|| rems(1.).into());

        div()
            //
            .hover(|style| style.opacity(0.6))
            .rounded_lg()
            .child(
                //
                img(self.icon_path.clone())
                    .size(image_size)
                    .max_w_full()
                    .object_fit(ObjectFit::Contain),
            )
    }
}
