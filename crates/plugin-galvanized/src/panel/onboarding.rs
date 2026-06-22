use std::sync::LazyLock;

use tracing::info;
use zed::unstable::{
    gpui::{Entity, FontWeight, Hsla, rgba},
    ui::{
        ActiveTheme, Color, Context, FluentBuilder as _, Icon, IconName, IconSize,
        InteractiveElement, IntoElement, ParentElement as _, StatefulInteractiveElement as _,
        Styled, Window, div, h_flex, px, v_flex,
    },
};

use crate::{
    panel::{OnboardingState, PanelRoot, PrimaryButton as _, gzed_icon},
    users::{User, UserHandle as _},
};

static GZED_ORANGE: LazyLock<Hsla> = LazyLock::new(|| Hsla::from(rgba(0xff6600ff)).opacity(0.8));

impl PanelRoot {
    /// Main onboarding panel layout: header, step progress, scenes, footer.
    pub fn render_onboarding_panel(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let panel_width = self.width.unwrap_or(px(380.)) - px(1.);

        v_flex()
            .id("onboarding-panel")
            .h_full()
            .w(panel_width)
            .bg(cx.theme().colors().panel_background)
            .child(self.render_onboarding_header(cx))
            .child(self.render_step_progress(cx))
            .child(
                div()
                    .id("onboarding-scenes")
                    .flex_1()
                    .overflow_y_scroll()
                    .px_5()
                    .py_5()
                    .child(self.render_current_scene(window, cx)),
            )
            .child(self.render_onboarding_footer(cx))
    }

    /// Panel header with logo, title, and subtitle.
    fn render_onboarding_header(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, subtitle) = self.header_for_state();

