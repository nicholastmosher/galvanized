use std::{sync::Arc, time::Duration};

use uuid::Uuid;
use zed::unstable::{
    gpui::{
        self, Animation, AnimationExt as _, AppContext as _, EventEmitter, FocusHandle, Focusable,
        Image, ImageSource, actions, img, linear_color_stop, linear_gradient, rgba,
    },
    ui::{
        ActiveTheme as _, App, Context, IntoElement, ParentElement as _, Render, SharedString,
        Styled, Window, div, h_flex, px, v_flex,
    },
    workspace::{Item, Workspace},
};

actions!(galvanized, [OpenUnlockUi]);

pub fn init(cx: &mut App) {
    cx.observe_new::<Workspace>(|workspace, window, cx| {
        let Some(window) = window else { return };
        let vault_login = cx.new(|cx| VaultLoginItem::new(cx));

        workspace.add_item_to_active_pane(
            Box::new(vault_login.clone()),
            Some(1),
            false,
            window,
            cx,
        );

        workspace.register_action(move |workspace, _: &OpenUnlockUi, window, cx| {
            workspace.add_item_to_active_pane(
                Box::new(vault_login.clone()),
                Some(1),
                false,
                window,
                cx,
            );
        });
    })
    .detach();
}

pub struct VaultLoginItem {
    focus_handle: FocusHandle,
    items: Vec<(Uuid, SharedString)>,
}

impl VaultLoginItem {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            items: vec![
                //
                (Uuid::new_v4(), "Nick".into()),
                (Uuid::new_v4(), "Robin".into()),
            ],
        }
    }
}

pub enum VaultLoginEvent {}
impl EventEmitter<VaultLoginEvent> for VaultLoginItem {}
impl Focusable for VaultLoginItem {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Item for VaultLoginItem {
    type Event = VaultLoginEvent;

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        "Login".into()
    }
}

const UNLOCK_BG_ORANGE: u32 = 0xff6600ff;
// const UNLOCK_BG_DARK: u32 = 0x00002bff;
const UNLOCK_BG_DARK: u32 = 0x155dfcff;

impl Render for VaultLoginItem {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            //
            .size_full()
            .bg(cx.theme().colors().background)
            .p_4()
            .child(
                //
                div()
                    //
                    // .h_40()
                    .w_80()
                    .mx_auto()
                    .self_center()
                    .p(px(1.))
                    .rounded_lg()
                    .shadow_lg()
                    .child(
                        //
                        v_flex()
                            //
                            .size_full()
                            .bg(cx.theme().colors().panel_background)
                            .p_2()
                            .gap_2()
                            .rounded_lg()
                            .child(
                                //
                                div()
                                    //
                                    .self_center()
                                    .text_3xl()
                                    .child("Locked"),
                            )
                            .children(
                                self.items
                                    //
                                    .iter()
                                    .map(|(id, name)| {
                                        let image =
                                            plot_icon::generate_png(id.as_bytes(), 32).unwrap();

                                        h_flex()
                                            .w_full()
                                            //
                                            .bg(cx.theme().colors().editor_background)
                                            .p_2()
                                            .gap_2()
                                            .rounded_lg()
                                            .border_1()
                                            .border_color(cx.theme().colors().border)
                                            .child(
                                                div()
                                                    //
                                                    // .bg(cx.theme().colors().editor_background)
                                                    .p_2()
                                                    .rounded_md()
                                                    .child(
                                                        //
                                                        img(ImageSource::Image(Arc::new(
                                                            Image::from_bytes(
                                                                gpui::ImageFormat::Png,
                                                                image,
                                                            ),
                                                        ))),
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
                            ),
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
}
