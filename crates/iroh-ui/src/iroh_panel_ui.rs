use std::collections::HashMap;

use anyhow::bail;
use iroh::Endpoint;
use iroh::protocol::{ProtocolHandler, Router};
use iroh_blobs::store::mem::MemStore;
use iroh_blobs::{ALPN as BLOBS_ALPN, BlobsProtocol};
use iroh_docs::{ALPN as DOCS_ALPN, protocol::Docs};
use iroh_gossip::{ALPN as GOSSIP_ALPN, Gossip, TopicId};
use tracing::{info, warn};
use zed::unstable::editor::Editor;
use zed::unstable::gpui::{
    self, App, AppContext as _, ClipboardItem, Context, Entity, EventEmitter, FocusHandle,
    Focusable, ParentElement as _, Render, Styled, Window, div,
};
use zed::unstable::ui::{
    Button, Clickable, FluentBuilder, IconPosition, IconSize, IntoElement, LabelSize, ListItem,
    Pixels,
};
use zed::unstable::workspace::Workspace;
use zed::unstable::workspace::dock::PanelEvent;
use zed::unstable::{
    gpui::{actions, px},
    workspace::{Panel, dock::DockPosition, ui::IconName},
};

use crate::iroh_topic_chat_ui::TopicChatUi;
use crate::{DebugViewExt as _, Ticket};

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, window, cx| {
        let Some(window) = window else {
            return;
        };

        workspace.register_action(|workspace, _: &ToggleIrohPanel, window, cx| {
            workspace.toggle_panel_focus::<IrohPanel>(window, cx);
        });

        let workspace_entity = cx.entity();
        let iroh_panel = cx.new(|cx| IrohPanel::new(workspace_entity, window, cx));
        workspace.add_panel(iroh_panel, window, cx);
    })
    .detach();
}

actions!(workspace, [ToggleIrohPanel]);

#[allow(unused)]
#[non_exhaustive]
#[derive(Clone)]
pub struct Iroh {
    pub endpoint: Endpoint,
    pub router: Router,
    pub blobs: BlobsProtocol,
    pub gossip: Gossip,
    pub docs: Docs,
}

pub struct IrohPanel {
    dock_position: DockPosition,
    focus_handle: FocusHandle,
    remote_ticket_editor: Entity<Editor>,
    iroh: Option<Iroh>,
    spaces: Vec<String>,
    topics: HashMap<TopicId, (Ticket, Entity<TopicChatUi>)>,
    width: Option<Pixels>,
    workspace: Entity<Workspace>,
}

#[derive(Debug, Clone)]
struct Handler;
impl Handler {
    const APLN: &[u8] = b"/test/docs";
}
impl ProtocolHandler for Handler {
    fn accept(
        &self,
        _connection: iroh::endpoint::Connection,
    ) -> impl Future<Output = Result<(), iroh::protocol::AcceptError>> + Send {
        async move {
            //
            info!("Accepted inbound connection");
            Ok(())
        }
    }
}

