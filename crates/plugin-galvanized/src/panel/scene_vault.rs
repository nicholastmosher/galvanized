use gpui::{
    Context, Entity, FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, TaskExt as _, Window, div,
    px,
};
use gpui_component::{ActiveTheme as _, h_flex, input::Input, tooltip::Tooltip, v_flex};
use tracing::info;

use crate::{
    GalvanizedHandle as _,
    domain::vault::{Vault, VaultHandle as _},
    panel::{BackButton, GalvanizedPanel, PrimaryButton as _, render_scene_header},
};

/// States for the onboarding flow.
///
/// Each variant corresponds to a scene in the onboarding panel.
/// The flow progresses linearly for new users, or branches to
/// sign-in for existing users.
pub enum VaultScene {
    /// Initial vault picker shows existing vaults and create-new
    VaultPicker,
    /// Create vault (master password + display name)
    CreateVault,
    /// Sign-in prompt for an existing vault
    UnlockPrompt(Entity<Vault>),
}

impl GalvanizedPanel {
    /// Navigate to a specific vault scene.
    pub fn go_to_vault_scene(&mut self, scene: VaultScene, cx: &mut Context<Self>) {
        self.vault_scene = scene;
        cx.notify();
    }

    /// Main onboarding panel layout: header, step progress, scenes, footer.
    pub fn render_scene_vault(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (title, subtitle) = self.header_for_state();

        v_flex()
            .id("vault-scene")
            .size_full()
            .bg(cx.theme().colors.sidebar)
            .child(render_scene_header(title.into(), subtitle.into(), cx))
            .child(
                div()
                    .id("vault-scenes")
                    .h_full()
                    .flex_1()
                    .overflow_y_scroll()
                    .p_5()
                    .child(self.render_scene_vault_exterior(window, cx)),
            )
    }

    /// Route to the current scene renderer based on onboarding state.
    fn render_scene_vault_exterior(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        match &self.vault_scene {
            VaultScene::VaultPicker => {
                //
                self.render_vault_scene_picker(window, cx)
                    .into_any_element()
            }
            VaultScene::CreateVault => {
                //
                self.render_vault_scene_create(window, cx)
                    .into_any_element()
            }
            VaultScene::UnlockPrompt(user) => {
                //
                self.render_vault_scene_unlock(user.clone(), window, cx)
                    .into_any_element()
            }
        }
    }

    fn render_vault_scene_picker(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let colors = cx.theme().colors;

        v_flex()
            .id("scene-picker")
            .pt_2()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .text_color(colors.foreground)
                    .mb_1()
                    .child("Welcome Back"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.muted_foreground)
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
                    .children(self.galvanized.read(cx).vaults.values().map(|vault| {
                        let name = vault.read(cx).name();
                        let initial = name.chars().next().unwrap_or('?').to_string();

                        h_flex()
                            .id(format!("user-card-{}", name))
                            .items_center()
                            .gap_3()
                            .p_3()
                            .rounded_xl()
                            .bg(colors.muted.opacity(0.5))
                            .border_1()
                            .border_color(colors.border)
                            .hover(|style| style.bg(colors.list_hover).border_color(colors.ring))
                            .cursor_pointer()
                            .on_click(cx.listener({
                                let vault = vault.clone();
                                move |this, _e, _window, cx| {
                                    this.go_to_vault_scene(
                                        VaultScene::UnlockPrompt(vault.clone()),
                                        cx,
                                    );
                                }
                            }))
                            .child(
                                h_flex()
                                    .id(format!("user-avatar-{}", name))
                                    .size(px(40.))
                                    .rounded_full()
                                    .bg(*crate::panel::GZED_ORANGE)
                                    .flex_shrink_0()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            // .mx_auto()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(colors.foreground)
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
                                            .text_color(colors.foreground)
                                            .child(name),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(colors.muted_foreground)
                                            .child("Vault locked"),
                                    ),
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
                            // .bg(colors.muted.opacity(0.5))
                            // .border_color(colors.accent.opacity(0.6))
                            .bg(colors.list_hover)
                            .border_color(colors.ring)
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
                            .bg(colors.border)
                            .border_2()
                            .border_dashed()
                            .border_color(colors.border)
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    //
                                    // .mx_auto()
                                    .text_lg()
                                    .text_color(colors.foreground)
                                    .child("+"),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(colors.foreground)
                                    .child("Create new vault"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.muted_foreground)
                                    .child("Set up a fresh decentralized data space"),
                            ),
                    ),
            )
    }

