use std::sync::LazyLock;

use tracing::info;
use zed::unstable::{
    gpui::{ClickEvent, Entity, FontWeight, Hsla, rgba},
    ui::{
        ActiveTheme, Color, Context, Icon, IconName, IconSize, InteractiveElement, IntoElement,
        ParentElement as _, StatefulInteractiveElement as _, Styled, Tooltip, Window, div, h_flex,
        px, v_flex,
    },
};

use crate::{
    domain::vaults::{Vault, VaultHandle as _},
    panel::{GalvanizedPanel, PrimaryButton as _, render_scene_header},
};

static GZED_ORANGE: LazyLock<Hsla> = LazyLock::new(|| Hsla::from(rgba(0xff6600ff)).opacity(0.8));

/// States for the onboarding flow.
///
/// Each variant corresponds to a scene in the onboarding panel.
/// The flow progresses linearly for new users, or branches to
/// sign-in for existing users.
pub enum VaultScene {
    /// Initial vault picker shows existing vaults and create-new
    VaultPicker,
    /// Sign-in prompt for an existing vault
    UnlockPrompt(Entity<Vault>),
    /// Create vault (master password + display name)
    CreateVault,
}

impl GalvanizedPanel {
    /// Main onboarding panel layout: header, step progress, scenes, footer.
    pub fn render_scene_vault(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (title, subtitle) = self.header_for_state();

        v_flex()
            .id("onboarding-panel")
            .size_full()
            .bg(cx.theme().colors().panel_background)
            .child(render_scene_header(title.into(), subtitle.into(), cx))
            .child(
                div()
                    .id("onboarding-scenes")
                    .h_full()
                    .flex_1()
                    .overflow_y_scroll()
                    .p_5()
                    .child(self.render_onboarding_scene(window, cx)),
            )
    }

    /// Route to the current scene renderer based on onboarding state.
    fn render_onboarding_scene(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        match &self.vault_scene {
            VaultScene::VaultPicker => {
                //
                self.render_scene_picker(window, cx).into_any_element()
            }
            VaultScene::UnlockPrompt(user) => {
                //
                self.render_scene_sign_in(user.clone(), window, cx)
                    .into_any_element()
            }
            VaultScene::CreateVault => {
                //
                self.render_scene_create_vault(window, cx)
                    .into_any_element()
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
                    .children(self.galvanized.read(cx).users.values().map(|user| {
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
                                    this.vault_scene = VaultScene::UnlockPrompt(user.clone());
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
                    })),
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
                        this.vault_scene = VaultScene::CreateVault;
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
        user: Entity<Vault>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let name = user.read(cx).name();
        let initial = name.chars().next().unwrap_or('?').to_string();

        h_flex()
            .size_full()
            //
            .justify_center()
            .child(
                v_flex()
                    .id("scene-sign-in")
                    .w_full()
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
                            .border_color(cx.theme().colors().border)
                            .child(
                                div()
                                    .mx_auto()
                                    .text_xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().colors().text)
                                    .child(initial),
                            ),
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().colors().text)
                            .mb_1()
                            .child(name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().colors().text_muted)
                            .mb_1()
                            .child("Enter your vault password to unlock"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().colors().text_placeholder)
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
                                    .bg(cx.theme().colors().border_variant)
                                    .hover(|style| style.bg(cx.theme().colors().border))
                                    .border_1()
                                    .border_color(cx.theme().colors().border)
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _e, _window, _cx| {
                                        this.vault_scene = VaultScene::VaultPicker;
                                    }))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().colors().text_muted)
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
                                                this.update(cx, |this, cx| {
                                                    this.galvanized.update(cx, |it, _cx| {
                                                        it.active_user = Some(user);
                                                    });
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
                                            .text_color(cx.theme().colors().text)
                                            .text_center()
                                            .child("Unlock"),
                                    ),
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

        h_flex()
            .size_full()
            //
            .justify_center()
            .child(
                v_flex()
                    .id("scene-create-vault")
                    .w_full()
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
                            .child(
                                "One master password unlocks all your profiles, keys, and data.",
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .child(
                                        h_flex()
                                            .id("vault-name-title")
                                            .gap_1()
                                            .text_xs()
                                            .text_color(colors.text_muted)
                                            .tooltip(Tooltip::text(
                                                "Visible before unlock, only visible to you",
                                            ))
                                            .child("Vault Name")
                                            .child(
                                                Icon::new(IconName::Info).size(IconSize::XSmall),
                                            ),
                                    )
                                    .child(self.vault_name_input.clone()),
                            )
                            .child(
                                div()
                                    .child(
                                        h_flex()
                                            .id("profile-name-title")
                                            .gap_1()
                                            .text_xs()
                                            .text_color(colors.text_muted)
                                            .tooltip(Tooltip::text(
                                                "Visible only after unlock, visible to contacts",
                                            ))
                                            .child("Profile Name")
                                            .child(
                                                Icon::new(IconName::Info).size(IconSize::XSmall),
                                            ),
                                    )
                                    .child(self.profile_name_input.clone()),
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
                                        this.vault_scene = VaultScene::VaultPicker;
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
                                    .primary_button()
                                    .shadow_lg()
                                    .cursor_pointer()
                                    .on_click(cx.listener(Self::submit_create_vault))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(colors.text)
                                            .text_center()
                                            .child("Create Vault"),
                                    ),
                            ),
                    ),
            )
    }

    /// Returns the (title, subtitle) for the current onboarding state.
    fn header_for_state(&self) -> (&'static str, &'static str) {
        match self.vault_scene {
            VaultScene::VaultPicker => ("Galvanized", "Your decentralized data space"),
            VaultScene::UnlockPrompt(_) => ("Sign In", "Unlock your vault"),
            VaultScene::CreateVault => ("Getting Started", "Set up your decentralized data space"),
        }
    }

    fn submit_create_vault(
        &mut self,
        _e: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let vault_name = self.vault_name_input.read(cx).text(cx);
        if vault_name.is_empty() {
            return;
        }

        let profile_name = self.profile_name_input.read(cx).text(cx);
        if profile_name.is_empty() {
            return;
        }

        let password = self.create_password_input.read(cx).text(cx);
        let password_confirm = self.create_password_confirmation_input.read(cx).text(cx);
        if password.is_empty() || password_confirm.is_empty() {
            return;
        }

        if password != password_confirm {
            return;
        }

        info!("Creating vault: {}", vault_name);
        cx.spawn(async move |this, cx| {
            let galvanized = this.read_with(cx, |this, _cx| this.galvanized.clone())?;
            let user = galvanized
                .update(cx, |g, cx| g.create_vault(vault_name, password, cx))
                .await?;
            this.update(cx, |this, cx| {
                this.galvanized.update(cx, |it, _cx| {
                    it.active_user = Some(user);
                });
                this.vault_scene = VaultScene::VaultPicker;
            })?;
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }
}
