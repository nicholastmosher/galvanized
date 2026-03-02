use zed::unstable::{
    gpui::{self, FontWeight, MouseButton},
    ui::{
        ActiveTheme as _, App, Clickable, FluentBuilder as _, Icon, IconName,
        InteractiveElement as _, IntoElement, ParentElement as _, RenderOnce, SharedString,
        StatefulInteractiveElement as _, Styled as _, Toggleable, Window, h_flex,
    },
};

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