        h_flex()
            .id("onboarding-header")
            .items_center()
            .gap_3()
            .p_4()
            .border_b_1()
            .border_color(cx.theme().colors().border_variant)
            .child(gzed_icon(
                "gzed-onboarding-header",
                cx,
                cx.listener(|_this, _e, _window, _cx| {
                    info!("Clicked gzed onboarding header");
                }),
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .id("panel-title")
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().colors().text)
                            .truncate()
                            .child(title),
                    )
                    .child(
                        div()
                            .id("panel-subtitle")
                            .text_xs()
                            .text_color(cx.theme().colors().text_placeholder)
                            .child(subtitle),
                    ),
            )
    }

    /// Step progress dots: hidden on picker, sign-in, and done scenes.
    fn render_step_progress(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_step = self.current_step();
        let show = current_step > 0;
        let colors = cx.theme().colors();
        let status = cx.theme().status();

        h_flex()
            .id("step-progress")
            .when(!show, |el| el.hidden())
            .items_center()
            .justify_center()
            .gap_2()
            .px_5()
            .py_3()
            .border_b_1()
            .border_color(colors.border_variant)
            .children((1..=2).map(|i| {
                let is_active = i == current_step;
                let is_done = i < current_step;

                let color = if is_done {
                    status.success
                } else if is_active {
                    *GZED_ORANGE
                } else {
                    colors.element_disabled
                };

                let mut dot = div()
                    .id(format!("step-{}", i))
                    .size(px(8.))
                    .rounded_full()
                    .bg(color);

                if is_active {
                    dot = dot.shadow_lg();
                }

                dot.into_any_element()
            }))
            .into_any_element()
    }

    /// Footer with encryption notice.
    fn render_onboarding_footer(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();

        h_flex()
            .id("onboarding-footer")
            .px_5()
            .py_3()
            .border_t_1()
            .border_color(colors.border_variant)
            .justify_center()
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_placeholder)
                    .child("🔐 Everything is end-to-end encrypted"),
            )
    }

    /// Route to the current scene renderer based on onboarding state.
    fn render_current_scene(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        match &self.onboarding_state {
            OnboardingState::Picker => self.render_scene_picker(window, cx).into_any_element(),
            OnboardingState::SignIn(user) => self
                .render_scene_sign_in(user.clone(), window, cx)
                .into_any_element(),
            OnboardingState::Welcome => self.render_scene_welcome(window, cx).into_any_element(),
            OnboardingState::CreateVault => self
                .render_scene_create_vault(window, cx)
                .into_any_element(),
            OnboardingState::Done => {
                //
                // self.render_scene_done(window, cx).into_any_element()
                div().into_any_element()
            }
        }
    }

    fn render_scene_picker(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let colors = cx.theme().colors();

        v_flex()
            .id("scene-picker")
            .pt_2()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .text_color(colors.text)
                    .mb_1()
                    .child("Welcome Back"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .mb_4()
                    .child("Choose a vault to unlock, or create a new one."),
            )
            .child(
                //
                v_flex()
                    //
                    .id("user-list")
                    .flex_col()
                    .gap_2()
                    .mb_5()
                    .children(
                        //
                        self
                            //
                            .users
                            .iter()
                            .enumerate()
                            .map(|(_ix, user)| {
                                let name = user.read(cx).name();
                                let initial = name.chars().next().unwrap_or('?').to_string();

                                h_flex()
                                    .id(format!("user-card-{}", name))
                                    .items_center()
                                    .gap_3()
                                    .p_3()
                                    .rounded_xl()
                                    .bg(colors.element_background.opacity(0.5))
                                    .border_1()
                                    .border_color(colors.border)
                                    .hover(|style| {
                                        style
                                            .bg(colors.element_hover)
                                            .border_color(colors.border_focused)
                                    })
                                    .cursor_pointer()
                                    .on_click(cx.listener({
                                        let user = user.clone();
                                        move |this, _e, _window, _cx| {
                                            this.onboarding_state =
                                                OnboardingState::SignIn(user.clone());
                                        }
                                    }))
                                    .child(
                                        h_flex()
                                            .id(format!("user-avatar-{}", name))
                                            .size(px(40.))
                                            .rounded_full()
                                            .bg(*GZED_ORANGE)
                                            .flex_shrink_0()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                div()
                                                    .mx_auto()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(colors.text)
                                                    .child(initial),
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
                                                    .text_color(colors.text)
                                                    .child(name),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(colors.text_placeholder)
                                                    .child("Vault locked"),
                                            ),
                                    )
                                    .child(
                                        Icon::new(IconName::ChevronRight)
                                            .size(IconSize::Small)
                                            .color(Color::Custom(colors.border_variant)),
                                    )
                            }),
                    ),
            )
            .child(
                // Create new vault button
                h_flex()
                    .id("create-new-vault")
                    .w_full()
                    .items_center()
                    .gap_3()
                    .p_3()
                    .rounded_xl()
                    .border_2()
                    .border_dashed()
                    .border_color(colors.border)
                    .hover(|style| {
                        style
                            .border_color(colors.text_accent.opacity(0.6))
                            .bg(colors.element_background.opacity(0.5))
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _e, _window, _cx| {
                        this.onboarding_state = OnboardingState::Welcome;
                    }))
                    .child(
                        h_flex()
                            .id("create-vault-icon")
                            .size(px(40.))
                            .rounded_full()
                            .bg(colors.border_variant)
                            .border_2()
                            .border_dashed()
                            .border_color(colors.border_variant)
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    //
                                    .mx_auto()
                                    .text_lg()
                                    .text_color(colors.text)
                                    .child("+"),
                            ),
                    )
                    .child(
                        div()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(colors.text)
                                    .child("Create new vault"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.text_placeholder)
                                    .child("Set up a fresh decentralized data space"),
                            ),
                    ),
            )
    }

    fn render_scene_sign_in(
        &mut self,
        user: Entity<User>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let name = user.read(cx).name();
        let initial = name.chars().next().unwrap_or('?').to_string();
        let colors = cx.theme().colors();

        v_flex()
            .id("scene-sign-in")
            .pt_2()
            .text_center()
            .child(
                h_flex()
                    .id("sign-in-avatar")
                    .size(px(64.))
                    .mx_auto()
                    .mb_3()
                    .rounded_full()
                    .bg(*GZED_ORANGE)
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .border_2()
                    .border_color(colors.border)
                    .child(
                        div()
                            .mx_auto()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(colors.text)
                            .child(initial),
                    ),
            )
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .text_color(colors.text)
                    .mb_1()
                    .child(name.clone()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .mb_1()
                    .child("Enter your vault password to unlock"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_placeholder)
                    .mb_5()
                    .child("1 profile"),
            )
            .child(
                div().flex_col().gap_3().child(
                    div()
                        .id("password-input-wrapper")
                        .w_full()
                        .child(self.login_password_input.clone()),
                ),
            )
            .child(
                h_flex()
                    .id("sign-in-actions")
                    .gap_2()
                    .mt_6()
                    .child(
                        div()
                            .id("sign-in-back")
                            .flex_1()
                            .px_3()
                            .py_2()
                            .rounded_lg()
                            .bg(colors.border_variant)
                            .hover(|style| style.bg(colors.border))
                            .border_1()
                            .border_color(colors.border)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _e, _window, _cx| {
                                this.onboarding_state = OnboardingState::Picker;
                            }))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(colors.text_muted)
                                    .text_center()
                                    .child("Back"),
                            ),
                    )
                    .child(
                        div()
                            .id("sign-in-unlock")
                            .flex_1()
                            .px_3()
                            .py_2()
                            .rounded_lg()
                            .primary_button()
                            .shadow_lg()
                            .cursor_pointer()
                            .on_click(cx.listener({
                                let user = user.clone();
                                move |this, _e, window, cx| {
                                    let input = this.login_password_input.clone();
                                    let password = input.read(cx).text(cx);
                                    input.update(cx, |input, cx| input.clear(window, cx));
                                    if password.trim().is_empty() {
                                        return;
                                    }

                                    info!("Sign in clicked");
                                    let user = user.clone();
                                    cx.spawn(async move |this, cx| {
                                        user.unlock(cx, password).await?;
                                        info!("Sign in succeeded");
                                        this.update(cx, |this, _cx| {
                                            this.active_user = Some(user);
                                        })?;
                                        anyhow::Ok(())
                                    })
                                    .detach_and_log_err(cx);
                                }
                            }))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(colors.text)
                                    .text_center()
                                    .child("Unlock"),
                            ),
                    ),
            )
    }

    fn render_scene_welcome(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .id("scene-welcome")
            .w_full()
            .text_center()
            .pt_4()
            .child(gzed_icon("gzed-welcome", cx, cx.listener(|_this, _e, _window, _cx| {
                info!("Clicked welcome");
            })))
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(cx.theme().colors().text)
                    .mb_2()
                    .child("Welcome to Galvanized"),
            )
            .child(
                div()
                    .id("welcome-description")
                    .text_sm()
                    .text_color(cx.theme().colors().text_muted)
                    .mb_6()
                    .child("Your decentralized data space. Everything encrypted, unlocked by one master password."),
            )
            .child(
                div()
                    .id("welcome-create-btn")
                    .w_full()
                    .self_center()
                    .px_4()
                    .py_2()
                    .rounded_lg()
                    .primary_button()
                    .shadow_lg()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _e, _window, _cx| {
                        this.onboarding_state = OnboardingState::CreateVault;
                    }))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().colors().text)
                            .text_center()
                            .child("Create Your First Vault"),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().colors().border_variant)
                    .mt_5()
                    .child(
                        div()
                            .id("welcome-back-link")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _e, _window, _cx| {
                                this.onboarding_state = OnboardingState::Picker;
                            }))
                            .child(
                                div()
                                    .text_color(*GZED_ORANGE)
                                    .child("← Back to user selection"),
                            ),
                    ),
            )
    }

    fn render_scene_create_vault(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let colors = cx.theme().colors();

        v_flex()
            .id("scene-create-vault")
            .pt_2()
            .child(
                h_flex()
                    .id("vault-icon")
                    .size(px(48.))
                    .mx_auto()
                    .mb_4()
                    .rounded_full()
                    .bg(colors.border_variant)
                    .border_2()
                    .border_color(colors.border)
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            //
                            .mx_auto()
                            .child(
                                Icon::new(IconName::LockOutlined)
                                    .size(IconSize::Medium)
                                    .color(Color::Custom(*GZED_ORANGE)),
                            ),
                    ),
            )
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .text_color(colors.text)
                    .text_center()
                    .mb_1()
                    .child("Create Your Vault"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .text_center()
                    .mb_5()
                    .child("One master password unlocks all your profiles, keys, and data."),
            )
            .child(
                v_flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.text_muted)
                                    .mb_1()
                                    .child("Display Name"),
                            )
                            .child(self.display_name_input.clone()),
                    )
                    .child(
                        div()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.text_muted)
                                    .mb_1()
                                    .child("Master Password"),
                            )
                            .child(self.create_password_input.clone()),
                    )
                    .child(
                        div()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.text_muted)
                                    .mb_1()
                                    .child("Confirm Password"),
                            )
                            .child(self.create_password_confirmation_input.clone()),
                    ),
            )
            .child(
                h_flex()
                    .id("vault-actions")
                    .gap_2()
                    .mt_6()
                    .child(
                        div()
                            .id("vault-back")
                            .flex_1()
                            .px_3()
                            .py_2()
                            .rounded_lg()
                            .bg(colors.border_variant)
                            .hover(|style| style.bg(colors.border))
                            .border_1()
                            .border_color(colors.border)
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _e, _window, _cx| {
                                this.onboarding_state = OnboardingState::Welcome;
                            }))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(colors.text_muted)
                                    .text_center()
                                    .child("Back"),
                            ),
                    )
                    .child(
                        div()
                            .id("vault-create")
                            .flex_1()
                            .px_3()
                            .py_2()
                            .rounded_lg()
                            .primary_button()
                            .shadow_lg()
                            .cursor_pointer()
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

                                info!("Creating vault: {}", display_name);
                                cx.spawn(async move |this, cx| {
                                    let galvanized =
                                        this.read_with(cx, |this, _cx| this.galvanized.clone())?;
                                    let user = galvanized
                                        .update(cx, |g, cx| {
                                            g.create_user(display_name, password, cx)
                                        })
                                        .await?;
                                    this.update(cx, |this, _cx| {
                                        this.users.push(user.clone());
                                        this.active_user = Some(user);
                                        this.onboarding_state = OnboardingState::Done;
                                    })?;
                                    anyhow::Ok(())
                                })
                                .detach_and_log_err(cx);
                            }))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(colors.text)
                                    .text_center()
                                    .child("Create Vault"),
                            ),
                    ),
            )
    }

    /// Returns the (title, subtitle) for the current onboarding state.
    fn header_for_state(&self) -> (&'static str, &'static str) {
        match self.onboarding_state {
            OnboardingState::Picker => ("Galvanized", "Your decentralized data space"),
            OnboardingState::SignIn(_) => ("Sign In", "Unlock your vault"),
            OnboardingState::Welcome | OnboardingState::CreateVault => {
                ("Getting Started", "Set up your decentralized data space")
            }
            OnboardingState::Done => ("Welcome!", "Your vault is ready"),
        }
    }

    /// Returns the current step number (1-3) for onboarding scenes, or 0 for non-onboarding scenes.
    fn current_step(&self) -> usize {
        match self.onboarding_state {
            OnboardingState::Picker | OnboardingState::SignIn(_) | OnboardingState::Done => 0,
            OnboardingState::Welcome => 1,
            OnboardingState::CreateVault => 2,
        }
    }
}
