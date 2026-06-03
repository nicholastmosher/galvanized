use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::Context as _;
use image::imageops::FilterType;
use tracing::{info, warn};
use uuid::Uuid;
use zed::unstable::{
    gpui::{
        self, Action, Animation, AnimationExt as _, AppContext as _, Entity, EventEmitter,
        FocusHandle, Focusable, Image, ImageSource, actions, bounce, img, linear_color_stop,
        linear_gradient, quadratic, rgb, rgba, svg, white,
    },
    ui::{
        ActiveTheme, App, Color, Context, ContextMenu, FluentBuilder as _, IconName,
        InteractiveElement as _, IntoElement, ListSeparator, ParentElement as _, Pixels,
        PopoverMenu, Render, SharedString, StatefulInteractiveElement, Styled, Tooltip, Window,
        div, h_flex, px, v_flex,
    },
    ui_input::InputField,
    workspace::{
        Panel, Workspace,
        dock::{DockPosition, PanelEvent},
    },
};

use crate::{
    components::{dropdown::Dropdown, profile_bar::ProfileBar, space_header::SpaceHeader},
    identicon,
    profiles::{Profile, ProfileHandle, ProfilesExt as _},
    views::connections::ConnectionsUi,
};
use plugin_willow::{WillowExt, space::Space};

actions!(
    galvanized,
    [
        //
        TogglePanel,
        FocusConnections,
        FocusDirectMessages,
        FocusSettings,
    ]
);

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, window, cx| {
        let Some(window) = window else {
            return;
        };

        let workspace_entity = cx.entity();
        let connections_ui = cx.new(|cx| ConnectionsUi::new(window, cx));
        let panel = cx.new(|cx| PanelRoot::new(workspace_entity, connections_ui, window, cx));
        workspace.add_panel(panel.clone(), window, cx);
        workspace.focus_panel::<PanelRoot>(window, cx);
        workspace.register_action(|workspace, _: &TogglePanel, window, cx| {
            workspace.toggle_panel_focus::<PanelRoot>(window, cx);
        });
        // .register_action({
        //     let panel = panel.clone();
        //     move |_workspace, _: &FocusConnections, _window, cx| {
        //         panel.update(cx, |panel, _cx| {
        //             //
        //             info!("Focus Connections");
        //             panel.content = PanelContent::Home(HomeContent::Connections);
        //         })
        //     }
        // })
        // .register_action({
        //     let panel = panel.clone();
        //     move |_workspace, _: &FocusDirectMessages, _window, cx| {
        //         panel.update(cx, |panel, _cx| {
        //             //
        //             info!("Focus Direct Messages");
        //             panel.content = PanelContent::Home(HomeContent::DirectMessages);
        //         })
        //     }
        // })
        // .register_action({
        //     let panel = panel.clone();
        //     move |_workspace, _: &FocusSettings, _window, cx| {
        //         panel.update(cx, |panel, _cx| {
        //             //
        //             info!("Focus Settings");
        //             panel.content = PanelContent::Home(HomeContent::Settings);
        //         })
        //     }
        // });
    })
    .detach();
}

pub struct PanelRoot {
    connections_ui: Entity<ConnectionsUi>,
    content: PanelContent,
    focus_handle: FocusHandle,
    width: Option<Pixels>,
    workspace: Entity<Workspace>,

    login_state: LoginState,
    display_name_input: Entity<InputField>,
    create_password_input: Entity<InputField>,
    create_password_confirmation_input: Entity<InputField>,
    login_password_input: Entity<InputField>,
    profile_identicon: Arc<Image>,
    profiles: Vec<Entity<Profile>>,
    active_profile: Option<Entity<Profile>>,
}

pub enum LoginState {
    Picker,
    CreateProfile,
    LoginPrompt(Entity<Profile>),
}

pub enum PanelContent {
    Home(HomeContent),
    Space(Entity<Space>),
}

#[derive(Debug, Clone)]
pub enum HomeContent {
    Connections,
    DirectMessages,
    Settings,
}

