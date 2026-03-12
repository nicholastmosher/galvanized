use std::{fmt::LowerHex, ops::Not, path::PathBuf, time::Duration};

use hex::ToHex;
use tracing::info;
use willow25::entry::{SubspaceId, SubspaceSecret, randomly_generate_subspace};
use zed::unstable::{
    editor::Editor,
    gpui::{
        Animation, AnimationExt, AppContext, Entity, EventEmitter, FocusHandle, Focusable,
        KeyDownEvent, bounce, ease_in_out, ease_out_quint, img, opaque_grey, quadratic,
    },
    ui::{
        ActiveTheme, AnimationDirection, App, ButtonCommon, Clickable, CommonAnimationExt, Context,
        DefaultAnimations, FluentBuilder as _, Icon, IconButton, IconName, InteractiveElement,
        IntoElement, ListSeparator, ParentElement, Render, SharedString,
        StatefulInteractiveElement, Styled, Tooltip, Window, div, h_flex, px, v_flex,
    },
    util::ResultExt,
    workspace::Item,
};

use crate::{state::profile::ProfileKey, willow::WillowExt as _};

pub fn init(_cx: &mut App) {
    // cx.observe_new(|workspace: &mut Workspace, window, cx| {
    //     let Some(window) = window else {
    //         return;
    //     };
    // })
    // .detach();
}

pub struct OnboardingItem {
    focus_handle: FocusHandle,
    profile_name_editor: Entity<Editor>,
    space_name_editor: Entity<Editor>,
    space_kind: SpaceKind,

    /// Whether the icon should bounce
    create_profile_ready: bool,
    create_profile_key: ProfileKey,
}

enum SpaceKind {
    Owned,
    Communal,
}

impl OnboardingItem {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let profile_name_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("Display name", window, cx);
            editor
        });

        let space_name_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("Space name", window, cx);
            editor.set_text("Home", window, cx);
            editor
        });

        Self {
            //
            focus_handle: cx.focus_handle(),
            profile_name_editor,
            space_kind: SpaceKind::Owned,
            space_name_editor,
            create_profile_ready: false,
            create_profile_key: ProfileKey::new(),
        }
    }
}

impl Render for OnboardingItem {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Trivial: ready when not empty.
        // Todo: ready validation here?
        self.create_profile_ready = self.profile_name_editor.read(cx).text(cx).is_empty().not();