impl IrohPanel {
    pub fn new(workspace: Entity<Workspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Initialize Iroh endpoint
        cx.spawn({
            async move |panel, cx| {
                let Some(panel) = panel.upgrade() else {
                    bail!("iroh panel not found");
                };

                // let mdns = iroh::discovery::mdns::MdnsDiscovery::builder();
                let endpoint = Endpoint::builder()
                    //
                    // .discovery(mdns)
                    .bind()
                    .await?;
                let store = MemStore::new();
                let blobs = BlobsProtocol::new(&store, None);
                let gossip = iroh_gossip::Gossip::builder().spawn(endpoint.clone());
                let docs = Docs::memory()
                    .spawn(endpoint.clone(), (*blobs).clone(), gossip.clone())
                    .await?;
                let router = Router::builder(endpoint.clone())
                    .accept(BLOBS_ALPN, blobs.clone())
                    .accept(GOSSIP_ALPN, gossip.clone())
                    .accept(DOCS_ALPN, docs.clone())
                    .accept(Handler::APLN, Handler)
                    .spawn();
                panel.update(cx, move |panel, _cx| {
                    panel.iroh = Some(Iroh {
                        endpoint,
                        router,
                        blobs,
                        gossip,
                        docs,
                    });
                })?;

                anyhow::Ok(())
            }
        })
        .detach();

        let remote_endpoint_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("Remote endpoint URL", window, cx);
            editor
        });

        Self {
            dock_position: DockPosition::Left,
            focus_handle: cx.focus_handle(),
            remote_ticket_editor: remote_endpoint_editor,
            iroh: None,
            spaces: vec!["Home".to_string(), "Family".to_string(), "Work".to_string()],
            topics: Default::default(),
            width: None,
            workspace,
        }
    }

    fn render_namespace_bar(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div().children(self.spaces.iter().enumerate().map(|(i, it)| {
            div().p_2().child(
                ListItem::new(i)
                    .rounded()
                    .child(div().p_4().child(it.to_string())),
            )
        }))
    }

    fn render_widget_feed(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .size_full()
            .debug_border()
            .flex_col()
            .child(self.render_local_endpoint(window, cx))
            .child(self.render_create_topic(window, cx))
            .child(self.render_topics(window, cx))
            .child(self.render_connect_remote(window, cx))
    }

    fn render_local_endpoint(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            //
            .p_1()
            .flex_row()
            .debug_border()
            .when_some(self.iroh.as_ref(), |div, Iroh { endpoint, .. }| {
                //
                div
                    //
                    .child(
                        Button::new(
                            "endpoint-id",
                            format!("Endpoint ID: .+{:.8}", endpoint.id().to_string()),
                        )
                        .label_size(LabelSize::Small)
                        .icon(IconName::Copy)
                        .icon_size(IconSize::Small)
                        .icon_position(IconPosition::Start)
                        .on_click(cx.listener(|this, _, _window, cx| {
                            let Some(Iroh { endpoint, .. }) = &this.iroh else {
                                return;
                            };

                            let text = endpoint.id().to_string();
                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                            info!("Clicked Copy");
                        })),
                    )
                    .child(
                        Button::new("endpoint-addr", "Endpoint Addr")
                            .label_size(LabelSize::Small)
                            .icon(IconName::Copy)
                            .icon_size(IconSize::Small)
                            .icon_position(IconPosition::Start)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                let Some(Iroh { endpoint, .. }) = &this.iroh else {
                                    return;
                                };

                                let text = format!("{:?}", endpoint.addr());
                                cx.write_to_clipboard(ClipboardItem::new_string(text));
                                info!("Clicked Copy");
                            })),
                    )
            })
    }

    fn render_create_topic(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .p_1()
            .debug_border()
            .flex()
            .gap_2()
            // .child(self.create_topic_editor.clone())
            .child(
                Button::new("create-topic", "Create Topic")
                    .label_size(LabelSize::Small)
                    .icon(IconName::Plus)
                    .icon_size(IconSize::Small)
                    .icon_position(IconPosition::Start)
                    .on_click(cx.listener(Self::click_create_topic)),
            )
    }

    fn render_topics(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_grow()
            .child("Topics:")
            .children(self.topics.iter().map(|(topic, (ticket, _ui))| {
                div()
                    .pl_2()
                    .child(
                        Button::new("topic-{topic}", {
                            let topic = topic.to_string();
                            format!("Copy Topic .+{}", &topic[topic.len() - 8..])
                        })
                        .label_size(LabelSize::Small)
                        .icon(IconName::Copy)
                        .icon_size(IconSize::Small)
                        .icon_position(IconPosition::Start)
                        .on_click(cx.listener({
                            let topic = topic.to_string();
                            move |_this, _, _window, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(topic.to_string()));
                                info!("Clicked Copy Topic");
                            }
                        })),
                    )
                    .child(
                        Button::new("topic-ticket-{ticket}", {
                            let ticket = ticket.to_string();
                            format!("Copy Ticket .+{}", &ticket[ticket.len() - 8..])
                        })
                        .label_size(LabelSize::Small)
                        .icon(IconName::Copy)
                        .icon_size(IconSize::Small)
                        .icon_position(IconPosition::Start)
                        .on_click(cx.listener({
                            let ticket = ticket.to_string();
                            move |_this, _, _window, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    ticket.to_string(),
                                ));
                                info!("Clicked Copy Ticket");
                            }
                        })),
                    )
            }))
    }

    fn render_connect_remote(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .p_1()
            .debug_border()
            .flex()
            .gap_2()
            .child(self.remote_ticket_editor.clone())
            .child(
                Button::new("connect-remote", "Connect")
                    .label_size(LabelSize::Small)
                    .icon(IconName::Plus)
                    .icon_size(IconSize::Small)
                    .icon_position(IconPosition::Start)
                    .on_click(cx.listener(Self::click_connect_remote)),
            )
    }

    fn click_create_topic(
        &mut self,
        _event: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(iroh) = self.iroh.as_ref() else {
            return;
        };

        let topic_id = TopicId::from_bytes(rand::random());
        let me = self.iroh.as_ref().unwrap().endpoint.addr();
        let ticket = Ticket {
            topic_id,
            endpoints: vec![me],
        };
        let topic_name = topic_id.to_string();
        let workspace = self.workspace.clone();
        let topic_id = TopicId::from_bytes(rand::random());
        let topic_chat_ui =
            cx.new(|cx| TopicChatUi::new(iroh.clone(), topic_id, topic_name, window, cx));

        workspace.update(cx, |workspace, cx| {
            //
            workspace.add_item_to_active_pane(
                Box::new(topic_chat_ui.clone()),
                Some(0),
                true,
                window,
                cx,
            );
        });

        //

        self.topics.insert(topic_id, (ticket, topic_chat_ui));

        info!("Clicked Create Topic");
    }

    fn click_connect_remote(
        &mut self,
        _event: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(iroh) = self.iroh.as_ref() else {
            return;
        };

        let ticket_text = self.remote_ticket_editor.read(cx).text(cx);
        let ticket_result = ticket_text.parse::<Ticket>();
        let ticket = match ticket_result {
            Ok(ticket) => ticket,
            Err(error) => {
                warn!("failed to parse ticket: {error}");
                return;
            }
        };

        let workspace = self.workspace.clone();
        let topic_chat_ui = cx.new(|cx| {
            TopicChatUi::new(
                iroh.clone(),
                ticket.topic_id,
                ticket.topic_id.to_string(),
                window,
                cx,
            )
        });

        workspace.update(cx, |workspace, cx| {
            workspace.add_item_to_active_pane(
                Box::new(topic_chat_ui.clone()),
                Some(0),
                false,
                window,
                cx,
            );
        });

        self.topics.insert(ticket.topic_id, (ticket, topic_chat_ui));

        // cx.spawn({
        //     let iroh = iroh.clone();
        //     async move |_this, cx| {
        //         //
        //         let endpoint_ids = ticket.endpoints.iter().map(|it| it.id).collect();
        //         let (sender, mut receiver) = iroh
        //             .gossip
        //             .subscribe_and_join(ticket.topic_id, endpoint_ids)
        //             .await?
        //             .split();

        //         // Receiver
        //         cx.spawn({
        //             //
        //             async move |_cx| {
        //                 //
        //                 while let Some(event) = receiver.try_next().await? {
        //                     let iroh_gossip::api::Event::Received(message) = event else {
        //                         continue;
        //                     };

        //                     let bytes = message.content;
        //                     let text = String::from_utf8_lossy(&bytes);
        //                     info!(%text, "Received:")
        //                 }

        //                 warn!("Receiver task quit");
        //                 anyhow::Ok(())
        //             }
        //         })
        //         .detach();

        //         let me = iroh.endpoint.id();
        //         sender
        //             .broadcast(Bytes::from(format!("{me} joined")))
        //             .await?;
        //         anyhow::Ok(())
        //     }
        // })
        // .detach_and_log_err(cx);

        info!("Clicked Connect");
    }
}

