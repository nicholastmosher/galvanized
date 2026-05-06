use std::time::Duration;

use plugin_vault::unlock_ui::Locked as _;
use zed::unstable::{
    gpui::{
        self, Animation, AnimationExt, AppContext as _, EventEmitter, FocusHandle, Focusable,
        actions, linear_color_stop, linear_gradient, rgb,
    },
    ui::{
        ActiveTheme, App, Context, IntoElement, ParentElement as _, Render, SharedString, Styled,
        Window, div, h_flex, px, v_flex,
    },
    workspace::{Item, Workspace},
};

const WILLOW_GREEN_RGB: u32 = 0x27E53B;
const WILLOW_YELLOW_RGB: u32 = 0xF5DF48;

pub fn init(cx: &mut App) {
    cx.observe_new::<Workspace>(|workspace, window, cx| {
        let Some(window) = window else { return };

        let willow_ui = cx.new(|cx| WillowUi::new(cx));

        // Open Willow on init for dev purposes
        workspace.add_item_to_active_pane(Box::new(willow_ui.clone()), Some(0), true, window, cx);

        workspace.register_action(move |workspace, _: &OpenWillow, window, cx| {
            workspace.add_item_to_active_pane(
                Box::new(willow_ui.clone()),
                Some(0),
                true,
                window,
                cx,
            );
        });
        //
    })
    .detach();
}

actions!(
    //
    willow,
    [
        //
        OpenWillow,
    ]
);

pub struct WillowUi {
    focus_handle: FocusHandle,
}

impl WillowUi {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for WillowUi {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<()> for WillowUi {}
impl Item for WillowUi {
    type Event = ();

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        "Willow".into()
    }
}

impl Render for WillowUi {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            //
            .p_2()
            .bg(cx.theme().colors().editor_background)
            .child(
                //
                v_flex()
                    .size_full()
                    //
                    .p(px(1.))
                    .rounded_lg()
                    .child(
                        //
                        div()
                            .size_full()
                            //
                            .bg(cx.theme().colors().panel_background)
                            .rounded_lg()
                            .size_full()
                            .child(self.render_panel(window, cx)),
                    )
                    .with_animation(
                        "willow-bg",
                        Animation::new(Duration::from_secs(120)).repeat(),
                        |el, t| {
                            //
                            el
                                //
                                .bg(linear_gradient(
                                    360. * t,
                                    linear_color_stop(rgb(WILLOW_GREEN_RGB), 0.0),
                                    linear_color_stop(rgb(WILLOW_YELLOW_RGB), 1.0),
                                ))
                        },
                    ),
            )
    }
}

impl WillowUi {
    //
    fn render_panel(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        //
        div()
            .size_full()
            //
            .p_2()
            .locked(cx, |el, _cx| {
                el
                    //
                    .child(
                        h_flex()
                            .debug()
                            .size_full()
                            .child(
                                //
                                v_flex()
                                    .debug()
                                    .w_80()
                                    .h_full()
                                    //
                                    .p_2(),
                            )
                            .child(
                                //
                                div()
                                    .debug()
                                    .size_full()
                                    //
                                    .p_2(),
                            ),
                    )
            })
    }
}
