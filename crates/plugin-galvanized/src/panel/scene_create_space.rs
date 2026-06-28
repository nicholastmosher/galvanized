use zed::unstable::{
    gpui::{FontWeight, linear_color_stop, linear_gradient},
    ui::{
        ActiveTheme as _, Color, Context, FluentBuilder as _, Icon, IconName, IconSize,
        InteractiveElement as _, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
        Styled as _, Window, div, h_flex, px, v_flex,
    },
};

use crate::panel::{
    CreateSpaceKind, GalvanizedPanel, PanelScene, PrimaryButton as _, render_scene_header,
};

impl GalvanizedPanel {
    pub fn render_scene_create_space(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .id("create-space-flow")
            .size_full()
            // .h_full()
            // .w(panel_width)
            .bg(cx.theme().colors().panel_background)
            .child(
                render_scene_header("Create Space".into(), "Spaces may be private or shared with others".into(), cx)
            )
            .child(
                h_flex()
                    .size_full()
                    .p_5()
                    .justify_center()
                    .child(
                        v_flex()
                            .w_full()
                            .gap_4()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().colors().text_muted)
                                    .child("Choose the type of space to create:"),
                            )
                            .child(
                                // Personal Space card
                                div()
                                    .id("flow-space-owned")
                                    .flex()
                                    .items_start()
                                    .gap_3()
                                    .p_3()
                                    .rounded_xl()
                                    .border_2()
                                    .border_color(cx.theme().colors().border)
                                    .when(self.create_space_kind.is_owned(), |el| {
                                        el
                                            .border_2()
                                            .border_color(cx.theme().colors().text_accent.opacity(0.7))
                                    })
                                    .bg(cx.theme().colors().element_background.opacity(0.5))
                                    .cursor_pointer()
                                    .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                                    .on_click(cx.listener(|this, _e, _window, _cx| {
                                        this.create_space_kind = CreateSpaceKind::Owned;
                                    }))
                                    .child(
                                        h_flex()
                                            .size(px(48.))
                                            .rounded_xl()
                                            .bg(linear_gradient(
                                                135.,
                                                linear_color_stop(cx.theme().colors().border, 0.0),
                                                linear_color_stop(cx.theme().colors().panel_background, 1.0),
                                            ))
                                            .flex_shrink_0()
                                            .border_1()
                                            .border_color(cx.theme().colors().border_variant)
                                            .items_center()
                                            .justify_center()
                                            .child(Icon::new(IconName::LockOutlined).size(IconSize::Medium).color(Color::Custom(cx.theme().colors().text_muted))),
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
                                                    .child("Personal Space (Owned)"),
                                            )
                                            .child(div().text_xs().text_color(cx.theme().colors().text_muted).child(
                                                "Private by default. You control access and delegate capabilities.",
                                            )),
                                    ),
                            )
                            .child(
                                // Community Space card
                                div()
                                    .id("flow-space-communal")
                                    .flex()
                                    .items_start()
                                    .gap_3()
                                    .p_3()
                                    .rounded_xl()
                                    .border_2()
                                    .border_color(cx.theme().colors().border)
                                    .when(self.create_space_kind.is_communal(), |el| {
                                        el
                                            .border_2()
                                            .border_color(cx.theme().colors().text_accent.opacity(0.7))
                                    })
                                    .bg(cx.theme().colors().element_background.opacity(0.5))
                                    .cursor_pointer()
                                    .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                                    .on_click(cx.listener(|this, _e, _window, _cx| {
                                        this.create_space_kind = CreateSpaceKind::Communal;
                                    }))
                                    .child(
                                        h_flex()
                                            .size(px(48.))
                                            .rounded_xl()
                                            .bg(linear_gradient(
                                                135.,
                                                linear_color_stop(cx.theme().colors().border, 0.0),
                                                linear_color_stop(cx.theme().colors().panel_background, 1.0),
                                            ))
                                            .flex_shrink_0()
                                            .border_1()
                                            .border_color(cx.theme().colors().border_variant)
                                            .items_center()
                                            .justify_center()
                                            .child(Icon::new(IconName::Person).size(IconSize::Medium).color(Color::Custom(cx.theme().colors().text_muted))),
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
                                                    .child("Community Space (Communal)"),
                                            )
                                            .child(div().text_xs().text_color(cx.theme().colors().text_muted).child(
                                                "Open to anyone. Any subspace can write.",
                                            )),
                                    ),
                            )
                            .child(
                                // Space Name input
                                div()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().colors().text_muted)
                                            .mb_1()
                                            .child("Space Name"),
                                    )
                                    .child(self.space_name_input.clone()),
                            )
                            .child(
                                h_flex()
                                    .id("flow-create-btns")
                                    .w_full()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("flow-back-btn")
                                            .flex_1()
                                            .px_4()
                                            .py_2()
                                            .rounded_lg()
                                            .bg(cx.theme().colors().border_variant)
                                            .cursor_pointer()
                                            .hover(|style| style.bg(cx.theme().colors().border))
                                            .on_click(cx.listener(|this, _e, _window, _cx| {
                                                this.scene = PanelScene::Home;
                                            }))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(cx.theme().colors().text)
                                                    .text_center()
                                                    .child("Back"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("flow-create-action-btn")
                                            .flex_1()
                                            .px_4()
                                            .py_2()
                                            .primary_button()
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _e, _window, cx| {
                                                let name = this.space_name_input.read(cx).text(cx);
                                                let kind = this.create_space_kind;
                                                this.create_space_kind = Default::default();
                                                if !name.is_empty() {
                                                    this.scene = PanelScene::Home;
                                                }

                                                let Some(user) = this.galvanized.read(cx).active_vault.clone() else {
                                                    return;
                                                };

                                                user.update(cx, |it, cx| {
                                                    if kind.is_owned() {
                                                        it.create_owned_space(name.into(), cx)
                                                    } else {
                                                        it.create_communal_space(name.into(), cx)
                                                    }
                                                }).detach_and_log_err(cx);
                                            }))
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(cx.theme().colors().text)
                                                    .text_center()
                                                    .child("Create Space"),
                                            ),
                                    ),
                            )
                        //
                    ),
            )
    }
}
