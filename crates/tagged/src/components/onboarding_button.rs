use std::path::PathBuf;

use zed::unstable::{
    gpui::{self, ClickEvent, Hsla, Length, img},
    ui::{
        ActiveTheme as _, App, ElementId, FluentBuilder as _, InteractiveElement as _, IntoElement,
        ParentElement as _, RenderOnce, SharedString, StatefulInteractiveElement as _, Styled as _,
        Window, div, h_flex, px, v_flex,
    },
};

#[derive(IntoElement)]
pub struct OnboardingButton {
    id: ElementId,

    border_color: Option<Hsla>,
    border_dashed: bool,
    disabled: bool,
    icon_path: PathBuf,
    icon_size: Option<Length>,
    on_click: Option<Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
    text: SharedString,
}

impl OnboardingButton {
    pub fn new(
        id: impl Into<ElementId>,
        text: impl Into<SharedString>,
        icon_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            id: id.into(),

            border_color: None,
            border_dashed: false,
            disabled: false,
            icon_path: icon_path.into(),
            icon_size: None,
            on_click: None,
            text: text.into(),
        }
    }

    pub fn border_color(mut self, border_color: impl Into<Hsla>) -> Self {
        self.border_color = Some(border_color.into());
        self
    }

    pub fn border_dashed(mut self, border_dashed: bool) -> Self {
        self.border_dashed = border_dashed;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn icon_size(mut self, icon_size: impl Into<Length>) -> Self {
        self.icon_size = Some(icon_size.into());
        self
    }

    pub fn on_click(
        mut self,
        listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(listener));
        self
    }
}

impl RenderOnce for OnboardingButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        v_flex()
            .flex_1()
            .w_full()
            //
            .p_2()
            .child(
                //
                h_flex()
                    .flex_grow()
                    .w_full()
                    //
                    .id(self.id)
                    .border_4()
                    .rounded_2xl()
                    .border_color(
                        self.border_color
                            .unwrap_or_else(|| cx.theme().colors().border_disabled),
                    )
                    .when(self.border_dashed, |el| {
                        //
                        el
                            //
                            .border_dashed()
                    })
                    .when(self.disabled, |el| {
                        //
                        el
                            //
                            .cursor_not_allowed()
                    })
                    .when(!self.disabled, |el| {
                        el
                            //
                            .hover(|style| {
                                style
                                    //
                                    .bg(cx.theme().colors().ghost_element_hover)
                            })
                            .active(|style| {
                                style
                                    //
                                    .bg(cx.theme().colors().ghost_element_active)
                            })
                    })
                    .when_some(self.on_click, |el, on_click| {
                        el
                            //
                            .on_click(move |event, window, cx| {
                                //
                                (on_click)(event, window, cx)
                            })
                    })
                    .child(
                        v_flex()
                            .mx_auto()
                            //
                            .child(
                                //
                                img(self.icon_path)
                                    .mx_auto()
                                    //
                                    .size(self.icon_size.unwrap_or(px(96.).into()))
                                    .rounded_xl(),
                            )
                            .child(
                                //
                                div()
                                    //
                                    .p_2()
                                    .text_center()
                                    .child(self.text),
                            ),
                    ),
            )
    }
}
