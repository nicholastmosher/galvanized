use tokio::sync::oneshot;
use tracing::warn;
use zed::unstable::{
    gpui::{AppContext as _, Entity, KeyDownEvent},
    ui::{
        ActiveTheme as _, Context, InteractiveElement as _, IntoElement, ParentElement as _,
        Render, Styled as _, Window, div, h_flex, v_flex,
    },
    ui_input::InputField,
};

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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                                                    tx.send(()).unwrap();
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