        v_flex()
            .size_full()
            .bg(cx.theme().colors().editor_background)
            .id("onboarding-item")
            //
            .p_4()
            .gap_4()
            .overflow_y_scroll()
            .child(
                div()
                    .bg(cx.theme().colors().panel_background)
                    // .flex_1()
                    .rounded_xl()
                    .shadow_lg()
                    //
                    .p_4()
                    .child(
                        //
                        v_flex()
                            .w_full()
                            //
                            .gap_2()
                            .child(
                                //
                                div()
                                    //
                                    .text_2xl()
                                    .child("Profiles"),
                            )
                            .child(
                                //
                                div()
                                    .mb_2()
                                    //
                                    .text_color(cx.theme().colors().text_muted)
                                    .child("Virtual identities you can use to create and view content"),
                            )
                            .children(
                                cx.willow().profiles(cx).iter().map(|profile| {
                                    //
                                    let profile = profile.read(cx);
                                    v_flex()
                                        .id(SharedString::from(format!("profile-{:x}", profile.id())))
                                        //
                                        .p_4()
                                        .border_2()
                                        .border_color(cx.theme().colors().border_selected)
                                        .rounded_xl()
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
                                        .child(profile.name())
                                        .child(
                                            div()
                                                //
                                                .text_color(cx.theme().colors().text_muted)
                                                .child("Profile ID")
                                        )
                                        .child(
                                            //
                                            div()
                                                //
                                                .w(px(320.))
                                                .child(format!("{:x}", profile.id()))
                                        )
                                })
                            )
                            // Start of Create Profile
                            .child(
                                //
                                h_flex()
                                    //
                                    .border_2()
                                    .border_dashed()
                                    .border_color(cx.theme().colors().border_disabled)
                                    .rounded_xl()
                                    .child(
                                        // Editor
                                        v_flex()
                                            .flex_1()
                                            //
                                            .p_2()
                                            .gap_2()
                                            .child(
                                                //
                                                div()
                                                    //
                                                    .child("Create Profile")
                                            )
                                            .child(
                                                v_flex()
                                                    .p_2()
                                                    .border_1()
                                                    .border_color(cx.theme().colors().border_disabled)
                                                    .rounded_md()
                                                    .child(self.profile_name_editor.clone())
                                                    .child(ListSeparator)
                                                    .child({
                                                        let id = self.create_profile_key.id();
                                                        v_flex()
                                                            .w(px(320.))
                                                            //
                                                            .child(
                                                                div()
                                                                    //
                                                                    .text_color(cx.theme().colors().text_muted)
                                                                    .child("Profile ID")
                                                            )
                                                            .child(
                                                                div()
                                                                    .id("regenerate-profile-key")
                                                                    .rounded_md()
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
                                                                    .tooltip(Tooltip::text("Regenerate Profile ID"))
                                                                    .on_click(cx.listener(|this, _event, window, cx| {
                                                                        this.create_profile_key = ProfileKey::new();
                                                                    }))
                                                                    .child(
                                                                        SharedString::from(format!("{:x}", id))
                                                                    )
                                                            )
                                                    })
                                            )
                                    )
                                    .child(
                                        // Icon
                                        h_flex()
                                            .id("create-profile-submit")
                                            .h_full()
                                            .items_center()
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
                                            .tooltip(Tooltip::text("Create Profile"))
                                            .on_click(cx.listener(|this, _event, window, cx| {
                                                let text = this.profile_name_editor.read(cx).text(cx);
                                                if text.trim().is_empty() {
                                                    return;
                                                }

                                                let _profile = cx.willow().create_profile(text, cx);
                                                this.profile_name_editor.update(cx, |editor, cx| {
                                                    editor.clear(window, cx);
                                                });
                                            }))
                                            .child({
                                                let create_profile_ready = self.create_profile_ready;
                                                img(PathBuf::from(".assets/create-profile.svg"))
                                                    //
                                                    .p_16()
                                                    .size(px(24. * 4.))
                                                    .with_animation("create-profile-bounce", Animation::new(Duration::from_millis(1800)).repeat().with_easing(bounce(quadratic)), move |this, t| {
                                                        if create_profile_ready {
                                                            //
                                                            this
                                                                //
                                                                .bottom(px((t * 6.) - 2.))
                                                        } else {
                                                            this
                                                        }
                                                    })
                                            })
                                    )
                            )
                    ),
            )
            .child(
                div()
                    .bg(cx.theme().colors().panel_background)
                    // .flex_1()
                    .rounded_xl()
                    .shadow_lg()
                    //
                    .p_4()
                    .gap_2()
                    .child(
                        //
                        v_flex()
                            .w_full()
                            //
                            .child(
                                //
                                div()
                                    //
                                    .text_2xl()
                                    .child("Space"),
                            )
                            .child(
                                //
                                div()
                                    //
                                    .text_color(cx.theme().colors().text_muted)
                                    .child("Will this space be public or private?"),
                            )
                            .child(
                                //
                                h_flex()
                                    .flex_wrap()
                                    .p_4()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        //
                                        h_flex()
                                            .id("space-owned")
                                            .w_1_2()
                                            .flex_1()
                                            //
                                            .p_4()
                                            .gap_2()
                                            .rounded_xl()
                                            .map(|el| {
                                                if let SpaceKind::Owned = self.space_kind {
                                                    el
                                                        //
                                                        .border_4()
                                                        .border_color(cx.theme().colors().border_selected)
                                                } else {
                                                    //
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
                                                }
                                            })
                                            .on_click(cx.listener(|this, _e, _window, _cx| {
                                                this.space_kind = SpaceKind::Owned;
                                            }))
                                            .child(
                                                h_flex()
                                                    .bg(opaque_grey(1., 1.))
                                                    //
                                                    .p_2()
                                                    .rounded_2xl()
                                                    .child(
                                                        //
                                                        img(PathBuf::from(
                                                            ".assets/owned-namespace.png",
                                                        ))
                                                        .size(px(48. * 2.)),
                                                    ),
                                            )
                                            .child(
                                                //
                                                v_flex()
                                                    .h_full()
                                                    .w_full()
                                                    //
                                                    .p_2()
                                                    .child(
                                                        //
                                                        div()
                                                            //
                                                            .text_xl()
                                                            .child("Owned Space"),
                                                    )
                                                    .child(
                                                        //
                                                        div()
                                                            .w_full()
                                                            //
                                                            // TODO: I spent way too much time trying and failing to get this to text-wrap
                                                            // - I think there may be a bug near here with h_flex and text wrap
                                                            // .child("Owned spaces by default are private to you. You may share read or write access to areas in your space, but nobody can access your space without explicit permission"),
                                                            .child("Owned spaces by default are private to you"),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        //
                                        h_flex()
                                            .id("space-communal")
                                            .w_1_2()
                                            .flex_1()
                                            //
                                            .p_4()
                                            .gap_2()
                                            .rounded_xl()
                                            .map(|el| {
                                                if let SpaceKind::Communal = self.space_kind {
                                                    el
                                                        //
                                                        .border_4()
                                                        .border_color(cx.theme().colors().border_selected)
                                                } else {
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
                                                }
                                            })
                                            .on_click(cx.listener(|this, _e, _window, _cx| {
                                                this.space_kind = SpaceKind::Communal;
                                            }))
                                            .child(
                                                div()
                                                    .bg(opaque_grey(1., 1.))
                                                    //
                                                    .p_2()
                                                    .rounded_2xl()
                                                    .child(
                                                        //
                                                        img(PathBuf::from(
                                                            ".assets/communal-namespace.png",
                                                        ))
                                                        .size(px(48. * 2.)),
                                                    ),
                                            )
                                            .child(
                                                //
                                                v_flex()
                                                    .h_full()
                                                    .w_full()
                                                    //
                                                    .p_2()
                                                    .child(
                                                        //
                                                        div()
                                                            //
                                                            .text_xl()
                                                            .child("Communal Space"),
                                                    )
                                                    .child(
                                                        //
                                                        div()
                                                            .w_full()
                                                            //
                                                            // .child("Owned spaces by default are private to you. You may share read or write access to areas in your space, but nobody can access your space without explicit permission"),
                                                            .child("Communal spaces are public and can be joined by anyone"),
                                                    ),
                                            ),
                                    ),
                            )
                            .child(
                                //
                                div()
                                    .id("space-name-input")
                                    .mt_2()
                                    //
                                    .p_2()
                                    .when(self.space_name_editor.read(cx).is_focused(window), |el| {
                                        //
                                        el
                                            //
                                            .border_2()
                                            .border_color(cx.theme().colors().border_selected)
                                            .rounded_md()
                                    })
                                    .on_key_down(cx.listener(|this, e: &KeyDownEvent, _window, cx| {
                                        info!(?e, "on_key_down");
                                        let Some("\n") = e.keystroke.key_char.as_deref() else {
                                            return;
                                        };

                                        let profile_name = this.space_name_editor.read(cx).text(cx);
                                        if profile_name.is_empty() {
                                            // Do nothing if empty, any other input valid
                                            return;
                                        }

                                        cx.willow().create_profile(profile_name, cx);

                                        info!("Submit Create Space");
                                        cx.stop_propagation();
                                    }))
                                    .child(
                                        self.space_name_editor.clone()
                                    )
                            )
                    ),
            )
    }
}

impl EventEmitter<()> for OnboardingItem {}
impl Focusable for OnboardingItem {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Item for OnboardingItem {
    type Event = ();

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        "Onboarding".into()
    }
}
