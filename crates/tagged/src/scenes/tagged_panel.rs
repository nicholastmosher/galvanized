use std::{ops::Not as _, path::PathBuf, time::Duration};

use tracing::info;
use willow25::entry::randomly_generate_subspace;
use zed::unstable::{
    editor::Editor,
    gpui::{
        self, Action, Animation, AnimationExt as _, AppContext as _, Entity, EventEmitter,
        FocusHandle, Focusable, KeyDownEvent, actions, bounce, img, quadratic,
    },
    ui::{
        ActiveTheme, App, Context, FluentBuilder as _, Icon, IconName, IconSize,
        InteractiveElement as _, IntoElement, ListSeparator, ParentElement as _, Pixels, Render,
        SharedString, StatefulInteractiveElement, Styled, Tooltip, Window, div, h_flex, px, v_flex,
    },
    workspace::{
        Panel, Workspace,
        dock::{DockPosition, PanelEvent},
    },
};

use crate::{
    components::{
        onboarding_button::OnboardingButton, profile_bar::ProfileBar, space_header::SpaceHeader,
        space_icon::SpaceIcon,
    },
    state::{
        onboarding::Onboarding,
        profile::{Profile, ProfileKey},
        space::Space,
    },
    willow::WillowExt as _,
};

actions!(workspace, [ToggleTaggedPanel]);

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, window, cx| {
        let Some(window) = window else {
            return;
        };

        let workspace_entity = cx.entity();
        let tagged_panel = cx.new(|cx| TaggedPanel::new(workspace_entity, window, cx));
        workspace.add_panel(tagged_panel, window, cx);
        workspace.focus_panel::<TaggedPanel>(window, cx);
        workspace.register_action(|workspace, _: &ToggleTaggedPanel, window, cx| {
            workspace.toggle_panel_focus::<TaggedPanel>(window, cx);
        });
    })
    .detach();
}

pub struct TaggedPanel {
    active_profile: Option<Entity<Profile>>,
    active_space: Option<Entity<Space>>,
    focus_handle: FocusHandle,
    onboarding: Entity<Onboarding>,
    width: Option<Pixels>,
    workspace: Entity<Workspace>,

    // temp
    demo_profile: Entity<Profile>,
    initial_panel: bool,
    create_profile_editor: Entity<Editor>,
    bottom_bar_height: Pixels,
    create_profile_key: ProfileKey,
}

impl TaggedPanel {
    pub fn new(workspace: Entity<Workspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let active_space = cx.willow().create_owned_space("Group's Space", cx);
        // let communal = active_space.read(cx).is_communal();
        // let active_space = cx.new(|cx| Space::new("Group's Space", cx));
        let onboarding = cx.new(|cx| Onboarding::new(workspace.clone(), cx));

        let demo_profile = cx.new(|cx| {
            //
            let mut csprng = rand_core_0_6_4::OsRng;
            let (_demo_id, demo_secret) = randomly_generate_subspace(&mut csprng);
            Profile::new("Myselfandi", demo_secret, cx).with_avatar(".assets/tagged.svg")
        });

        Self {
            //
            active_profile: None,
            // active_profile: Some(demo_profile.clone()),
            active_space: None,
            // active_space: Some(active_space),
            focus_handle: cx.focus_handle(),
            onboarding,
            width: None,
            workspace,

            // temp
            demo_profile,
            initial_panel: true,
            create_profile_editor: cx.new(|cx| {
                let mut editor = Editor::single_line(window, cx);
                editor.set_placeholder_text("Display name", window, cx);
                editor
            }),
            bottom_bar_height: px(48.),
            create_profile_key: ProfileKey::new(),
        }
    }
}

impl Render for TaggedPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h_full()
            .bg(cx.theme().colors().editor_background)
            .w(self.width.unwrap_or(px(300.)) - px(1.))
            // .when(self.initial_panel, |el| {
            //     el
            //         //
            //         .child(
            //             //
            //             self.render_initial_panel(window, cx),
            //         )
            // })
            // .when(!self.initial_panel, |el| {
            //     //
            //     el
            //         //
            //         .child(self.render_active_panel(window, cx))
            // })
            .child(self.render_active_panel(window, cx))
    }
}

