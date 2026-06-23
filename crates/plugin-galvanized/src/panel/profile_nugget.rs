use zed::unstable::{
    gpui::{self, ClickEvent, CursorStyle, Entity, FontWeight, Stateful, rgba},
    ui::{
        ActiveTheme as _, App, Clickable, Div, ElementId, FluentBuilder, InteractiveElement as _,
        IntoElement, ParentElement as _, RenderOnce, StatefulInteractiveElement as _, Styled as _,
        Toggleable, Window, div, h_flex, px,
    },
};

use crate::{panel::PanelRoot, users::Profile};

use super::PanelScene;

/// Clickable profile element that opens the profile selector
#[derive(IntoElement)]
pub struct ProfileNugget {
    base: Stateful<Div>,
    selected: bool,
    panel: Entity<PanelRoot>,
    profile: Option<Entity<Profile>>,
}

impl ProfileNugget {
    pub fn new(
        id: impl Into<ElementId>,
        panel: Entity<PanelRoot>,
        profile: Option<Entity<Profile>>,
    ) -> Self {
        Self {
            base: h_flex().id(id),
            selected: false,
            panel,
            profile,
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
        let panel = self.panel.clone();
        self.base
            .flex_grow()
            .p_2()
            .gap_2()
            .border_2()
            .border_dashed()
            .border_color(cx.theme().colors().border.opacity(0.5))
            .rounded_xl()
            .hover(|style| {
                //
                style
                    //
                    // .bg(cx.theme().colors().ghost_element_hover)
                    .rounded_lg()
                    .border_color(cx.theme().colors().border)
            })
            .active(|style| style.bg(cx.theme().colors().ghost_element_active))
            //
            .when_none(&self.profile, |el| {
                el
                    //
                    .child(
                        div()
                            .id("profile-nugget-empty")
                            .p_2()
                            .on_click(move |_e, _window, cx| {
                                panel.update(cx, |panel, _cx| {
                                    panel.scene = PanelScene::CreatingProfile;
                                });
                            })
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().colors().text)
                                    .child("+ Create Profile"),
                            ),
                    )
            })
            .when_some(self.profile.as_ref(), |el, profile| {
                let profile_name = profile.read(cx).name();
                let initial = profile_name.chars().next().unwrap_or('?').to_string();

                el.child(
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
                                .child(initial),
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
                                .child(profile_name),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().colors().text_muted)
                                .child("Online"),
                        ),
                )
            })
    }
}
