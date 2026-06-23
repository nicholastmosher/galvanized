use zed::unstable::{
    gpui::{self, ClickEvent, CursorStyle, FontWeight, Stateful, rgba},
    ui::{
        ActiveTheme as _, App, Clickable, Div, ElementId, InteractiveElement as _, IntoElement,
        ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _,
        Toggleable, Window, div, h_flex, px,
    },
};

/// Clickable profile element that opens the profile selector
#[derive(IntoElement)]
pub struct ProfileNugget {
    base: Stateful<Div>,
    selected: bool,
    initial: SharedString,
    user_name: SharedString,
}

impl ProfileNugget {
    pub fn new(id: impl Into<ElementId>, initial: SharedString, user_name: SharedString) -> Self {
        Self {
            base: h_flex().id(id),
            selected: false,
            initial,
            user_name,
        }
    }
}

impl Clickable for ProfileNugget {
    fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.base = self.base.on_click(handler);
        self
    }

    fn cursor_style(mut self, cursor_style: CursorStyle) -> Self {
        self.base = self.base.cursor(cursor_style);
        self
    }
}

impl Toggleable for ProfileNugget {
    fn toggle_state(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl RenderOnce for ProfileNugget {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.base
            .flex_grow()
            .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
            .active(|style| style.bg(cx.theme().colors().ghost_element_active))
            .p_2()
            .gap_2()
            .rounded_md()
            //
            .child(
                h_flex()
                    .size_10()
                    .rounded_full()
                    .bg(rgba(0xea580cff))
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .mx_auto()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgba(0xffffffff))
                            .child(self.initial),
                    )
                    .child(
                        // Online indicator
                        div()
                            .absolute()
                            .bottom(px(0.))
                            .right(px(0.))
                            .size(px(10.))
                            .rounded_full()
                            .bg(rgba(0x22c55eff))
                            .border_2()
                            .border_color(cx.theme().colors().editor_background),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().colors().text)
                            .child(self.user_name),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().colors().text_muted)
                            .child("Online"),
                    ),
            )
    }
}
