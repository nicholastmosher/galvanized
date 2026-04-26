// The Connections UI will initially be Iroh peer connections
//
// This UI is in charge of:
// - Displaying a list of connected peers
// - Allowing adding new peers
// - Allowing removing peers

use std::{ops::Not, path::PathBuf};

use anyhow::{Context as _, anyhow, bail};
use iroh::{EndpointAddr, EndpointId};
use tracing::info;
use zed::unstable::{
    editor::Editor,
    gpui::{AppContext as _, ClickEvent, ClipboardItem, Entity, Global, KeyDownEvent, img},
    ui::{
        ActiveTheme as _, App, Context, FluentBuilder, Icon, IconName, IconSize,
        InteractiveElement as _, IntoElement, ListSeparator, ParentElement as _, Render,
        SharedString, StatefulInteractiveElement as _, Styled, Tooltip, Window, div, h_flex, px,
        v_flex,
    },
    ui_input::InputField,
    util::ResultExt as _,
    workspace::Workspace,
};

use crate::Ticket;
use plugin_chat::ChatUi;
use plugin_iroh::IrohExt as _;

struct GlobalWorkspace(Entity<Workspace>);
impl Global for GlobalWorkspace {}
pub fn init(cx: &mut App) {
    // Store the workspace entity in a local global (lol) to pass it to ConnectionsUi construction
    cx.observe_new::<Workspace>(|_workspace, _window, cx| {
        let workspace_entity = cx.entity();
        cx.set_global(GlobalWorkspace(workspace_entity));
    })
    .detach();
}

pub struct ConnectionsUi {
    input_local_name: Entity<InputField>,
    input_ticket: Entity<Editor>,
    workspace: Entity<Workspace>,
}

impl ConnectionsUi {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_ticket = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("Paste remote ticket", window, cx);
            editor
        });
        let input_local_name = cx.new(|cx| InputField::new(window, cx, "Local peer name"));
        let workspace = cx.global::<GlobalWorkspace>().0.clone();
        Self {
            input_ticket,
            input_local_name,
            workspace,
        }
    }
}

impl Render for ConnectionsUi {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            // .debug()
            .size_full()
            //
            .p_2()
            .gap_2()
            .child(self.render_connection_header(window, cx))
            .child(ListSeparator)
            .child(self.render_connected_peers(window, cx))
    }
}

impl ConnectionsUi {
    fn render_connection_header(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let endpoint_id = cx.iroh().endpoint_id().ok();

        v_flex()
            //
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
                                div()
                                    .when_none(&endpoint_id, |el| {
                                        //
                                        el.child("Iroh not yet initialized")
                                    })
                                    .when_some(endpoint_id, |el, endpoint_id| {
                                        //
                                        el.child(
                                            //
                                            h_flex()
                                                .id("local-endpoint-id")
                                                .text_color(cx.theme().colors().text_muted)
                                                .hover(|style| {
                                                    style.text_color(cx.theme().colors().text)
                                                })
                                                .tooltip(Tooltip::text("Copy peer ticket"))
                                                .on_click(cx.listener(
                                                    move |_this, _e, _window, cx| {
                                                        let endpoints =
                                                            vec![EndpointAddr::from_parts(
                                                                endpoint_id,
                                                                [],
                                                            )];
                                                        let ticket = Ticket { endpoints };
                                                        let ticket_text = ticket.to_string();
                                                        cx.write_to_clipboard(
                                                            ClipboardItem::new_string(ticket_text),
                                                        );
                                                    },
                                                ))
                                                .child(
                                                    Icon::new(IconName::Copy)
                                                        .size(IconSize::Medium),
                                                )
                                                .child(div().px_1())
                                                .child(
                                                    //
                                                    div()
                                                        //
                                                        .text_sm()
                                                        .child({
                                                            let mut string =
                                                                endpoint_id.to_string();
                                                            let suffix =
                                                                string.split_off(string.len() - 8);
                                                            suffix
                                                        }),
                                                ),
                                        )
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
                        if e.keystroke.key != "enter" {
                            return;
                        }
                        this.connect_peer(window, cx);
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
    }

    fn render_connected_peers(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // let peers = cx.iroh().galvanized().remote_peers();
        let peers = cx.iroh().galvanized().ok().map(|gzed| gzed.remote_peers());

        div()
            .size_full()
            //
            .child(
                div()
                    //
                    .when_none(&peers, |el| {
                        el
                            //
                            .child("No remote peers")
                    })
                    .when_some(peers, |el, peers| {
                        //
                        el
                            //
                            .children(peers.iter().map(|endpoint_id| {
                                h_flex()
                                    .id(format!("remote-peer-{endpoint_id}"))
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
                                    // Connect to the clicked peer
                                    // - Create a document for the chat
                                    // -
                                    .on_click({
                                        let endpoint_id = *endpoint_id;
                                        cx.listener(move |this, e, window, cx| {
                                            //
                                            this.open_chat(endpoint_id, e, window, cx);
                                        })
                                    })
                                    .child({
                                        //
                                        let mut string = endpoint_id.to_string();
                                        let suffix = string.split_off(string.len() - 8);
                                        SharedString::from(suffix)
                                    })
                                    .child(div().flex_grow())
                                    .child(
                                        //
                                        div()
                                            //
                                            .child(
                                                //
                                                img(PathBuf::from(".assets/chat-bubble.svg"))
                                                    .size(px(24.)),
                                            ),
                                    )
                            }))
                    }),
            )
    }

    /// Attempt to connect to a peer using the ticket pasted by the user
    fn connect_peer(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let ticket_text = self.input_ticket.read(cx).text(cx);
        let ticket = ticket_text
            .parse::<Ticket>()
            .map_err(|e| anyhow!("failed to parse Ticket: {e}"))
            .log_err();
        let Some(ticket) = ticket else {
            return;
        };
        let Some(addr) = ticket.endpoints.get(0).cloned() else {
            return;
        };

        cx.spawn(async move |_this, cx| {
            let Some(gzed) = cx.iroh().galvanized().ok() else {
                bail!("Iroh not yet initialized");
            };
            gzed.connect(addr).await?;
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);

        // let chat = cx.new(|cx| ChatUi::new(endpoint_addr.id, window, cx));
        // this.workspace.update(cx, |workspace, cx| {
        //     workspace.add_item_to_active_pane(
        //         Box::new(chat),
        //         Some(0),
        //         true,
        //         window,
        //         cx,
        //     );
        // });
    }

    fn open_chat(
        &mut self,
        endpoint_id: EndpointId,
        _e: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |this_weak, cx| {
            let gzed = cx
                .iroh()
                .galvanized()
                .context("Iroh not initialized when trying to open chat")?;

            let doc_handle = gzed.create_or_open_doc(&endpoint_id).await?;
            info!(
                doc_id = ?doc_handle.document_id(),
                "Successfully got doc_handle for peer"
            );

            cx.update(|window, cx| {
                let workspace = this_weak.read_with(cx, |it, _cx| it.workspace.clone())?;
                let chat_ui = cx.new(|cx| ChatUi::new(endpoint_id, doc_handle, window, cx));
                workspace.update(cx, |workspace, cx| {
                    workspace.add_item_to_active_pane(Box::new(chat_ui), Some(0), true, window, cx);
                });

                anyhow::Ok(())
            })??;

            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }
}
