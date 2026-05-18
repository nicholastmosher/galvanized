use std::path::PathBuf;

use tracing::info;
use zed::unstable::{
    gpui::{self, AppContext, Entity, EventEmitter, FocusHandle, Focusable, actions, img},
    ui::{
        ActiveTheme as _, App, Context, InteractiveElement, IntoElement, ParentElement, Render,
        SharedString, StatefulInteractiveElement, Styled, Window, div, h_flex, px, v_flex,
    },
    ui_input::InputField,
    workspace::{Item, Workspace},
};

actions!(galvanized, [OpenProfile]);

pub fn init(cx: &mut App) {
    //

    cx.observe_new::<Workspace>(move |workspace, window, cx| {
        let Some(window) = window else { return };
        let profile_item = cx.new(|cx| CreateProfileItem::new(window, cx));

        workspace.add_item_to_active_pane(
            Box::new(profile_item.clone()),
            Some(0),
            false,
            window,
            cx,
        );

        workspace.register_action(move |workspace, _: &OpenProfile, window, cx| {
            workspace.add_item_to_active_pane(
                Box::new(profile_item.clone()),
                Some(0),
                false,
                window,
                cx,
            );
        });
    })
    .detach();
}

pub struct CreateProfileItem {
    //
    focus_handle: FocusHandle,
    name_input: Entity<InputField>,
    icon_path: Option<PathBuf>,
}

impl CreateProfileItem {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name_input = cx.new(|cx| InputField::new(window, cx, "Profile name"));
        Self {
            focus_handle: cx.focus_handle(),
            name_input,
            icon_path: None,
        }
    }
}

impl Render for CreateProfileItem {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .size_full()
            .bg(cx.theme().colors().editor_background)
            .child(
                //
                div()
                    .size_full()
                    //
                    .p_4()
                    .child(
                        //
                        h_flex()
                            .size_full()
                            //
                            .bg(cx.theme().colors().panel_background)
                            .border_1()
                            .border_color(cx.theme().colors().border)
                            .rounded_lg()
                            .child(
                                //
                                div()
                                    //
                                    .h_full()
                                    .w_40()
                                    .border_r_1()
                                    .border_color(cx.theme().colors().border),
                            )
                            .child(
                                v_flex()
                                    .self_start()
                                    //
                                    .p_2()
                                    .gap_2()
                                    .child(
                                        // Profile icon container
                                        div()
                                            .id("profile-icon")
                                            .size_40()
                                            //
                                            .p_2()
                                            .bg(cx.theme().colors().editor_background)
                                            .rounded_lg()
                                            .border_1()
                                            .border_color(cx.theme().colors().border)
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
                                            .on_click(cx.listener(|this, e, window, cx| {
                                                info!("Clicked create profile");
                                                //
                                            }))
                                            .child(
                                                //
                                                img(PathBuf::from(".assets/create-profile.svg"))
                                                    //
                                                    .size_full()
                                                    .debug(),
                                            ),
                                    )
                                    .child(
                                        //
                                        div()
                                            //
                                            .child(
                                                //
                                                self.name_input.clone(),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}

impl Focusable for CreateProfileItem {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
impl EventEmitter<()> for CreateProfileItem {}
impl Item for CreateProfileItem {
    type Event = ();

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        "Create Profile".into()
    }
}
