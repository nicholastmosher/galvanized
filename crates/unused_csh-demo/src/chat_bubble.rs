use std::path::PathBuf;

use zed::unstable::{
    component,
    gpui::{AppContext as _, img, rgb},
    ui::{
        ActiveTheme as _, AnyElement, App, Component, ComponentScope, Context, IntoElement,
        ParentElement as _, RegisterComponent, Render, SharedString, Styled as _, Window, div,
        h_flex, px, v_flex,
    },
};

pub fn init(_cx: &mut App) {
    //
}

#[derive(RegisterComponent)]
pub struct ChatBubble {
    //
    display_name: SharedString,
    message: SharedString,
    icon_path: PathBuf,
}

impl ChatBubble {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            display_name: "Alice".into(),
            message: "Hey, are you online?".into(),
            icon_path: PathBuf::from(".assets/galvanized.png"),
        }
    }
}

impl Render for ChatBubble {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            //
            .p_2()
            .gap_2()
            .children(["Alice", "Bob"].into_iter().map(|name| {
                //
                h_flex()
                    .flex_shrink()
                    //
                    .bg(cx.theme().colors().panel_background)
                    .p_4()
                    .gap_4()
                    .border_2()
                    .border_color(rgb(0x00b8db))
                    .rounded_bl_lg()
                    .rounded_br_lg()
                    .rounded_tr_lg()
                    .child(
                        //
                        img(self.icon_path.clone())
                            //
                            .w(px(48.))
                            .rounded_lg(),
                    )
                    .child(
                        v_flex()
                            //
                            .child(
                                //
                                div()
                                    //
                                    .text_lg()
                                    .child(SharedString::from(name)),
                            )
                            .child(
                                //
                                div()
                                    //
                                    .child(self.message.clone()),
                            ),
                    )
            }))
    }
}

impl Component for ChatBubble {
    //
    fn scope() -> ComponentScope {
        ComponentScope::None
    }

    fn preview(_window: &mut Window, cx: &mut App) -> Option<AnyElement> {
        Some(cx.new(|cx| ChatBubble::new(cx)).into_any_element())
    }
}
