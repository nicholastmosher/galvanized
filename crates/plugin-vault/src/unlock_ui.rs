use std::{boxed::Box, pin::Pin};

use anyhow::{Context as _, Result};
use tokio::sync::oneshot;
use tracing::info;
use zed::unstable::{
    gpui::{
        AppContext, Bounds, Entity, KeyDownEvent, TitlebarOptions, WindowBounds, WindowHandle,
        WindowKind, WindowOptions,
    },
    ui::{
        ActiveTheme as _, App, Context, FluentBuilder as _, InteractiveElement as _, IntoElement,
        ParentElement, Render, StatefulInteractiveElement as _, Styled as _, Window, div, h_flex,
        px, v_flex,
    },
    ui_input::InputField,
    util::ResultExt,
};

use crate::{Unlock, VaultsCx, gpui::size, vault_db::VaultId};

impl<T: IntoElement> Locked for T {}
pub trait Locked {
    /// Render the inner element only when the vault is unlocked.
    ///
    /// When the vault is locked, this renders a standard locked element with an unlock button.
    fn locked<C: 'static>(
        self,
        cx: &mut Context<C>,
        f: impl FnOnce(Self, &mut Context<C>) -> Self,
    ) -> Self
    where
        Self: Sized,
        Self: ParentElement,
    {
        // if cx.vault().is_unlocked() {
        //     return f(self, cx);
        // }

        self
            //
            .child(locked_ui(None, cx))
    }

    /// Render the inner element only when the vault is unlocked.
    ///
    /// When the vault is locked, this renders a standard locked element with an unlock button.
    fn locked_prompt<C: 'static>(
        self,
        password_input: Entity<InputField>,
        window: &mut Window,
        cx: &mut Context<C>,
        f: impl FnOnce(Self, &mut Window, &mut Context<C>) -> Self,
    ) -> Self
    where
        Self: Sized,
        Self: ParentElement,
    {
        // if cx.vault().is_unlocked() {
        //     return f(self, window, cx);
        // }

        // Enforce input field to be masked
        password_input.update(cx, |input, cx| input.set_masked(true, window, cx));
        self
            //
            .child(locked_ui(Some(password_input), cx))
    }
}

fn locked_ui<C: 'static>(
    password_prompt: Option<Entity<InputField>>,
    cx: &mut Context<C>,
) -> impl IntoElement {
    h_flex()
        .size_full()
        //
        .items_center()
        .child(
            //
            v_flex()
                // .debug()
                //
                .mx_auto()
                .items_center()
                .gap_2()
                .child(
                    //
                    div()
                        //
                        .text_color(cx.theme().colors().text)
                        .text_3xl()
                        .child("Locked"),
                )
                .map(|el| match password_prompt {
                    None => {
                        el
                            //
                            .child(
                                //
                                div()
                                    //
                                    .id("unlock-button")
                                    .p_2()
                                    .border_1()
                                    .border_color(cx.theme().colors().border)
                                    .rounded_lg()
                                    .hover(|style| {
                                        style.bg(cx.theme().colors().ghost_element_hover)
                                    })
                                    .active(|style| {
                                        style.bg(cx.theme().colors().ghost_element_active)
                                    })
                                    .on_click(|_e, window, cx| {
                                        window.dispatch_action(Box::new(Unlock), cx);
                                    })
                                    .child("Unlock"),
                            )
                    }
                    Some(input) => {
                        //
                        el.child(
                            div()
                                //
                                .id("unlock-password")
                                .w_full()
                                //
                                .p_2()
                                .items_center()
                                .on_key_down(cx.listener({
                                    let input = input.clone();
                                    move |_this, e: &KeyDownEvent, window, cx| {
                                        if e.keystroke.key != "enter" {
                                            return;
                                        }

                                        let text = input.read(cx).text(cx);
                                        input.update(cx, |input, cx| input.clear(window, cx));
                                        // if cx.vault().unlock(&text).is_err() {
                                        //     warn!("Incorrect password");
                                        // }
                                    }
                                }))
                                .child(input.clone()),
                        )
                    }
                }),
        )
}

impl<'a> VaultsCx<'a, App> {
    // pub fn lock(&mut self) {
    //     self.cx
    //         .update_entity(&self.state, |state, cx| {
    //             if let Some((_tx, window)) = state.pending_unlock.take() {
    //                 window.update(cx, |_view, window, _cx| {
    //                     window.remove_window();
    //                 })?;
    //             }
    //             anyhow::Ok(())
    //         })
    //         .log_err();
    // }

