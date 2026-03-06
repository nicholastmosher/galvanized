use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use zed::unstable::{
    gpui::{self, Stateful, img},
    ui::{
        AbsoluteLength, ActiveTheme as _, App, Div, ElementId, InteractiveElement, IntoElement,
        ParentElement as _, RenderOnce, StatefulInteractiveElement, Styled, Window, div, rems,
    },
};

pub fn init(_cx: &mut App) {
    // cx.observe_new(|workspace: &mut Workspace, window, cx| {
    //     let Some(window) = window else {
    //         return;
    //     };
    // })
    // .detach();
}

#[derive(IntoElement)]
pub struct SpaceIcon {
    div: Stateful<Div>,
    icon_path: Arc<Path>,
    size: Option<AbsoluteLength>,
}

impl StatefulInteractiveElement for SpaceIcon {}
impl InteractiveElement for SpaceIcon {
    fn interactivity(&mut self) -> &mut gpui::Interactivity {
        self.div.interactivity()
    }
}

impl SpaceIcon {
    pub fn new(id: impl Into<ElementId>, icon_path: impl Into<PathBuf>) -> Self {
        Self {
            div: div().id(id),
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
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let image_size = self.size.unwrap_or_else(|| rems(1.).into());

        self
            //
            .div
            .hover(|style| style.opacity(0.6))
            .active(|style| style.bg(cx.theme().colors().ghost_element_active))
            .rounded_xl()
            .child(
                //
                img(self.icon_path.clone())
                    .size(image_size)
                    .rounded_xl()
                    .max_w_full(),
            )
    }
}