impl HomeContent {
    //
    fn title(&self) -> &'static str {
        match self {
            HomeContent::Connections => "Connections",
            HomeContent::DirectMessages => "Direct Messages",
            HomeContent::Settings => "Settings",
        }
    }
}

impl PanelRoot {
    pub fn new(
        workspace: Entity<Workspace>,
        connections_ui: Entity<ConnectionsUi>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let display_name_input = cx.new(|cx| InputField::new(window, cx, "Display name"));
        let create_password_input =
            cx.new(|cx| InputField::new(window, cx, "Create Password").masked(true));
        let create_password_confirmation_input =
            cx.new(|cx| InputField::new(window, cx, "Confirm Password").masked(true));
        let login_password_input =
            cx.new(|cx| InputField::new(window, cx, "Password").masked(true));

        let id = Uuid::new_v4();
        let profile_identicon = identicon(id.as_bytes());

        cx.spawn(async move |this, cx| {
            let profiles = cx
                .profiles()
                .list()
                .await
                .context("failed to list Profiles")?;

            this.update(cx, |this, _cx| {
                this.profiles = profiles;
            })?;

            anyhow::Ok(())
        })
        .detach_and_log_err(cx);

        Self {
            connections_ui,
            content: PanelContent::Home(HomeContent::Settings),
            focus_handle: cx.focus_handle(),
            width: None,
            workspace,

            login_state: LoginState::Picker,
            display_name_input,
            create_password_input,
            create_password_confirmation_input,
            login_password_input,
            profile_identicon: Arc::new(profile_identicon),
            profiles: Default::default(),
            active_profile: Default::default(),
        }
    }
}

const UNLOCK_BG_ORANGE: u32 = 0xff6600ff;
const UNLOCK_BG_DARK: u32 = 0x155dfcff;

impl Render for PanelRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.active_profile {
            None => {
                //
                self.render_login_frame(window, cx).into_any_element()
            }
            Some(profile) => {
                //
                self.render_profile_panel(profile.clone(), window, cx)
                    .into_any_element()
            }
        }
    }
}

// impl Render for PanelRoot {
//     fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
//         div()
//             .h_full()
//             .bg(cx.theme().colors().editor_background)
//             .w(self.width.unwrap_or(px(300.)) - px(1.))
//             .on_action(cx.listener(|this, _: &FocusConnections, window, cx| {
//                 info!("Action: FocusConnections");
//                 this.content = PanelContent::Home(HomeContent::Connections);
//             }))
//             .on_action(cx.listener(|this, _: &FocusDirectMessages, window, cx| {
//                 info!("Action: FocusDirectMessages");
//                 this.content = PanelContent::Home(HomeContent::DirectMessages);
//             }))
//             .on_action(cx.listener(|this, _: &FocusSettings, window, cx| {
//                 info!("Action: FocusSettings");
//                 this.content = PanelContent::Home(HomeContent::Settings);
//             }))
//             .child(self.render_active_panel(window, cx))
//     }
// }

impl PanelRoot {
    fn render_login_frame(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            //
            .size_full()
            .bg(cx.theme().colors().panel_background)
            .p_2()
            .child(
                //
                div()
                    //
                    .w_full()
                    .self_center()
                    .p(px(1.))
                    .rounded_lg()
                    .shadow_lg()
                    .child(
                        //
                        div()
                            //
                            .size_full()
                            // .bg(cx.theme().colors().panel_background)
                            // .p_1()
                            .rounded_lg()
                            .child({
                                match &self.login_state {
                                    LoginState::Picker => {
                                        //
                                        self.render_profile_picker(window, cx).into_any_element()
                                    }
                                    LoginState::LoginPrompt(profile) => {
                                        //
                                        self.render_login_prompt(profile.clone(), window, cx)
                                            .into_any_element()
                                    }
                                    LoginState::CreateProfile => {
                                        //
                                        self.render_create_profile(window, cx).into_any_element()
                                    }
                                }
                            }),
                    )
                    .with_animation(
                        "unlock-bg",
                        Animation::new(Duration::from_secs(120)).repeat(),
                        |el, t| {
                            //
                            el
                                //
                                .bg(linear_gradient(
                                    90. + 360. * t,
                                    linear_color_stop(rgba(UNLOCK_BG_ORANGE), 0.0),
                                    linear_color_stop(rgba(UNLOCK_BG_DARK), 1.0),
                                ))
                        },
                    ),
            )
    }

