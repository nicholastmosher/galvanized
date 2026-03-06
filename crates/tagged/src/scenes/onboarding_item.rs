use std::path::PathBuf;

use tracing::info;
use zed::unstable::{
    editor::Editor,
    gpui::{AppContext, Entity, EventEmitter, FocusHandle, Focusable, img, opaque_grey},
    ui::{
        ActiveTheme, App, Context, InteractiveElement, IntoElement, ParentElement, Render,
        SharedString, StatefulInteractiveElement, Styled, Window, div, h_flex, px, v_flex,
    },
    workspace::Item,
};

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
}

impl OnboardingItem {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let profile_name_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("Display name", window, cx);
            editor
        });

        Self {
            //
            focus_handle: cx.focus_handle(),
            profile_name_editor,
        }
    }
}

impl Render for OnboardingItem {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                            .child(
                                //
                                div()
                                    //
                                    .text_2xl()
                                    .child("Profile"),
                            )
                            .child(
                                //
                                div()
                                    //
                                    .text_color(cx.theme().colors().text_muted)
                                    .child("Pick a profile name and generate a key"),
                            )
                            .child(
                                //
                                div()
                                    .mt_4()
                                    //
                                    .text_xl()
                                    .child("Display name")
                            )
                            .child(
                                //
                                div()
                                    //
                                    .text_color(cx.theme().colors().text_muted)
                                    .child("You can change this later")
                            )
                            .child(
                                //
                                div()
                                    .id("profile-name-input")
                                    //
                                    .p_2()
                                    .on_key_down(cx.listener(|this, e, window, cx| {
                                        info!(?e, "on_key_down");
                                        //
                                    }))
                                    .child(
                                        self.profile_name_editor.clone()
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
                                    .gap_4()
                                    .items_center()
                                    .child(
                                        //
                                        h_flex()
                                            .w_1_2()
                                            .flex_1()
                                            //
                                            .p_2()
                                            .gap_2()
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
                                                        .size(px(48. * 3.)),
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
                                            .w_1_2()
                                            .flex_1()
                                            //
                                            .p_2()
                                            .gap_2()
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
                                                        .size(px(48. * 3.)),
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
                                                            .child("Communal spaces can be joined by anyone"),
                                                    ),
                                            ),
                                    ),
                            ),
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