    // pub fn unlock(&mut self, password: &str) -> Result<TimedCap<VaultAll>> {
    //     // TODO: Obviously we need to do a check here
    //     if password != "password" {
    //         bail!("invalid password");
    //     }

    //     let cap = self.cx.update_entity(&self.state, |state, cx| {
    //         if let Some((_tx, window)) = state.pending_unlock.take() {
    //             window.update(cx, |_view, window, _cx| {
    //                 window.remove_window();
    //             })?;
    //         }

    //         let cap = state.root.grant();
    //         let cap = TimedCap::new(cap, LOCK_TIMEOUT);
    //         state.vault_cap = Some(cap.clone());
    //         anyhow::Ok(cap)
    //     })?;
    //     Ok(cap)
    // }

    pub fn unlock_window(&mut self) {
        //
    }

    // /// Time-bounded permission to full profile access
    // pub fn unlock_window(&mut self) -> Task<Result<TimedCap<VaultAll>>> {
    //     // Three possible states:
    //     // 1) Vault is already unlocked, return cached capability
    //     // 2) Vault is locked and no pending unlock exists, open a new unlock window
    //     // 3) Vault is locked but a pending unlock exists, return the existing cap

    //     // 1) Check if the vault is already unlocked
    //     {
    //         let vault_cap = self
    //             .cx
    //             .read_entity(&self.state, |state, _cx| state.vault_cap.clone());

    //         if let Some(timed_cap) = vault_cap {
    //             if timed_cap.is_active() {
    //                 info!("Vault already unlocked, returning cached capability");
    //                 return Task::ready(Ok(timed_cap));
    //             }
    //             debug!("Vault cap present but expired");
    //         }
    //     }

    //     // 2) If there's an open Unlock window, return a task subscribed to it
    //     {
    //         let pending_rx = self.cx.update_entity(&self.state, |state, _cx| {
    //             state
    //                 .pending_unlock
    //                 .as_ref()
    //                 .map(|(tx, _window)| tx.subscribe())
    //         });

    //         // If an Unlock window is already open, return a task that yields the result
    //         // In other words, only open one unlock window at a time
    //         if let Some(mut pending_rx) = pending_rx {
    //             info!("Unlock window already open, waiting for password");
    //             let task = self.cx.spawn(async move |_cx| {
    //                 let cap = pending_rx.recv().await?;
    //                 anyhow::Ok(cap)
    //             });

    //             return task;
    //         }
    //     }

    //     // 3) Open a new Unlock window
    //     // - Oneshot from Unlock window -> Vault when password is accepted
    //     // - Vault task grants and caches capability
    //     // - Vault broadcasts capability to all waiting client tasks
    //     let (unlock_tx, unlock_rx) = oneshot::channel();
    //     let unlock_init_result = (|| {
    //         let window = self.open_unlock_window(unlock_tx)?;
    //         let (tx, _rx) = broadcast::channel(1);
    //         self.state.update(self.cx, |state, _cx| {
    //             state.pending_unlock = Some((tx, window));
    //         });
    //         anyhow::Ok(())
    //     })();

    //     let state = self.state.clone();
    //     let task = self.cx.spawn(async move |cx| {
    //         // Propagate potential window error from above
    //         unlock_init_result?;

    //         // Wait for unlock to complete
    //         unlock_rx.await?;

    //         let cap = cx.update_entity(&state, |state, cx| {
    //             // Newly minted capability
    //             let cap = state.root.grant::<VaultAll>();
    //             let cap = TimedCap::new(cap, LOCK_TIMEOUT);
    //             state.vault_cap = Some(cap.clone());

    //             // Take the receiver, so future unlocks prompt a new window
    //             if let Some((tx, window)) = state.pending_unlock.take() {
    //                 tx.send(cap.clone()).ok();

    //                 // Close unlock window in case it wasn't already scheduled
    //                 window.update(cx, |_view, window, _cx| {
    //                     window.remove_window();
    //                 })?;
    //             }

    //             anyhow::Ok(cap)
    //         })?;

    //         anyhow::Ok(cap)
    //     });

    //     task
    // }

    // pub fn is_unlocked(&self) -> bool {
    //     self.state
    //         .read(self.cx)
    //         .vault_cap
    //         .as_ref()
    //         .map(|cap| cap.is_active())
    //         .unwrap_or(false)
    // }

