use std::path::PathBuf;

use anyhow::bail;
use zed::unstable::{
    gpui::{
        AppContext as _, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
        PathPromptOptions, img,
    },
    menu,
    ui::{
        ActiveTheme, App, Button, Clickable as _, Context, FluentBuilder as _, InteractiveElement,
        IntoElement, KeyBinding, Label, Modal, ModalFooter, ModalHeader, ParentElement as _,
        Render, StatefulInteractiveElement, Styled, StyledExt as _, Window, div, h_flex, px, rems,
        rems_from_px, v_flex,
    },
    ui_input::InputField,
    util::ResultExt as _,
    workspace::{ModalView, Workspace},
};

use plugin_willow::WillowExt as _;

pub struct CreateProfileModal {
    focus_handle: FocusHandle,
    icon_path: Option<PathBuf>,
    input: CreateProfileInput,
}

pub struct CreateProfileInput {
    //
    name: Entity<InputField>,
}

impl CreateProfileModal {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputField::new(window, cx, "Comrade Birb").label("Display name"));
        let input = CreateProfileInput { name };

        Self {
            //
            focus_handle: cx.focus_handle(),
            icon_path: None,
            input,
        }
    }

    pub fn toggle(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
        workspace.toggle_modal(window, cx, |window, cx| Self::new(window, cx));
    }
}

impl Render for CreateProfileModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("create-profile-modal")
            .w(rems(34.))
            //
            .p_2()
            .elevation_3(cx)
            .child(
                //
                Modal::new("create-profile-modal-inner", None)
                    //
                    .header(
                        //
                        ModalHeader::new()
                            //
                            .headline("Create Profile")
                            .description("Profiles are virtual identities you can use to create and view content"),
                    )
                    .child(
                        //
                        v_flex()
                            .w_full()
                            //
                            .gap_2()
                            .child(self.input.name.clone())
                            .child(Label::new("Icon"))
                            .child(
                                //
                                img({
                                    if let Some(path) = &self.icon_path {
                                        path.to_path_buf()
                                    } else {
                                        PathBuf::from(".assets/create-profile.svg")
                                    }
                                })
                                    .id("create-profile-select-icon")
                                    .mx_auto()
                                    //
                                    .size(px(12. * 10.))
                                    .p_2()
                                    .bg(cx.theme().colors().background)
                                    .border_1()
                                    .border_color(cx.theme().colors().border)
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
                                    .on_click(cx.listener(|_this, _e, _window, cx| {
                                        let options =  PathPromptOptions {
                                            files: true,
                                            directories: false,
                                            multiple: false,
                                            prompt: Some("Select profile icon".into()),
                                        };
                                        let rx = cx.prompt_for_paths(options);

                                        // TODO don't dangle
                                        cx.spawn(async move |weak_this, cx| {
                                            let Some(this_entity) = weak_this.upgrade() else {
                                                bail!("Entity released");
                                            };

                                            let selection = rx.await.log_err().transpose().log_err().flatten().flatten();
                                            let Some(paths) = selection else {
                                                bail!("Selected no paths");
                                            };

                                            let [path] = &*paths else {
                                                bail!("Profile icon expected one path");
                                            };

                                            this_entity.update(cx, |this, _cx| {
                                                this.icon_path = Some(path.clone());
                                            });

                                            anyhow::Ok(())
                                        }).detach_and_log_err(cx);
                                    }))
                            )
                    )
                    .footer(
                        //
                        ModalFooter::new()
                            //
                            .end_slot(
                                //
                                h_flex()
                                    //
                                    .w_full()
                                    .gap_1()
                                    .child(
                                        //
                                        Button::new("cancel-create-profile", "Cancel")
                                            .key_binding(
                                                KeyBinding::for_action_in(
                                                    &menu::Cancel,
                                                    &self.focus_handle,
                                                    cx,
                                                )
                                                .map(|kb| kb.size(rems_from_px(12.))),
                                            )
                                            .on_click(cx.listener(|_this, _e, _window, cx| {
                                                // on cancel
                                                cx.emit(DismissEvent);
                                            })),
                                    )
                                    .child(
                                        Button::new("create-profile", "Create Profile")
                                            .key_binding(
                                                KeyBinding::for_action_in(
                                                    &menu::Confirm,
                                                    &self.focus_handle,
                                                    cx,
                                                )
                                                .map(|kb| kb.size(rems_from_px(12.))),
                                            )
                                            .on_click(cx.listener(|this, _e, _window, cx| {
                                                let name = this.input.name.read(cx).text(cx);
                                                if name.trim().is_empty() {
                                                    return;
                                                }

                                                // Create Profile
                                                let profile = cx.willow().create_profile(name, cx);
                                                if let Some(icon) = &this.icon_path {
                                                    profile.update(cx, |profile, _cx| {
                                                        profile.set_avatar(icon);
                                                    });
                                                }

                                                cx.emit(DismissEvent);
                                            })),
                                    ),
                            ),
                    ),
            )
    }
}

impl ModalView for CreateProfileModal {}
impl EventEmitter<DismissEvent> for CreateProfileModal {}
impl Focusable for CreateProfileModal {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
