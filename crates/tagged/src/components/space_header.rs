use tracing::info;
use zed::unstable::{
    gpui::{self, Entity, FontWeight, MouseButton},
    ui::{
        ActiveTheme, App, ButtonLike, Clickable, Context, ContextMenu, FluentBuilder as _, Icon,
        IconName, InteractiveElement, IntoElement, ParentElement as _, PopoverMenu, RenderOnce,
        SharedString, StatefulInteractiveElement as _, Styled, Toggleable, Window, div, h_flex,
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

#[derive(IntoElement)]
pub struct SpaceDropdown {
    open: bool,
    on_click: Option<Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>>,
    text: SharedString,
}

impl SpaceDropdown {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            open: false,
            on_click: None,
            text: text.into(),
        }
    }
}

impl Clickable for SpaceDropdown {
    fn on_click(
        mut self,
        handler: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    fn cursor_style(self, _cursor_style: gpui::CursorStyle) -> Self {
        self
    }
}

// impl Disableable for SpaceDropdown {
//     fn disabled(mut self, disabled: bool) -> Self {
//         self
//     }
// }

impl Toggleable for SpaceDropdown {
    fn toggle_state(mut self, selected: bool) -> Self {
        self.open = selected;
        self
    }
}

impl RenderOnce for SpaceDropdown {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            //
            .id("asdfas")
            .p_2()
            .rounded_md()
            .font_weight(FontWeight::BOLD)
            .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
            .when_some(self.on_click, |this, on_click| {
                this.on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                    .on_click(move |event, window, cx| {
                        cx.stop_propagation();
                        (on_click)(event, window, cx)
                    })
            })
            .child(self.text.clone())
            .child(Icon::new(IconName::ChevronDown))
    }
}
