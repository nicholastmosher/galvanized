// The Connections UI will initially be Iroh peer connections
//
// This UI is in charge of:
// - Displaying a list of connected peers
// - Allowing adding new peers
// - Allowing removing peers

use std::path::PathBuf;

use anyhow::anyhow;
use iroh::EndpointAddr;
use tracing::{info, warn};
use zed::unstable::{
    editor::Editor,
    gpui::{AppContext as _, ClipboardItem, Entity, KeyDownEvent, img},
    ui::{
        ActiveTheme as _, App, Context, FluentBuilder, Icon, IconName, IconSize,
        InteractiveElement as _, IntoElement, ListSeparator, ParentElement as _, Render,
        SharedString, StatefulInteractiveElement as _, Styled, Tooltip, Window, div, h_flex, px,
        v_flex,
    },
    ui_input::InputField,
    util::ResultExt as _,
};

use crate::{Ticket, iroh::IrohExt};

pub fn init(cx: &mut App) {
    //
}

pub struct ConnectionsUi {
    //
    input_local_name: Entity<InputField>,
    // input_ticket: Entity<InputField>,
    input_ticket: Entity<Editor>,
}

impl ConnectionsUi {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        //
        // let input_ticket = cx.new(|cx| InputField::new(window, cx, "Paste remote ticket"));
        let input_ticket = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("Paste remote ticket", window, cx);
            editor
        });
        let input_local_name = cx.new(|cx| InputField::new(window, cx, "Local peer name"));
        Self {
            input_ticket,
            input_local_name,
        }
    }
}

impl Render for ConnectionsUi {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let endpoint_id = cx.iroh().endpoint_id();

        //
        v_flex()
            // .debug()
            .size_full()
            //
            .p_2()
            .gap_2()
            .child(
                //
                h_flex()
                    //
                    .pl_2()
                    .gap_2()
                    .text_lg()
                    .child(
                        //
                        img(PathBuf::from(".assets/connect-peers.svg"))
                            //
                            .size(px(32.)),
                    )
                    .child(
                        //
                        v_flex()
                            //
                            .child("Connections")
                            .child(
                                //
                                h_flex()
                                    .id("local-endpoint-id")
                                    .text_color(cx.theme().colors().text_muted)
                                    .when_some(endpoint_id.clone(), |el, endpoint_id| {
                                        //
                                        el
                                            //
                                            .hover(|style| {
                                                style.text_color(cx.theme().colors().text)
                                            })
                                            .tooltip(Tooltip::text("Copy peer ticket"))
                                            .on_click(cx.listener(move |_this, _e, _window, cx| {
                                                let endpoints =
                                                    vec![EndpointAddr::from_parts(endpoint_id, [])];
                                                let ticket = Ticket { endpoints };
                                                let ticket_text = ticket.to_string();
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    ticket_text,
                                                ));
                                            }))
                                            .child(Icon::new(IconName::Copy).size(IconSize::Medium))
                                            .child(div().px_1())
                                            .child(
                                                //
                                                div()
                                                    //
                                                    .text_sm()
                                                    .child({
                                                        let mut string = endpoint_id.to_string();
                                                        let suffix =
                                                            string.split_off(string.len() - 8);
                                                        suffix
                                                    }),
                                            )
                                    })
                                    .when_none(&endpoint_id, |el| {
                                        //
                                        el.child("Local endpoint unavailable")
                                    }),
                            ),
                    ),
            )
            .child(
                //
                h_flex()
                    // .debug()
                    //
                    .border_1()
                    .border_color(cx.theme().colors().border)
                    .rounded_md()
                    .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                        info!(?e, "KEYDOWN");

                        if e.keystroke.key == "enter" {
                            info!("Do the thing on ENTER");

                            let ticket_text = this.input_ticket.read(cx).text(cx);
                            let ticket = ticket_text
                                .parse::<Ticket>()
                                .map_err(|e| anyhow!("failed to parse Ticket: {e}"))
                                .log_err();
                            let Some(ticket) = ticket else {
                                return;
                            };
                            let Some(endpoint_addr) = ticket.endpoints.get(0) else {
                                return;
                            };

                            cx.iroh().connect(cx, endpoint_addr.clone());
                        }
                    }))
                    .child(
                        //
                        div()
                            .flex_grow()
                            //
                            .p_2()
                            .child(self.input_ticket.clone()),
                    ),
            )
            .child(ListSeparator)
            .child(
                //
                div()
                    // .debug()
                    .size_full()
                    //
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
                                        h_flex()
                                            .id(format!("remote-peer-{it}"))
                                            //
                                            .p_2()
                                            .rounded_md()
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
                                            .child({
                                                //
                                                let mut string = it.to_string();
                                                let suffix = string.split_off(string.len() - 8);
                                                SharedString::from(suffix)
                                            })
                                            .child(div().flex_grow())
                                            .child(
                                                //
                                                img(PathBuf::from(".assets/chat-bubble.svg"))
                                                    .size(px(24.)),
                                            )
                                    }))
                            }),
                    ),
            )
    }
}
