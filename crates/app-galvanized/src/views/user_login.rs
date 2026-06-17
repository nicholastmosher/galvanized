use std::{path::PathBuf, sync::Arc, time::Duration};

use tracing::info;
use uuid::Uuid;
use zed::unstable::{
    gpui::{
        Animation, AnimationExt as _, Entity, ImageSource, img, linear_color_stop, linear_gradient,
        rgba, svg, white,
    },
    ui::{
        ActiveTheme as _, Color, Context, FluentBuilder as _, IconName, InteractiveElement as _,
        IntoElement, ParentElement as _, StatefulInteractiveElement as _, Styled as _, Tooltip,
        Window, div, h_flex, px, v_flex,
    },
};

use crate::{
    identicon,
    panel::{LoginState, PanelRoot},
    users::{User, UserHandle as _, UsersExt as _},
};

const UNLOCK_BG_ORANGE: u32 = 0xff6600ff;
const UNLOCK_BG_DARK: u32 = 0x155dfcff;

impl PanelRoot {
    pub fn render_login_frame(
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
                                        self.render_login_picker(window, cx).into_any_element()
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
        profile: Entity<User>,
        _window: &mut Window,
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
                                        cx.spawn(async move |this, cx| {
                                            profile.unlock(cx, password).await?;
                                            info!("Login succeeded?");
                                            this.update(cx, |this, _cx| {
                                                this.active_profile = Some(profile);
                                            })?;
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

    fn render_login_picker(
        &mut self,
        _window: &mut Window,
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
        _window: &mut Window,
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
                            let profile = cx.users().create(display_name, password).await?;
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
}
