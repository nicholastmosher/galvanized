use zed::unstable::{
    gpui::{Entity, FontWeight},
    ui::{
        ActiveTheme as _, Context, InteractiveElement as _, IntoElement, ParentElement,
        StatefulInteractiveElement as _, Styled as _, Window, div, h_flex, v_flex,
    },
};

use crate::{
    domain::vault::Vault,
    panel::{GZED_ORANGE, GalvanizedPanel, PanelScene, PrimaryButton as _},
};

impl GalvanizedPanel {
    pub fn render_scene_create_profile(
        &mut self,
        user: Entity<Vault>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .id("create-profile-flow")
            .size_full()
            .bg(cx.theme().colors().panel_background)
            .child(
                h_flex()
                    .id("flow-header")
                    .items_center()
                    .gap_3()
                    .p_4()
                    .border_b_1()
                    .border_color(cx.theme().colors().border_variant)
                    .child(
                        div()
                            .id("flow-back-btn")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _e, _window, _cx| {
                                this.scene = PanelScene::Home;
                            }))
                            .child(div().text_sm().text_color(*GZED_ORANGE).child("← Back")),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().colors().text)
                            .child("Create Profile"),
                    ),
            )
            .child(
                div()
                    .id("flow-content")
                    .flex_1()
                    .overflow_y_scroll()
                    .px_5()
                    .py_5()
                    .child(
                        v_flex()
                            .gap_4()
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().colors().text_muted)
                                    .child("Create a new profile within your vault:"),
                            )
                            .child(
                                div()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().colors().text_muted)
                                            .mb_1()
                                            .child("Display Name"),
                                    )
                                    .child(self.vault_name_input.clone()),
                            )
                            .child(
                                div()
                                    .id("flow-create-btn")
                                    .w_full()
                                    .px_4()
                                    .py_2()
                                    .primary_button()
                                    .shadow_lg()
                                    .cursor_pointer()
                                    .on_click(cx.listener(move |this, _e, _window, cx| {
                                        let name = this.vault_name_input.read(cx).text(cx);
                                        if name.is_empty() {
                                            return;
                                        }

                                        user.update(cx, |it, cx| {
                                            it.create_profile(name.into(), cx)
                                        })
                                        .detach_and_log_err(cx);

                                        this.scene = PanelScene::Home;
                                    }))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().colors().text)
                                            .text_center()
                                            .child("Create Profile"),
                                    ),
                            ),
                    ),
            )
    }
}
