use tokio::sync::oneshot;
use tracing::{info, warn};
use zed::unstable::{
    gpui::{AppContext as _, Entity, KeyDownEvent},
    ui::{
        ActiveTheme as _, Context, FluentBuilder as _, InteractiveElement as _, IntoElement,
        ParentElement, Render, StatefulInteractiveElement as _, Styled as _, Window, div, h_flex,
        v_flex,
    },
    ui_input::InputField,
    util::ResultExt,
};

use crate::{Unlock, VaultExt};

/// Top-level UI for the unlock window
pub struct VaultUnlockUi {
    //
    input: Entity<InputField>,
    tx: Option<oneshot::Sender<()>>,
}

impl VaultUnlockUi {
    pub fn new(tx: oneshot::Sender<()>, window: &mut Window, cx: &mut Context<Self>) -> Self {
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
        window.on_window_should_close(cx, |_window, cx| {
            info!("Closing Unlock window, locking");
            cx.vault().lock();
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

                                            let text = this.input.read(cx).text(cx);
                                            // TODO actual password verification
                                            if text == "password" {
                                                if let Some(tx) = this.tx.take() {
                                                    tx.send(()).log_err();
                                                    window.remove_window();
                                                }
                                            } else {
                                                warn!("Incorrect password");
                                            }
                                        },
                                    ))
                                    .child(self.input.clone()),
                            ),
                    ),
            )
    }
}

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
        if cx.vault().is_unlocked() {
            return f(self, cx);
        }

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
        if cx.vault().is_unlocked() {
            return f(self, window, cx);
        }

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
                                        if cx.vault().unlock(&text).is_err() {
                                            warn!("Incorrect password");
                                        }
                                    }
                                }))
                                .child(input.clone()),
                        )
                    }
                }),
        )
}