    fn render_vault_scene_create(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let colors = cx.theme().colors;

        v_flex()
            .size_full()
            //
            .justify_center()
            .child(
                v_flex()
                    //
                    .id("scene-create-vault")
                    .w_full()
                    .child(
                        h_flex()
                            .id("vault-icon")
                            .size(px(48.))
                            .mx_auto()
                            .mb_4()
                            .rounded_full()
                            .bg(colors.border)
                            .border_2()
                            .border_color(colors.border)
                            .items_center()
                            .justify_center(),
                    ),
            )
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .text_color(colors.foreground)
                    .text_center()
                    .mb_1()
                    .child("Create Your Vault"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.muted_foreground)
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
                                h_flex()
                                    .id("vault-name-title")
                                    .gap_1()
                                    .text_xs()
                                    .text_color(colors.muted_foreground)
                                    .tooltip(|window, cx| {
                                        Tooltip::new("Visible before unlock, only visible to you")
                                            .build(window, cx)
                                    })
                                    .child("Vault Name"),
                            )
                            .child(Input::new(&self.vault_name_input)),
                    )
                    .child(
                        div()
                            .child(
                                h_flex()
                                    .id("profile-name-title")
                                    .gap_1()
                                    .text_xs()
                                    .text_color(colors.muted_foreground)
                                    .tooltip(|window, cx| {
                                        Tooltip::new(
                                            "Visible only after unlock, visible to contacts",
                                        )
                                        .build(window, cx)
                                    })
                                    .child("Profile Name"),
                            )
                            .child(Input::new(&self.profile_name_input)),
                    )
                    .child(
                        div()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.muted_foreground)
                                    .mb_1()
                                    .child("Master Password"),
                            )
                            .child(Input::new(&self.create_password_input)),
                    )
                    .child(
                        div()
                            .id("vault-create-confirm-password")
                            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                                if e.keystroke.key == "enter" {
                                    this.submit_create_vault(window, cx);
                                }
                            }))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(colors.muted_foreground)
                                    .mb_1()
                                    .child("Confirm Password"),
                            )
                            .child(Input::new(&self.create_password_confirmation_input)),
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
                            .back_button(cx)
                            .on_click(cx.listener(|this, _e, _window, _cx| {
                                this.vault_scene = VaultScene::VaultPicker;
                            }))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(colors.muted_foreground)
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
                            .on_click(cx.listener(|this, _e, window, cx| {
                                this.submit_create_vault(window, cx);
                            }))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(colors.foreground)
                                    .text_center()
                                    .child("Create Vault"),
                            ),
                    ),
            )
    }

    fn render_vault_scene_unlock(
        &mut self,
        vault: Entity<Vault>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let name = vault.read(cx).name();
        let initial = name.chars().next().unwrap_or('?').to_string();

        h_flex()
            .size_full()
            //
            .justify_center()
            .child(
                v_flex()
                    .id("scene-unlock")
                    .w_full()
                    .text_center()
                    .child(
                        h_flex()
                            .id("sign-in-avatar")
                            .size(px(64.))
                            .mx_auto()
                            .mb_3()
                            .rounded_full()
                            .bg(*crate::panel::GZED_ORANGE)
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .border_2()
                            .border_color(cx.theme().colors.border)
                            .child(
                                div()
                                    .mx_auto()
                                    .text_xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().colors.foreground)
                                    .child(initial),
                            ),
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().colors.foreground)
                            .mb_1()
                            .child(name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().colors.muted_foreground)
                            .mb_1()
                            .child("Enter your vault password to unlock"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().colors.muted_foreground)
                            .mb_5()
                            .child("1 profile"),
                    )
                    .child(
                        div().flex_col().gap_3().child(
                            div()
                                .id("unlock-password-input")
                                .w_full()
                                .on_key_down(cx.listener({
                                    let vault = vault.clone();
                                    move |this, e: &KeyDownEvent, window, cx| {
                                        if e.keystroke.key == "enter" {
                                            this.submit_unlock_vault(vault.clone(), window, cx);
                                        }
                                    }
                                }))
                                .child(Input::new(&self.unlock_password_input)),
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
                                    .back_button(cx)
                                    .on_click(cx.listener(|this, _e, _window, _cx| {
                                        this.vault_scene = VaultScene::VaultPicker;
                                    }))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().colors.foreground)
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
                                    .on_click(cx.listener(move |this, _e, window, cx| {
                                        //
                                        this.submit_unlock_vault(vault.clone(), window, cx);
                                    }))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().colors.foreground)
                                            .text_center()
                                            .child("Unlock"),
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

    fn submit_create_vault(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let vault_name = self.vault_name_input.read(cx).value();
        if vault_name.is_empty() {
            return;
        }

        let profile_name = self.profile_name_input.read(cx).value();
        if profile_name.is_empty() {
            return;
        }

        let password = self.create_password_input.read(cx).value();
        if password.is_empty() {
            return;
        }
        let password_confirm = self.create_password_confirmation_input.read(cx).value();
        if password.is_empty() || password_confirm.is_empty() {
            return;
        }

        if password != password_confirm {
            return;
        }

        info!("Creating vault: {}", vault_name);
        cx.spawn(async move |this, cx| {
            let galvanized = this.read_with(cx, |this, _cx| this.galvanized.clone())?;
            let vault = galvanized
                .create_vault(vault_name.to_string(), password.to_string(), cx)
                .await?;
            galvanized.set_active_vault(vault.clone(), cx);
            vault
                .update(cx, |vault, cx| {
                    vault.create_profile(profile_name.into(), cx)
                })
                .await?;
            this.update(cx, |this, _cx| {
                this.vault_scene = VaultScene::VaultPicker;
            })?;
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }

    fn submit_unlock_vault(
        &mut self,
        vault: Entity<Vault>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = self.unlock_password_input.clone();
        let password = input.read(cx).value();
        input.update(cx, |input, cx| input.set_value("", window, cx));
        if password.trim().is_empty() {
            return;
        }

        info!("Unlock clicked");
        let vault = vault.clone();
        cx.spawn(async move |this, cx| {
            vault.unlock(cx, password.to_string()).await?;
            this.update(cx, |this, cx| {
                this.galvanized.set_active_vault(vault, cx);
            })?;
            info!("Unlock succeeded");
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }
}
