use tracing::info;
use zed::unstable::{
    gpui::{self, ClickEvent, Corner, CursorStyle, Entity, FontWeight, Stateful, point, rgba},
    ui::{
        ActiveTheme as _, App, Clickable, Context, ContextMenu, Div, ElementId, FluentBuilder as _,
        InteractiveElement as _, IntoElement, ParentElement as _, PopoverMenu, RenderOnce,
        StatefulInteractiveElement as _, Styled as _, Toggleable, Window, div, h_flex, px,
    },
};

use crate::{
    domain::{profile::Profile, vault::Vault},
    panel::{CreateProfile, GalvanizedPanel, PanelScene},
};

impl GalvanizedPanel {
    pub fn render_profile_bar(
        &mut self,
        user: Entity<Vault>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active_profile = user.read(cx).active_profile();

        h_flex()
            .id("profile-bar")
            .items_center()
            .p_2()
            .gap_2()
            .bg(cx.theme().colors().editor_background)
            .border_t_1()
            .border_color(cx.theme().colors().border)
            .when_none(&active_profile, |el| {
                //
                el.child(render_empty_profile_bar(cx))
            })
            .when_some(active_profile, |el, profile| {
                //
                el.child(render_profile_popover_bar(user, profile, cx))
            })
    }
}

/// Element for bar when there's no active profile
fn render_empty_profile_bar<T: 'static>(cx: &mut Context<T>) -> impl IntoElement {
    div()
        .id("profile-nugget-empty")
        .w_full()
        .p_2()
        .border_2()
        .border_color(cx.theme().colors().border.opacity(0.5))
        .border_dashed()
        .rounded_xl()
        .hover(|style| style.rounded_lg().border_color(cx.theme().colors().border))
        .on_click(cx.listener(|_this, _e, window, cx| {
            window.dispatch_action(Box::new(CreateProfile), cx);
        }))
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().colors().text)
                .child("+ Create Profile"),
        )
}

/// Element for bar when there's an active profile, including popup menu
fn render_profile_popover_bar(
    user: Entity<Vault>,
    profile: Entity<Profile>,
    cx: &mut Context<GalvanizedPanel>,
) -> impl IntoElement {
    let panel = cx.entity();
    PopoverMenu::new("popover")
        .full_width(true)
        .anchor(Corner::BottomLeft)
        .attach(Corner::TopLeft)
        .offset(point(px(0.), px(-4.)))
        .trigger(ProfileNugget::new("profile-nugget", profile))
        .menu({
            move |window, cx| {
                let panel = panel.clone();
                let user = user.clone();
                let profiles = user.read(cx).profiles();
                Some(ContextMenu::build(window, cx, move |menu, _window, _cx| {
                    let mut menu = menu.header("Profiles");
                    for profile in profiles {
                        menu = menu.custom_entry(
                            {
                                let profile = profile.clone();
                                move |_window, cx| {
                                    //
                                    div()
                                        //
                                        .p_2()
                                        .child(profile.read(cx).name())
                                        .into_any_element()
                                }
                            },
                            move |_window, cx| {
                                //
                                info!(name = &**profile.read(cx).name(), "Clicked profile");
                            },
                        );
                    }

                    menu = menu.separator().custom_entry(
                        |_window, cx| {
                            div()
                                .p_2()
                                .text_sm()
                                .text_color(cx.theme().colors().text_accent)
                                .child("+ Create Profile")
                                .into_any_element()
                        },
                        move |_window, cx| {
                            panel.update(cx, |this, _cx| {
                                this.scene = PanelScene::CreatingProfile;
                            });
                        },
                    );

                    menu
                }))
            }
        })
}

/// Clickable profile element that opens the profile selector
#[derive(IntoElement)]
pub struct ProfileNugget {
    base: Stateful<Div>,
    selected: bool,
    profile: Entity<Profile>,
}

impl ProfileNugget {
    pub fn new(id: impl Into<ElementId>, profile: Entity<Profile>) -> Self {
        Self {
            base: h_flex().id(id),
            selected: false,
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
        let profile_name = self.profile.read(cx).name();
        let initial = profile_name.chars().next().unwrap_or('?').to_string();

        self.base
            .flex_grow()
            .p_2()
            .gap_2()
            .rounded_xl()
            .hover(|style| {
                //
                style.bg(cx.theme().colors().ghost_element_hover)
            })
            .active(|style| style.bg(cx.theme().colors().ghost_element_active))
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
    }
}
