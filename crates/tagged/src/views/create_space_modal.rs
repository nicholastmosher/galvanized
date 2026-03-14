use std::{collections::HashMap, path::PathBuf};

use zed::unstable::{
    gpui::{
        AppContext as _, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable, img,
        opaque_grey,
    },
    menu::{self, Cancel},
    ui::{
        ActiveTheme as _, App, Button, Checkbox, Clickable as _, Context, FluentBuilder as _, Icon,
        IconName, InteractiveElement as _, IntoElement, KeyBinding, Label, LabelCommon as _,
        LabelSize, Modal, ModalFooter, ModalHeader, ParentElement as _, Render,
        StatefulInteractiveElement as _, Styled as _, StyledExt, ToggleState, Tooltip, Window, div,
        h_flex, px, rems_from_px, v_flex,
    },
    ui_input::InputField,
    workspace::{ModalView, Workspace},
};

use crate::{state::profile::Profile, willow::WillowExt as _};

pub struct CreateSpaceModal {
    focus_handle: FocusHandle,
    input: CreateSpaceInput,
}

pub struct CreateSpaceInput {
    space_name: Entity<InputField>,
    space_kind: SpaceKind,

    profile_toggle_states: HashMap<Entity<Profile>, ToggleState>,
}

enum SpaceKind {
    Owned,
    Communal,
}

impl CreateSpaceModal {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let space_name = cx
            .new(|cx| InputField::new(window, cx, "Personal, Family, Work...").label("Space name"));
        let input = CreateSpaceInput {
            //
            space_name,
            space_kind: SpaceKind::Owned,
            profile_toggle_states: Default::default(),
        };

        Self {
            //
            focus_handle: cx.focus_handle(),
            input,
        }
    }

    pub fn toggle(workspace: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
        workspace.toggle_modal(window, cx, |window, cx| Self::new(window, cx));
    }
}

impl Render for CreateSpaceModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_handle = self.focus_handle(cx);

        div()
            .id("create-space-modal")
            .w_full()
            //
            .elevation_3(cx)
            .child(
                //
                Modal::new("create-space-modal-inner", None)
                    .header(ModalHeader::new().headline("Create Space"))
                    .child(
                        //
                        v_flex()
                            .p_2()
                            .gap_2()
                            // .items_center()
                            .child(self.input.space_name.clone())
                                    // .size(LabelSize::Small)
                                    // .color(if self.disabled {
                                    //     Color::Disabled
                                    // } else {
                                    //     Color::Default
                                    // }),
                            .child(
                                Label::new("Space kind").size(LabelSize::Small)
                            )
                            .child(
                                //
                                h_flex()
                                    .id("space-owned")
                                    .w_full()
                                    // .w_1_2()
                                    .flex_1()
                                    //
                                    .p_4()
                                    .gap_2()
                                    .border_4()
                                    .rounded_xl()
                                    .map(|el| {
                                        if let SpaceKind::Owned = self.input.space_kind {
                                            el
                                                //
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
                                        this.input.space_kind = SpaceKind::Owned;
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
                                    .w_full()
                                    // .w_1_2()
                                    // .flex_1()
                                    //
                                    .p_4()
                                    .gap_2()
                                    .border_4()
                                    .rounded_xl()
                                    .map(|el| {
                                        if let SpaceKind::Communal = self.input.space_kind {
                                            el
                                                //
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
                                        this.input.space_kind = SpaceKind::Communal;
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
                            )
                            .child(
                                h_flex()
                                    .child(
                                        //
                                        Label::new("Root Profiles").size(LabelSize::Small)
                                    )
                                    .child(
                                        //
                                        div()
                                            .id("create-space-admin-profiles-info")
                                            .tooltip(Tooltip::text("Profiles to be given full access to the Space"))
                                            .cursor_pointer()
                                            .child(Icon::new(IconName::Info))
                                    )
                            )
                            .child(
                                //
                                v_flex()
                                    .p_2()
                                    .gap_2()
                                    .rounded_sm()
                                    .border_1()
                                    .border_dashed()
                                    .border_color(cx.theme().colors().border.opacity(0.6))
                                    .bg(cx.theme().colors().element_active.opacity(0.15))
                                    .children(cx.willow().profiles(cx).into_iter().enumerate().map(|(i, profile)| {
                                        let toggle_state = self.input.profile_toggle_states.entry(profile.clone()).or_insert_with(|| {
                                            if i == 0 {
                                                ToggleState::Selected
                                            } else {
                                                ToggleState::Unselected
                                            }
                                        });
                                        Checkbox::new(("space-add-profile-admin", i), *toggle_state)
                                            //
                                            .label(profile.read(cx).name())
                                            .on_click(cx.listener(move |this, e, window, cx| {
                                            let profile = profile.clone();
                                            let Some(toggle_state) = this.input.profile_toggle_states.get_mut(&profile) else {
                                                return;
                                            };
                                            *toggle_state = toggle_state.inverse();
                                        }))
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
                                        Button::new("cancel-create-space", "Cancel")
                                            .key_binding(
                                                KeyBinding::for_action_in(
                                                    &menu::Cancel,
                                                    &focus_handle,
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
                                        Button::new("create-space", "Create Space")
                                            .key_binding(
                                                KeyBinding::for_action_in(
                                                    &menu::Confirm,
                                                    &focus_handle,
                                                    cx,
                                                )
                                                .map(|kb| kb.size(rems_from_px(12.))),
                                            )
                                            .on_click(cx.listener(|this, _e, _window, cx| {
                                                let name = this.input.space_name.read(cx).text(cx);
                                                if name.trim().is_empty() {
                                                    return;
                                                }

                                                match this.input.space_kind {
                                                    SpaceKind::Owned => {
                                                        cx.willow().create_owned_space(name, cx);
                                                    },
                                                    SpaceKind::Communal => {
                                                        cx.willow().create_communal_space(name, cx);
                                                    },
                                                }

                                                cx.emit(DismissEvent);
                                            })),
                                    ),
                            ),
                    ),
            )
    }
}

impl ModalView for CreateSpaceModal {}
impl EventEmitter<DismissEvent> for CreateSpaceModal {}
impl Focusable for CreateSpaceModal {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