    fn open_unlock_window(
        &mut self,
        tx: oneshot::Sender<String>,
    ) -> Result<WindowHandle<VaultUnlockUi>> {
        let bounds = Bounds::centered(None, size(px(300.), px(300.)), self.cx);
        let titlebar = TitlebarOptions {
            title: Some("Vault Unlock".into()),
            appears_transparent: true,
            ..Default::default()
        };
        let window_bounds = WindowBounds::Windowed(bounds);
        let window_options = WindowOptions {
            window_bounds: Some(window_bounds),
            titlebar: Some(titlebar),
            // window_background: WindowBackgroundAppearance::Transparent,
            // kind: WindowKind::Floating,
            kind: WindowKind::PopUp,
            ..Default::default()
        };
        let window = self
            .cx
            .open_window(window_options, |window, cx| {
                cx.new(|cx| VaultUnlockUi::new(tx, window, cx))
            })
            .context("failed to open vault unlock window")?;

        Ok(window)
    }
}

pub trait UnlockPrompt {
    fn prompt_unlock(
        &mut self,
        vault_id: &VaultId,
        cx: &mut App,
    ) -> Pin<Box<dyn Future<Output = Result<String>>>>;
}

pub struct WindowUnlockPrompt {
    //
}

impl UnlockPrompt for WindowUnlockPrompt {
    fn prompt_unlock(
        &mut self,
        vault_id: &VaultId,
        cx: &mut App,
    ) -> Pin<Box<dyn Future<Output = Result<String>>>> {
        let (tx, rx) = oneshot::channel();

        let bounds = Bounds::centered(None, size(px(300.), px(300.)), cx);
        let titlebar = TitlebarOptions {
            title: Some("Vault Unlock".into()),
            appears_transparent: true,
            ..Default::default()
        };
        let window_bounds = WindowBounds::Windowed(bounds);
        let window_options = WindowOptions {
            window_bounds: Some(window_bounds),
            titlebar: Some(titlebar),
            // window_background: WindowBackgroundAppearance::Transparent,
            // kind: WindowKind::Floating,
            kind: WindowKind::PopUp,
            ..Default::default()
        };

        cx.open_window(window_options, |window, cx| {
            //
            cx.new(|cx| VaultUnlockUi::new(tx, window, cx))
        });

        let cx = cx.to_async();
        let vault_id = vault_id.clone();
        Box::pin(async move {
            let password = rx.await.expect("channel error");
            // let task = cx.vaults().unlock(&vault_id, password);
            Ok(password)
        })
    }
}

impl<F> UnlockPrompt for F
where
    F: Fn(&VaultId, &mut App) -> String,
{
    fn prompt_unlock(
        &mut self,
        vault_id: &VaultId,
        cx: &mut App,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + 'static>> {
        let password = (self)(vault_id, cx);
        Box::pin(async move {
            //
            Ok(password)
        })
    }
}

/// Top-level UI for the unlock window
pub struct VaultUnlockUi {
    //
    input: Entity<InputField>,
    tx: Option<oneshot::Sender<String>>,
}

impl VaultUnlockUi {
    pub fn new(tx: oneshot::Sender<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputField::new(window, cx, "Password").masked(true));
        Self {
            input,
            tx: Some(tx),
        }
    }
}

impl Render for VaultUnlockUi {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // If the unlock window is closed, reset vault state
        window.on_window_should_close(cx, |_window, _cx| {
            info!("Closing Unlock window, locking");
            // cx.vault().lock();
            true
        });

        div()
            .size_full()
            //
            .p_6()
            .bg(cx.theme().colors().editor_background)
            .border_2()
            .border_color(cx.theme().colors().border_selected)
            .rounded_lg()
            .child(
                //
                h_flex()
                    .size_full()
                    .bg(cx.theme().colors().panel_background)
                    //
                    .rounded_lg()
                    .shadow_lg()
                    .child(
                        //
                        v_flex()
                            .my_auto()
                            .mx_auto()
                            .w_full()
                            //
                            .items_center()
                            .child(
                                //
                                div()
                                    //
                                    .text_3xl()
                                    .text_color(cx.theme().colors().text)
                                    .child("Locked"),
                            )
                            .child(
                                //
                                div()
                                    .id("unlock-password")
                                    .w_full()
                                    //
                                    .p_2()
                                    .items_center()
                                    .on_key_down(cx.listener(
                                        |this, e: &KeyDownEvent, window, cx| {
                                            if e.keystroke.key != "enter" {
                                                return;
                                            }

                                            let password = this.input.read(cx).text(cx);
                                            if let Some(tx) = this.tx.take() {
                                                tx.send(password).log_err();
                                                window.remove_window();
                                            }
                                        },
                                    ))
                                    .child(self.input.clone()),
                            ),
                    ),
            )
    }
}