impl TaggedPanel {
    fn render_initial_panel(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Full panel body is a vertical flex
        v_flex()
            .id("tagged-panel")
            .size_full()
            //
            .p_2()
            // .py_20()
            // Create Profile title
            .overflow_y_scroll()
            .child(
                //
                div()
                    //
                    .p_2()
                    .child(
                        div()
                            //
                            .text_lg()
                            .child("Welcome!"),
                    )
                    .child(
                        //
                        div()
                            //
                            .text_sm()
                            .text_color(cx.theme().colors().text_muted)
                            .child("Let's get you started"),
                    ),
            )
            // Create Profile
            .child(
                //
                OnboardingButton::new(
                    "create-profile",
                    "Create a Profile",
                    ".assets/create-profile.svg",
                )
                .when(self.active_profile.is_some(), |el| {
                    el
                        //
                        .border_color(cx.theme().colors().border_selected)
                })
                .when(self.active_profile.is_none(), |el| {
                    el
                        //
                        .border_dashed(true)
                })
                .on_click({
                    let onboarding = self.onboarding.downgrade();
                    move |_e, window, cx| {
                        let Some(onboarding) = onboarding.upgrade() else {
                            return;
                        };

                        onboarding.update(cx, |onboarding, cx| {
                            onboarding.open_tab(window, cx);
                            //
                        });
                    }
                }),
            )
            // Create Space
            .child(
                //
                OnboardingButton::new("create-space", "Create a Space", ".assets/create-space.svg")
                    .border_color(cx.theme().colors().border_selected)
                    .disabled(true)
                    .border_dashed(true),
            )
            // Next steps
            .child(
                //
                OnboardingButton::new(
                    "connect-peers",
                    "Connect with Peers",
                    ".assets/connect-peers.svg",
                )
                .border_color(cx.theme().colors().border_disabled)
                .border_dashed(true)
                .on_click(cx.listener(|this, _e, _window, _cx| {
                    this.initial_panel = !this.initial_panel;
                })),
            )
    }

