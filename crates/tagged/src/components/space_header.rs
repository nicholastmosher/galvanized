use tracing::info;
use zed::unstable::{
    gpui::{self, Entity},
    ui::{
        ActiveTheme, App, ContextMenu, Icon, IconName, InteractiveElement, IntoElement,
        ParentElement as _, PopoverMenu, RenderOnce, Styled, Window, div, h_flex,
    },
};

use crate::{components::space_dropdown::SpaceDropdown, state::space::Space};

pub fn init(_cx: &mut App) {
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
    space: Entity<Space>,
}

impl SpaceHeader {
    pub fn new(space: Entity<Space>) -> Self {
        SpaceHeader { space }
    }
}

impl RenderOnce for SpaceHeader {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let menu = ContextMenu::build(window, cx, |this, _window, _cx| {
            this.custom_entry(
                |window, cx| {
                    //
                    div()
                        .debug()
                        .p_4()
                        .child("Click me if you can")
                        .into_any_element()
                },
                |window, cx| {
                    //
                    info!("Chose custom 1");
                },
            )
            .custom_entry(
                |window, cx| {
                    //
                    div()
                        .debug()
                        .p_4()
                        .child("Click me if you can NUMBER TWO")
                        .into_any_element()
                },
                |window, cx| {
                    //
                    info!("Chose custom 2");
                },
            )
        });

        let popover = PopoverMenu::new("popover-menu")
            .full_width(false)
            .menu(move |_window, _cx| Some(menu.clone()));

        h_flex()
            .child(popover.trigger(SpaceDropdown::new("Group's Space")))
            .child(div().flex_grow())
            .child(
                div()
                    //
                    .p_4()
                    .rounded_md()
                    .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                    .child(Icon::new(IconName::Person)),
            )
    }
}