    fn render_login_prompt(
        &mut self,
        profile: Entity<Profile>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // let id = profile.read(cx).id();
        // let id = Uuid::new_v4();
        // warn!("TODO proper ID");
        let name = profile.read(cx).name();
        // // TODO don't generate every render
        // let image = plot_icon::generate_png(id.as_bytes(), 256).unwrap();
        let image = self.profile_identicon.clone();

        v_flex()
            .size_full()
            // .bg(cx.theme().colors().panel_background)
            //
            .rounded_lg()
            .child(
                h_flex()
                    .bg(cx.theme().colors().panel_background)
                    .rounded_t_lg()
                    .border_b_1()
                    .border_color(cx.theme().colors().border)
                    .child(
                        div()
                            .id("create-profile-back")
                            .flex_shrink()
                            .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                            .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                            .on_click(cx.listener(|this, _e, _window, _cx| {
                                this.login_state = LoginState::Picker;
                            }))
                            //
                            .p_2()
                            .rounded_tl_lg()
                            .border_r_1()
                            .border_color(cx.theme().colors().border)
                            .child(
                                //
                                svg()
                                    //
                                    .path(IconName::ArrowLeft.path())
                                    .size(px(36.))
                                    .text_color(Color::default().color(cx))
                                    .rounded_tl_lg(),
                            ),
                    )
                    .child(
                        //
                        div(),
                    ),
            )
            .child(
                //
                v_flex()
                    .child(
                        h_flex()
                            .id(format!("login-profile-{}", name))
                            .bg(cx.theme().colors().panel_background)
                            // .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                            // .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                            .w_full()
                            //
                            .child(
                                h_flex()
                                    .flex_shrink()
                                    .mx_auto()
                                    //
                                    .p_2()
                                    .gap_2()
                                    .child(
                                        div()
                                            .pl_2()
                                            //
                                            .child(
                                                //
                                                img(ImageSource::Image(image)).size(px(28.)),
                                                // .size(px(32.)),
                                            ),
                                    )
                                    .child(
                                        //
                                        div()
                                            //
                                            .flex_grow()
                                            // .bg(cx.theme().colors().editor_background)
                                            .p_2()
                                            .rounded_lg()
                                            .child(name.clone()),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .id(format!("login-prompt-{}", name))
                            .w_full()
                            .bg(cx.theme().colors().panel_background)
                            //
                            .p_2()
                            .gap_2()
                            .child(self.login_password_input.clone()),
                    )
                    .child(
                        div()
                            //
                            .child(
                                div()
                                    .id("login-button")
                                    .w_full()
                                    .bg(cx.theme().colors().panel_background)
                                    .hover(|style| style.bg(rgba(0x00000000)))
                                    .active(|style| style.bg(rgba(0x00000000)))
                                    .on_click(cx.listener(move |this, _e, window, cx| {
                                        let input = this.login_password_input.clone();
                                        let password = input.read(cx).text(cx);
                                        input.update(cx, |input, cx| input.clear(window, cx));
                                        if password.trim().is_empty() {
                                            return;
                                        }

                                        info!("Login clicked");
                                        let profile = profile.clone();
                                        cx.spawn(async move |_this, cx| {
                                            profile.login(cx, password).await?;
                                            info!("Login succeeded?");
                                            anyhow::Ok(())
                                        })
                                        .detach_and_log_err(cx);
                                    }))
                                    //
                                    .p_2()
                                    .rounded_b_lg()
                                    .child(
                                        //
                                        h_flex()
                                            //
                                            .justify_center()
                                            .child(
                                                //
                                                img(PathBuf::from(".assets/unlock-profile.svg"))
                                                    .flex_shrink_0()
                                                    .size(px(36.)),
                                            )
                                            .child(
                                                //
                                                div()
                                                    //
                                                    .pr_4()
                                                    .text_lg()
                                                    .text_color(white())
                                                    .text_center()
                                                    .child("Login"),
                                            ),
                                    ),
                            ),
                    ),
            )
    }

    fn render_profile_picker(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().colors().panel_background)
            .rounded_lg()
            //
            .children(
                //
                self.profiles
                    //
                    .iter()
                    .enumerate()
                    .map(|(ix, profile)| {
                        // let id = profile.read(cx).id();
                        // let id = Uuid::new_v4();
                        // warn!("TODO proper ID");
                        let name = profile.read(cx).name();
                        let image = self.profile_identicon.clone();
                        // let image = plot_icon::generate_png(id.as_bytes(), 256).unwrap();

                        h_flex()
                            .id(format!("login-profile-{}", name))
                            .w_full()
                            //
                            // .bg(cx.theme().colors().editor_background)
                            .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                            .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                            .on_click(cx.listener({
                                let profile = profile.clone();
                                move |this, _e, _window, _cx| {
                                    this.login_state = LoginState::LoginPrompt(profile.clone());
                                }
                            }))
                            .p_2()
                            .gap_2()
                            .when(ix == 0, |el| el.rounded_t_lg())
                            .child(
                                div()
                                    .pl_2()
                                    //
                                    .child(
                                        //
                                        img(ImageSource::Image(image)).size(px(28.)),
                                        // .size(px(32.)),
                                    ),
                            )
                            .child(
                                //
                                div()
                                    //
                                    .flex_grow()
                                    // .bg(cx.theme().colors().editor_background)
                                    .p_2()
                                    .rounded_lg()
                                    .child(name.clone()),
                            )
                    }),
            )
            .child(
                h_flex()
                    .id("create-profile-button")
                    .w_full()
                    .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                    .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                    .on_click(cx.listener(|this, _e, _window, _cx| {
                        this.login_state = LoginState::CreateProfile;
                    }))
                    //
                    .p_2()
                    .gap_2()
                    .rounded_b_lg()
                    .when(self.profiles.is_empty(), |el| el.rounded_t_lg())
                    .child(
                        div()
                            //
                            .child(
                                //
                                img(PathBuf::from(".assets/create-profile.svg"))
                                    .flex_shrink_0()
                                    .size(px(36.))
                                    .top(px(1.)),
                            ),
                    )
                    .child(
                        //
                        div()
                            //
                            .text_center()
                            .child("Create Profile"),
                    ),
            )
    }

    fn render_create_profile(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .rounded_t_lg()
            .child(
                //
                v_flex()
                    .bg(cx.theme().colors().panel_background)
                    .rounded_t_lg()
                    .child(
                        //
                        h_flex()
                            .w_full()
                            .bg(cx.theme().colors().panel_background)
                            .border_b_1()
                            .border_color(cx.theme().colors().border)
                            .rounded_t_lg()
                            //
                            .child(
                                //
                                div()
                                    .id("create-profile-back")
                                    .hover(|style| {
                                        style.bg(cx.theme().colors().ghost_element_hover)
                                    })
                                    .active(|style| {
                                        style.bg(cx.theme().colors().ghost_element_active)
                                    })
                                    .on_click(cx.listener(|this, _e, _window, _cx| {
                                        this.login_state = LoginState::Picker;
                                    }))
                                    //
                                    // .flex_1()
                                    .flex_shrink()
                                    .p_2()
                                    .rounded_tl_lg()
                                    .border_r_1()
                                    .border_color(cx.theme().colors().border)
                                    .child(
                                        svg()
                                            //
                                            .path(IconName::ArrowLeft.path())
                                            .size(px(36.))
                                            .text_color(Color::default().color(cx))
                                            .mx_auto(),
                                    ),
                            )
                            .child(
                                //
                                div(),
                            ),
                    )
                    .child(
                        v_flex()
                            .p_2()
                            .gap_2()
                            .child(
                                //
                                h_flex()
                                    .w_full()
                                    .bg(cx.theme().colors().panel_background)
                                    //
                                    .gap_2()
                                    .child(
                                        div()
                                            //
                                            .pl_2()
                                            //
                                            .child(
                                                img(ImageSource::Image(
                                                    self.profile_identicon.clone(),
                                                ))
                                                .id("identicon-img")
                                                .flex_shrink_0()
                                                .tooltip(Tooltip::text(
                                                    "Reroll identicon (can't change later)",
                                                ))
                                                .on_click(cx.listener(|this, _e, _window, _cx| {
                                                    let id = Uuid::new_v4();
                                                    let profile_identicon =
                                                        identicon(id.as_bytes());
                                                    this.profile_identicon =
                                                        Arc::new(profile_identicon);
                                                }))
                                                .size(px(28.)),
                                            ),
                                    )
                                    .child(self.display_name_input.clone()),
                            )
                            .child(
                                //
                                div()
                                    //
                                    .w_full()
                                    .bg(cx.theme().colors().panel_background)
                                    .child(self.create_password_input.clone()),
                            )
                            .child(
                                //
                                div()
                                    //
                                    .w_full()
                                    .bg(cx.theme().colors().panel_background)
                                    .child(self.create_password_confirmation_input.clone()),
                            ),
                    ),
            )
            .child(
                div()
                    .id("create-profile-button")
                    .w_full()
                    .bg(cx.theme().colors().panel_background)
                    // .border_color(rgba(UNLOCK_BG_ORANGE))
                    // .hover(|style| style.bg(rgba(UNLOCK_BG_ORANGE)))
                    // .active(|style| style.bg(rgba(UNLOCK_BG_ORANGE)))
                    .hover(|style| style.bg(rgba(0x00000000)))
                    .on_click(cx.listener(|this, _e, _window, cx| {
                        let display_name = this.display_name_input.read(cx).text(cx);
                        if display_name.is_empty() {
                            return;
                        }

                        let password = this.create_password_input.read(cx).text(cx);
                        let password_confirm =
                            this.create_password_confirmation_input.read(cx).text(cx);
                        if password.is_empty() || password_confirm.is_empty() {
                            return;
                        }

                        if password != password_confirm {
                            return;
                        }

                        cx.spawn(async move |this, cx| {
                            let profile = cx.profiles().create(display_name, password).await?;
                            this.update(cx, |this, _cx| {
                                this.profiles.push(profile.clone());
                                this.active_profile = Some(profile);
                            })?;
                            anyhow::Ok(())
                        })
                        .detach_and_log_err(cx);
                    }))
                    //
                    .p_2()
                    .rounded_b_lg()
                    // .border_1()
                    .child(
                        //
                        h_flex()
                            //
                            .justify_center()
                            .child(
                                //
                                img(PathBuf::from(".assets/create-profile.svg"))
                                    .flex_shrink_0()
                                    .size(px(36.)),
                            )
                            .child(
                                //
                                div()
                                    //
                                    .pr_4()
                                    .text_lg()
                                    .text_color(white())
                                    .text_center()
                                    .child("Create Profile"),
                            ),
                    ),
            )
    }

    fn render_profile_panel(
        &mut self,
        profile: Entity<Profile>,
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
                        self.render_panel_content(window, cx),
                    ),
            )
            // Profile bar/selector
            .child(self.render_bottom_bar(profile, window, cx))
    }

    fn render_bottom_bar(
        &mut self,
        profile: Entity<Profile>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .w_full()
            //
            .p_1()
            .child(ProfileBar::new(profile))
        // .map(|el| {
        //     match cx.willow().active_profile() {
        //         None => {
        //             //
        //             el
        //                 // Bottom bar initialization
        //                 .child(
        //                     //
        //                     // self.render_bottom_bar_create_profile(window, cx),
        //                     self.render_bottom_bar_create_profile_button(window, cx),
        //                 )
        //         }
        //         Some(profile) => {
        //             //
        //             el
        //                 //
        //                 .child(
        //                     //
        //                     ProfileBar::new(profile),
        //                 )
        //         }
        //     }
        // })
    }

    fn render_bottom_bar_create_profile_button(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .size_full()
            .bg(cx.theme().colors().panel_background)
            .p_1()
            .rounded_lg()
            .child(
                div()
                    .id("create-profile-bar-button")
                    .w_full()
                    //
                    .p_2()
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
                    .on_click(cx.listener(|this, _e, window, cx| {
                        info!("Clicked Create Profile");
                        // this.workspace.update(cx, |workspace, cx| {
                        //     CreateProfileModal::toggle(workspace, window, cx);
                        // })
                    }))
                    .child(
                        img(PathBuf::from(".assets/create-profile.svg"))
                            .size(px(12. * 4.))
                            .mx_auto()
                            .with_animation(
                                "create-profile-bounce",
                                Animation::new(Duration::from_millis(1800))
                                    .repeat()
                                    .with_easing(bounce(quadratic)),
                                move |this, t| {
                                    this
                                        //
                                        .bottom(px((t * 6.) - 3.))
                                },
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
            .child(
                div()
                    .id("home-icon")
                    .hover(|style| style.opacity(0.6))
                    .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                    .on_click(cx.listener(|this, _e, _window, _cx| {
                        this.content = PanelContent::Home(HomeContent::Connections);
                    }))
                    //
                    .rounded_xl()
                    .child(
                        //
                        div()
                            //
                            .p(px(2.))
                            .rounded_xl()
                            .child(
                                //
                                div()
                                    //
                                    .p(px(1.))
                                    .bg(linear_gradient(
                                        30. + 180.,
                                        //
                                        linear_color_stop(rgba(0x929292ff), -1.5),
                                        // linear_color_stop(rgb(0x000000), 1.0),
                                        linear_color_stop(rgba(0x000000ff), 1.2),
                                    ))
                                    .rounded_xl()
                                    .child(
                                        //
                                        img(PathBuf::from(".assets/galvanized-gz.png"))
                                            .size(px(48.))
                                            .rounded_xl(),
                                    ),
                            )
                            .with_animation(
                                "title-icon-animation",
                                Animation::new(Duration::from_secs(10))
                                    .repeat()
                                    .with_easing(|t| {
                                        // t: [0.0, 1.0]
                                        quadratic(
                                            //
                                            t,
                                        )
                                    }),
                                |el, t| {
                                    //
                                    el
                                        //
                                        .bg(linear_gradient(
                                            30. + 360. * t,
                                            //
                                            linear_color_stop(rgb(0xff6600), 0.0),
                                            // linear_color_stop(rgb(0x000000), 1.0),
                                            linear_color_stop(rgb(0x00002b), 1.0),
                                        ))
                                },
                            ),
                    ),
            )
            .child(ListSeparator)
            .children(cx.willow().spaces().iter().enumerate().map(|(i, space)| {
                div()
                    .id(SharedString::from(format!("space-icon-{i}")))
                    .hover(|style| style.opacity(0.6))
                    .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                    .rounded_lg()
                    // .map(|el| {
                    //     if space.read(cx).is_communal() {
                    //         el.rounded_lg()
                    //     } else {
                    //         el.rounded_full()
                    //     }
                    // })
                    .border_1()
                    .border_color(cx.theme().colors().border)
                    .tooltip(Tooltip::text(space.read(cx).name()))
                    .on_click(cx.listener({
                        let space = space.clone();
                        move |this, _e, _window, cx| {
                            cx.willow().set_active_space(space.clone());
                            this.content = PanelContent::Space(space.clone());
                        }
                    }))
                    .child(
                        //
                        img(space
                            .read(cx)
                            .icon_path()
                            .unwrap_or_else(|| Path::new(&".assets/create-space.svg")))
                        // img(PathBuf::from(".assets/galvanized.png"))
                        .size(px(48.))
                        .rounded_lg(),
                        //
                        // .map(|el| {
                        //     if space.read(cx).is_communal() {
                        //         el.rounded_lg()
                        //     } else {
                        //         el.rounded_full()
                        //     }
                        // }),
                    )
            }))
            .child(div().flex_grow())
            .child({
                // Bounce when empty to prompt user to create a space
                let new_space_bounces = true;
                // cx.willow().active_profile().is_some() && cx.willow().spaces().is_empty();

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
                    .on_click(cx.listener(|this, _e, window, cx| {
                        info!("Clicked create space");
                        // this.workspace.update(cx, |workspace, cx| {
                        //     CreateSpaceModal::toggle(workspace, window, cx);
                        // })
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
    fn render_panel_content(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        match &self.content {
            PanelContent::Home(content) => {
                //
                self.render_home_content(content.clone(), window, cx)
                    .into_any_element()
            }
            PanelContent::Space(space) => {
                //
                self.render_content_space(space.clone(), window, cx)
                    .into_any_element()
            }
        }
    }

    fn render_home_content(
        &mut self,
        content: HomeContent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // v_flex()
        //     .size_full()
        //     //
        //     .p_1()
        //     .child(self.connections_ui.clone())

        let panel = cx.entity();
        let menu = ContextMenu::build(window, cx, |this, window, cx| {
            this.custom_entry(
                |window, cx| {
                    //
                    div()
                        //
                        .p_2()
                        .child("Connections")
                        .into_any_element()
                },
                {
                    let panel = panel.clone();
                    move |window, cx| {
                        //
                        info!("Focus Connections");
                        panel.update(cx, |panel, cx| {
                            //
                            panel.content = PanelContent::Home(HomeContent::Connections);
                        });
                    }
                },
            )
            .custom_entry(
                |window, cx| {
                    //
                    div()
                        //
                        .p_2()
                        .child("Direct Messages")
                        .into_any_element()
                },
                {
                    let panel = panel.clone();
                    move |window, cx| {
                        //
                        info!("Focus Direct Messages");
                        panel.update(cx, |panel, cx| {
                            //
                            panel.content = PanelContent::Home(HomeContent::DirectMessages);
                        });
                    }
                },
            )
            .custom_entry(
                move |window, cx| {
                    //
                    div()
                        //
                        .p_2()
                        .child("Settings")
                        .into_any_element()
                },
                {
                    let panel = panel.clone();
                    move |window, cx| {
                        //
                        info!("Focus Settings");
                        panel.update(cx, |panel, cx| {
                            //
                            panel.content = PanelContent::Home(HomeContent::Settings);
                        });
                    }
                },
            )
        });

        v_flex()
            //
            .debug()
            .size_full()
            .p_2()
            .gap_2()
            .child(
                //
                PopoverMenu::new("home-panel-dropdown")
                    .menu(move |window, cx| {
                        //
                        Some(menu.clone())
                    })
                    .trigger(Dropdown::new(
                        "home-panel-dropdown-trigger",
                        content.title(),
                    )),
            )
            .map(|el| match &content {
                HomeContent::Connections => {
                    el
                        //
                        .child(
                            //
                            self.connections_ui.clone(),
                        )
                }
                HomeContent::DirectMessages => {
                    el
                        //
                        .child(
                            //
                            div()
                                .debug()
                                .size_full()
                                //
                                .p_2()
                                .child("Direct Messages"),
                        )
                }
                HomeContent::Settings => {
                    el
                        //
                        .child(
                            //
                            div()
                                .debug()
                                .size_full()
                                //
                                .p_2()
                                .child("Settings"),
                        )
                }
            })
    }

    fn render_content_space(
        &mut self,
        space: Entity<Space>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // Container, no flex
        v_flex()
            .bg(cx.theme().colors().editor_background)
            //
            .p_2()
            .size_full()
            // Header
            .child(SpaceHeader::new(space))
    }
}

impl EventEmitter<PanelEvent> for PanelRoot {}
impl Focusable for PanelRoot {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for PanelRoot {
    fn persistent_name() -> &'static str {
        "Galvanized"
    }

    fn panel_key() -> &'static str {
        "galvanized"
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
        Some("Galvanized")
    }

    fn toggle_action(&self) -> Box<dyn Action> {
        Box::new(TogglePanel)
    }

    fn activation_priority(&self) -> u32 {
        10
    }
}