impl EventEmitter<PanelEvent> for IrohPanel {}

impl Focusable for IrohPanel {
    fn focus_handle(&self, _cx: &gpui::App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for IrohPanel {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        // Panel root
        div()
            .h_full()
            .w(self.width.unwrap_or(px(300.)))
            .flex()
            .flex_row()
            .debug_border()
            // Left vertical sidebar
            .child(self.render_namespace_bar(window, cx))
            // Right vertical list of widgets
            .child(self.render_widget_feed(window, cx))
    }
}

impl Panel for IrohPanel {
    fn persistent_name() -> &'static str {
        "Iroh"
    }

    fn panel_key() -> &'static str {
        "iroh-panel"
    }

    fn position(
        &self,
        _window: &zed::unstable::gpui::Window,
        _cx: &zed::unstable::gpui::App,
    ) -> zed::unstable::workspace::dock::DockPosition {
        self.dock_position
    }

    fn position_is_valid(&self, _position: zed::unstable::workspace::dock::DockPosition) -> bool {
        true
    }

    fn set_position(
        &mut self,
        position: zed::unstable::workspace::dock::DockPosition,
        _window: &mut zed::unstable::gpui::Window,
        _cx: &mut zed::unstable::gpui::Context<Self>,
    ) {
        self.dock_position = position;
    }

    fn size(
        &self,
        _window: &zed::unstable::gpui::Window,
        _cx: &zed::unstable::gpui::App,
    ) -> Pixels {
        self.width.unwrap_or(px(300.))
    }

    fn set_size(
        &mut self,
        size: Option<Pixels>,
        _window: &mut zed::unstable::gpui::Window,
        _cx: &mut zed::unstable::gpui::Context<Self>,
    ) {
        self.width = size.map(|it| it - px(1.));
    }

    fn icon(
        &self,
        _window: &zed::unstable::gpui::Window,
        _cx: &zed::unstable::gpui::App,
    ) -> Option<zed::unstable::workspace::ui::IconName> {
        Some(IconName::Link)
    }

    fn icon_tooltip(
        &self,
        _window: &zed::unstable::gpui::Window,
        _cx: &zed::unstable::gpui::App,
    ) -> Option<&'static str> {
        Some("Iroh")
    }

    fn toggle_action(&self) -> Box<dyn zed::unstable::gpui::Action> {
        Box::new(ToggleIrohPanel)
    }

    fn activation_priority(&self) -> u32 {
        0
    }
}