    fn render_active_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .h_full()
            .w(self.width.unwrap_or(px(300.)) - px(1.))
            // Profile space?
            .gap_1()
            .child(
                h_flex()
                    .h_full()
                    .flex_grow()
                    // Spaces bar
                    .child(
                        //
                        self.render_spaces_column(window, cx),
                    )
                    .child(
                        div()
                            .h_full()
                            .w_0()
                            .mt_2()
                            .border_1()
                            .border_color(cx.theme().colors().border),
                    )
                    // Active space content
                    .child(
                        //
                        self.render_active_space(window, cx),
                    ),
            )
            // Profile bar/selector
            .child(self.render_bottom_bar(window, cx))
    }

    fn render_bottom_bar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .w_full()
            //
            .p_1()
            .map(|el| {
                match &self.active_profile {
                    None => {
                        //
                        el
                            // Bottom bar initialization
                            .child(
                                //
                                self.render_bottom_bar_create_profile(window, cx),
                            )
                    }
                    Some(profile) => {
                        //
                        el
                            //
                            .child(
                                //
                                ProfileBar::new(profile.clone()),
                            )
                    }
                }
            })
    }

    fn render_bottom_bar_create_profile(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let display_name_empty = self.create_profile_editor.read(cx).text(cx).is_empty();
        h_flex()
            .size_full()
            .bg(cx.theme().colors().panel_background)
            // Floating heart-plus
            .gap_2()
            .p_1()
            .rounded_lg()
            .child(
                //
                div()
                    .flex_shrink_0()
                    //
                    .id("create-profile-icon-button")
                    .rounded_lg()
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
                        let name = this.create_profile_editor.read(cx).text(cx);
                        if name.is_empty() {
                            return;
                        }

                        let profile = cx.willow().create_profile(name, cx);
                        this.active_profile = Some(profile);
                    }))
                    .on_key_down(cx.listener(|this, e: &KeyDownEvent, _window, cx| {
                        info!(?e, "on_key_down");
                        let Some("\n") = e.keystroke.key_char.as_deref() else {
                            return;
                        };

                        let name = this.create_profile_editor.read(cx).text(cx);
                        if name.is_empty() {
                            return;
                        }

                        let profile = cx.willow().create_profile(name, cx);
                        this.active_profile = Some(profile);
                    }))
                    .child(
                        img(PathBuf::from(".assets/create-profile.svg"))
                            .size(px(12. * 5.))
                            .with_animation(
                                "create-profile-bounce",
                                Animation::new(Duration::from_millis(1800))
                                    .repeat()
                                    .with_easing(bounce(quadratic)),
                                move |this, t| {
                                    if !display_name_empty {
                                        //
                                        this
                                            //
                                            .bottom(px((t * 6.) - 3.))
                                    } else {
                                        this
                                    }
                                },
                            ),
                    ),
            )
            .child(
                //
                v_flex()
                    .flex_grow()
                    //
                    .child(
                        //
                        div()
                            //
                            .p_2()
                            .border_b_1()
                            .map(|el| {
                                if self.create_profile_editor.read(cx).is_focused(window) {
                                    el
                                        //
                                        .border_color(cx.theme().colors().border_selected)
                                } else {
                                    el
                                }
                            })
                            .child(self.create_profile_editor.clone())
                            .child(
                                //
                                div()
                                    //
                                    .absolute()
                                    .bottom_12()
                                    .child(
                                        //
                                        img(PathBuf::from(".assets/arrow-down.png"))
                                            .size(px(48.))
                                            .with_animation(
                                                "display-name-arrow",
                                                Animation::new(Duration::from_millis(2_000))
                                                    .repeat()
                                                    .with_easing(bounce(quadratic)),
                                                move |el, t| {
                                                    if display_name_empty {
                                                        el.mb(px(0. - (t * 6.)))
                                                    } else {
                                                        //
                                                        el
                                                    }
                                                },
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            //
                            .p_2()
                            .child(
                                //
                                h_flex()
                                    //
                                    // .text_color(cx.theme().colors().text_muted)
                                    .gap_1()
                                    .child(
                                        img(PathBuf::from(".assets/refresh-arrows.png"))
                                            .id("regenerate-profile-key")
                                            .justify_center()
                                            //
                                            .p_1()
                                            .size(px(24.))
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
                                            })),
                                    )
                                    .child("ID: ")
                                    .child(
                                        //
                                        div()
                                            //
                                            .text_sm()
                                            .child({
                                                let mut id_hex =
                                                    format!("{:x}", self.create_profile_key.id());
                                                let lsbs = id_hex.split_off(id_hex.len() - 8);
                                                SharedString::from(format!(".+{lsbs}"))
                                            }),
                                    ),
                            ),
                    ),
            )
    }

    fn render_spaces_column(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .id("spaces-column")
            .h_full()
            .pt_2()
            .px_2()
            .gap_1()
            .overflow_y_scroll()
            .child(SpaceIcon::new("space-icon-1", ".assets/tagged.svg").size(px(48.)))
            .child(ListSeparator)
            .children(cx.willow().spaces(cx).iter().enumerate().map(|(i, space)| {
                // TODO real icon properties
                SpaceIcon::new(
                    SharedString::from(format!("space-icon-{i}")),
                    ".assets/tagged.svg",
                )
                .size(px(48.))
                .tooltip(Tooltip::text(format!("Space {i}")))
            }))
            .child(div().flex_grow())
            .child({
                let new_space_bounces =
                    self.active_profile.is_some() && self.active_space.is_none();
                div()
                    //
                    .id("create-space")
                    .bg(cx.theme().colors().panel_background)
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
                    .on_click(cx.listener(|this, _e, _window, _cx| {
                        this.initial_panel = !this.initial_panel;
                    }))
                    .child(
                        img(PathBuf::from(".assets/create-space.svg"))
                            .size(px(48.))
                            .tooltip(Tooltip::text("Create Space")),
                    )
                    .with_animation(
                        "create-space-animation",
                        Animation::new(Duration::from_millis(1800))
                            .repeat()
                            .with_easing(bounce(quadratic)),
                        move |el, t| {
                            if new_space_bounces {
                                el
                                    //
                                    .bottom(px((t * 6.) - 0.))
                            } else {
                                //
                                el
                            }
                        },
                    )
            })
    }

    /// The area above the Profiles bar and right of the Spaces bar
    fn render_active_space(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Container, no flex
        v_flex()
            // .debug()
            .bg(cx.theme().colors().editor_background)
            //
            .p_2()
            .size_full()
        // .when(self.active_space.is_none(), |el| {
        //     //
        //     el
        //         //
        //         .child(
        //             //
        //             self.render_create_space(window, cx),
        //         )
        // })
        // .child(SpaceHeader::new(self.active_space.clone()))
        // .child(ListSeparator)
    }

    fn render_create_space(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            //
            .child(div().flex_grow())
            .child(
                //
                img(PathBuf::from(IconName::ArrowLeft.path().to_string())).with_animation(
                    "create-space-bounce",
                    Animation::new(Duration::from_millis(1800))
                        .repeat()
                        .with_easing(bounce(quadratic)),
                    move |this, t| {
                        if true {
                            //
                            this
                                //
                                .bottom(px((t * 6.) - 3.))
                        } else {
                            this
                        }
                    },
                ),
            )
    }
}

impl EventEmitter<PanelEvent> for TaggedPanel {}
impl Focusable for TaggedPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for TaggedPanel {
    fn persistent_name() -> &'static str {
        "TaggedPanel"
    }

    fn panel_key() -> &'static str {
        "tagged-panel"
    }

    fn position(&self, _window: &Window, _cx: &App) -> DockPosition {
        DockPosition::Left
    }

    fn position_is_valid(&self, _position: DockPosition) -> bool {
        true
    }

    fn set_position(
        &mut self,
        _position: DockPosition,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    fn size(&self, _window: &Window, _cx: &App) -> Pixels {
        self.width.unwrap_or(px(300.))
    }

    fn set_size(&mut self, size: Option<Pixels>, _window: &mut Window, _cx: &mut Context<Self>) {
        self.width = size;
    }

    fn icon(&self, _window: &Window, _cx: &App) -> Option<IconName> {
        Some(IconName::Hash)
    }

    fn icon_tooltip(&self, _window: &Window, _cx: &App) -> Option<&'static str> {
        Some("Tagged")
    }

    fn toggle_action(&self) -> Box<dyn Action> {
        Box::new(ToggleTaggedPanel)
    }

    fn activation_priority(&self) -> u32 {
        0
    }
}
