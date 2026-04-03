// The Connections UI will initially be Iroh peer connections
//
// This UI is in charge of:
// - Displaying a list of connected peers
// - Allowing adding new peers
// - Allowing removing peers

use tracing::warn;
use zed::unstable::{
    gpui::{AppContext as _, ClipboardItem, Entity},
    ui::{
        ActiveTheme as _, App, Context, FluentBuilder, Icon, IconName, InteractiveElement as _,
        IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled, Tooltip,
        Window, div, h_flex, v_flex,
    },
    ui_input::InputField,
};

use crate::iroh::IrohExt;

pub fn init(cx: &mut App) {
    //
}

pub struct ConnectionsUi {
    //
    input_ticket: Entity<InputField>,
}

impl ConnectionsUi {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        //
        let input_ticket = cx.new(|cx| InputField::new(window, cx, "Paste remote ticket"));
        Self { input_ticket }
    }
}

impl Render for ConnectionsUi {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let endpoint_id = cx.iroh().endpoint_id();

        //
        v_flex()
            .debug()
            .size_full()
            //
            .child(
                //
                div()
                    //
                    .p_2()
                    .text_lg()
                    .child("Connections"),
            )
            .child(
                //
                div()
                    //
                    .p_2()
                    .child(
                        h_flex()
                            .id("copy-endpoint-id")
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
                            .when_some(endpoint_id.clone(), |el, endpoint_id| {
                                el.tooltip(Tooltip::text(format!("ID: {endpoint_id}")))
                            })
                            .on_click(cx.listener(|_this, _e, _window, cx| {
                                let Some(endpoint_id) = cx.iroh().endpoint_id() else {
                                    warn!("Iroh Endpoint not available");
                                    return;
                                };
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    endpoint_id.to_string(),
                                ));
                            }))
                            //
                            .p_1()
                            .bg(cx.theme().colors().panel_background)
                            .rounded_md()
                            .child(Icon::new(IconName::Copy))
                            .child("Copy Endpoint Ticket"),
                    ),
            )
            .child(
                //
                div()
                    //
                    .p_2()
                    .child(self.input_ticket.clone()),
            )
            .child(
                //
                div()
                    .debug()
                    .size_full()
                    //
                    .p_2()
                    .child(
                        div()
                            //
                            .when_none(&cx.iroh().remote_peers(), |el| {
                                el
                                    //
                                    .child("No remote peers")
                            })
                            .when_some(cx.iroh().remote_peers(), |el, peers| {
                                //
                                el
                                    //
                                    .children(peers.iter().map(|it| {
                                        div()
                                            //
                                            .p_2()
                                            .child(it.to_string())
                                    }))
                            }),
                    ),
            )
    }
}
