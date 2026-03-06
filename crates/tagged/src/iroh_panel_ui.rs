use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock};

use anyhow::bail;
use automerge::Automerge;
use autosurgeon::reconcile;
use iroh::endpoint::Connection;
use iroh::protocol::{AcceptError, ProtocolHandler, Router};
use iroh::{Endpoint, EndpointAddr};
use iroh_automerge_repo::IrohRepo;
use iroh_blobs::store::mem::MemStore;
use iroh_blobs::{ALPN as BLOBS_ALPN, BlobsProtocol};
use iroh_docs::{ALPN as DOCS_ALPN, protocol::Docs};
use iroh_gossip::{ALPN as GOSSIP_ALPN, Gossip, TopicId};
use samod::storage::TokioFilesystemStorage;
use samod::{PeerId, Repo};
use tracing::{info, warn};
use zed::unstable::editor::Editor;
use zed::unstable::gpui::{
    self, App, AppContext as _, ClickEvent, ClipboardItem, Context, Entity, EventEmitter,
    FocusHandle, Focusable, ParentElement as _, Render, Styled, Task, Window, div,
};
use zed::unstable::ui::{
    Button, Clickable, FluentBuilder, IconPosition, IconSize, IntoElement, LabelSize, ListItem,
    Pixels, SharedString,
};
use zed::unstable::workspace::Workspace;
use zed::unstable::workspace::dock::PanelEvent;
use zed::unstable::{
    gpui::{actions, px},
    workspace::{Panel, dock::DockPosition, ui::IconName},
};

use crate::iroh_automerge_chat_ui::{AutomergeChatUi, AutomergeTicket, DocContent};
use crate::iroh_topic_chat_ui::TopicChatUi;
use crate::{DebugViewExt as _, Ticket};

actions!(workspace, [ToggleIrohPanel]);

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

#[allow(unused)]
#[non_exhaustive]
#[derive(Clone)]
pub struct Iroh {
    pub automerge: IrohRepo,
    pub endpoint: Endpoint,
    pub router: Router,
    pub blobs: BlobsProtocol,
    pub gossip: Gossip,
    pub docs: Docs,
    pub handler: CustomHandler,
}

impl Iroh {
    async fn try_new() -> anyhow::Result<Iroh> {
        // let mdns = iroh::discovery::mdns::MdnsDiscovery::builder();
        let endpoint = Endpoint::builder()
            //
            // .discovery(mdns)
            .bind()
            .await?;
        let tagt_dir = tagt_dir();
        let store = MemStore::new();
        let blobs = BlobsProtocol::new(&store, None);
        let gossip = iroh_gossip::Gossip::builder().spawn(endpoint.clone());
        let docs = Docs::persistent(tagt_dir.join("iroh-docs"))
            .spawn(endpoint.clone(), (*blobs).clone(), gossip.clone())
            .await?;

        let handler = CustomHandler::new();
        let repo = Repo::build_tokio()
            .with_peer_id(PeerId::from_string(endpoint.id().to_string()))
            .with_storage(TokioFilesystemStorage::new(tagt_dir.join("samod-repo")))
            .load()
            .await;
        let automerge = IrohRepo::new(endpoint.clone(), repo);
        let router = Router::builder(endpoint.clone())
            .accept(BLOBS_ALPN, blobs.clone())
            .accept(GOSSIP_ALPN, gossip.clone())
            .accept(DOCS_ALPN, docs.clone())
            .accept(IrohRepo::SYNC_ALPN, automerge.clone())
            .accept(CustomHandler::APLN, handler.clone())
            .spawn();
        let iroh = Iroh {
            automerge,
            endpoint,
            router,
            blobs,
            gossip,
            docs,
            handler,
        };

        Ok(iroh)
    }
}

pub struct IrohPanel {
    docs: Vec<Entity<AutomergeChatUi>>,
    dock_position: DockPosition,
    focus_handle: FocusHandle,
    iroh: Option<Iroh>,
    per_endpoint_state: HashMap<EndpointAddr, EndpointState>,
    remote_doc_editor: Entity<Editor>,
    remote_topic_editor: Entity<Editor>,
    spaces: Vec<String>,
    topics: HashMap<TopicId, Entity<TopicChatUi>>,
    width: Option<Pixels>,
    workspace: Entity<Workspace>,
}

#[derive(Default)]
pub struct EndpointState {
    chat_ui: Option<Entity<AutomergeChatUi>>,
    doc_lookup_task: Option<Task<anyhow::Result<()>>>,
    doc_sync_task: Option<Task<anyhow::Result<()>>>,
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct CustomHandler(Arc<HandlerState>);
#[derive(Debug)]
struct HandlerState {
    //
}

impl CustomHandler {
    const APLN: &[u8] = b"/test/handler";

    pub fn new() -> Self {
        Self(Arc::new(HandlerState {}))
    }
}

impl ProtocolHandler for CustomHandler {
    fn accept(
        &self,
        connection: Connection,
    ) -> impl Future<Output = Result<(), AcceptError>> + Send {
        async move {
            let (_sender, _recvv) = connection.accept_bi().await?;
            info!("Accepted inbound connection");
            Ok(())
        }
    }
}

// TODO caching etc
fn tagt_dir() -> PathBuf {
    static TAGT_INSTANCE: LazyLock<String> = LazyLock::new(|| {
        let instance = std::env::var("TAGT_INSTANCE").unwrap_or_else(|_| "0".to_string());
        info!("Tagt Instance: {}", instance);
        instance
    });

    let tagt_dir = zed::unstable::paths::data_dir()
        .join("tagt")
        .join(&*TAGT_INSTANCE);
    std::fs::create_dir_all(&tagt_dir).expect("create zed/tagt directory");
    std::fs::create_dir_all(tagt_dir.join("iroh-docs"))
        .expect("create zed/tagt/iroh-docs directory");
    std::fs::create_dir_all(tagt_dir.join("samod-repo"))
        .expect("create zed/tagt/samod-repo directory");
    tagt_dir
}

impl IrohPanel {
    pub fn new(workspace: Entity<Workspace>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Initialize Iroh endpoint
        cx.spawn({
            async move |panel, cx| {
                let Some(panel) = panel.upgrade() else {
                    bail!("iroh panel not found");
                };

                let iroh = Iroh::try_new().await?;
                panel.update(cx, move |panel, _cx| {
                    panel.iroh = Some(iroh);
                })?;

                anyhow::Ok(())
            }
        })
        .detach();

        let remote_topic_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("Topic ticket", window, cx);
            editor
        });

        let remote_doc_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("Doc ticket", window, cx);
            editor
        });

        Self {
            docs: Default::default(),
            dock_position: DockPosition::Left,
            focus_handle: cx.focus_handle(),
            iroh: None,
            per_endpoint_state: Default::default(),
            remote_doc_editor,
            remote_topic_editor,
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
        div()
            //
            .children(self.spaces.iter().enumerate().map(|(i, it)| {
                div()
                    //
                    .p_2()
                    .child(
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
            .child(self.render_connect_topic(window, cx))
            .child(self.render_create_doc(window, cx))
            .child(self.render_docs(window, cx))
            .child(self.render_connect_doc(window, cx))
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
            .children(self.topics.keys().map(|topic_id| {
                div().pl_2().child(
                    Button::new(SharedString::from(format!("topic-{topic_id}")), {
                        let topic = topic_id.to_string();
                        format!("Copy Ticket .+{}", &topic[topic.len() - 8..])
                    })
                    .label_size(LabelSize::Small)
                    .icon(IconName::Copy)
                    .icon_size(IconSize::Small)
                    .icon_position(IconPosition::Start)
                    .on_click(cx.listener(Self::click_copy_ticket(*topic_id))),
                )
            }))
    }

    fn render_connect_topic(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .p_1()
            .debug_border()
            .flex()
            .gap_2()
            .child(self.remote_topic_editor.clone())
            .child(
                Button::new("connect-topic", "Connect")
                    .label_size(LabelSize::Small)
                    .icon(IconName::Plus)
                    .icon_size(IconSize::Small)
                    .icon_position(IconPosition::Start)
                    .on_click(cx.listener(Self::click_connect_topic)),
            )
    }

    fn render_connect_doc(&self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .p_1()
            .debug_border()
            .flex()
            .gap_2()
            .child(self.remote_doc_editor.clone())
            .child(
                Button::new("connect-doc", "Connect Doc")
                    .label_size(LabelSize::Small)
                    .icon(IconName::Plus)
                    .icon_size(IconSize::Small)
                    .icon_position(IconPosition::Start)
                    .on_click(cx.listener(Self::click_connect_doc)),
            )
    }

    fn render_create_doc(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .p_1()
            .debug_border()
            .flex()
            .gap_2()
            // .child(self.remote_ticket_editor.clone())
            .child(
                Button::new("create-doc", "Create doc")
                    .label_size(LabelSize::Small)
                    .icon(IconName::Plus)
                    .icon_size(IconSize::Small)
                    .icon_position(IconPosition::Start)
                    .on_click(cx.listener(Self::click_create_doc)),
            )
    }

    fn render_docs(
        //
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div().children(self.docs.iter().enumerate().map(|(i, ui)| {
            let id = ui.read(cx).doc.document_id();
            div().child(
                ListItem::new(SharedString::from(format!("doc-{i}")))
                    .child(format!("Doc ID: {id}"))
                    .on_click(cx.listener(Self::click_doc(ui.clone()))),
            )
        }))
    }

    fn click_copy_ticket(
        topic_id: TopicId,
    ) -> impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) {
        move |this, _, _window, cx| {
            let Some(me) = this.iroh.as_ref().map(|it| it.endpoint.addr()) else {
                warn!("missing iroh on copy ticket");
                return;
            };
            let ticket = Ticket {
                topic_id,
                endpoints: vec![me],
            };
            cx.write_to_clipboard(ClipboardItem::new_string(ticket.to_string()));
            info!("Clicked Copy Ticket");
        }
    }

    fn click_create_topic(
        &mut self,
        _event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let topic_id = TopicId::from_bytes(rand::random());
        let endpoints = vec![];
        self.create_topic_ui(topic_id, endpoints, window, cx);
        info!("Clicked Create Topic");
    }

    fn click_connect_topic(
        &mut self,
        _event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ticket_text = self.remote_topic_editor.read(cx).text(cx);
        let ticket_result = ticket_text.parse::<Ticket>();
        let ticket = match ticket_result {
            Ok(ticket) => ticket,
            Err(error) => {
                warn!("failed to parse ticket: {error}");
                return;
            }
        };

        self.create_topic_ui(ticket.topic_id, ticket.endpoints, window, cx);
        info!("Clicked Connect");
    }

    fn click_connect_doc(
        &mut self,
        _event: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(iroh) = self.iroh.clone() else {
            warn!("missing Iroh instance");
            return;
        };

        let ticket_text = self.remote_doc_editor.read(cx).text(cx);
        let ticket_result = ticket_text.parse::<AutomergeTicket>();
        let ticket = match ticket_result {
            Ok(ticket) => ticket,
            Err(error) => {
                warn!("failed to parse ticket: {error}");
                return;
            }
        };

        info!("Ticket endpoints: {:?}", ticket.endpoints);
        for endpoint_addr in ticket.endpoints {
            let doc_lookup_task = cx.spawn({
                let iroh = iroh.clone();
                let doc_id = ticket.doc_id.clone();
                let endpoint_addr = endpoint_addr.clone();
                async move |ui, cx| {
                    let Some(ui) = ui.upgrade() else {
                        bail!("connect-docs: missing Entity<IrohPanel>");
                    };

                    // Spawn sync task
                    let sync_task = cx.spawn({
                        let iroh = iroh.clone();
                        let endpoint_addr = endpoint_addr.clone();
                        async move |_cx| {
                            info!(?endpoint_addr, "Starting automerge sync");
                            let reason = iroh.automerge.sync_with(endpoint_addr).await?;
                            warn!(?reason, "Stopped automerge sync");
                            anyhow::Ok(())
                        }
                    });

                    info!("Connecting to automerge repo peer");
                    iroh.automerge
                        .repo()
                        .when_connected(PeerId::from_string(endpoint_addr.id.to_string()))
                        .await?;
                    info!("Connected to automerge repo peer");

                    // Search repo network for the document
                    info!("Searching repo network for document {doc_id}");
                    let Some(doc_handle) = iroh.automerge.repo().find(doc_id.clone()).await? else {
                        bail!("failed to find document: {doc_id}");
                    };

                    // Open outbound chat view
                    info!("Creating chat UI");
                    let chat_ui = cx.new(|cx| AutomergeChatUi::new(doc_handle, cx))?;
                    ui.update(cx, |this, cx| {
                        let per_endpoint = this
                            .per_endpoint_state
                            .entry(endpoint_addr)
                            .or_insert_with(Default::default);
                        per_endpoint.chat_ui = Some(chat_ui.clone());
                        per_endpoint.doc_sync_task = Some(sync_task);

                        let Some(window) = cx.active_window() else {
                            bail!("no active window");
                        };
                        let Some(window) = window.downcast::<Workspace>() else {
                            bail!("window downcast");
                        };

                        info!("Opening chat UI in workspace");
                        window.update(cx, |workspace, window, cx| {
                            workspace.add_item_to_active_pane(
                                Box::new(chat_ui),
                                Some(0),
                                false,
                                window,
                                cx,
                            );
                        })?;

                        cx.notify();
                        anyhow::Ok(())
                    })??;

                    anyhow::Ok(())
                }
            });

            self.per_endpoint_state
                .entry(endpoint_addr)
                .or_default()
                .doc_lookup_task = Some(doc_lookup_task);
        }

        info!("Clicked Connect");
    }

    fn click_create_doc(
        &mut self,
        _event: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(iroh) = &self.iroh else {
            warn!("Missing iroh");
            return;
        };
        info!("Create doc");

        cx.spawn({
            let iroh = iroh.clone();
            async move |ui, cx| {
                let Some(ui) = ui.upgrade() else {
                    bail!("Failed to upgrade UI (create doc)");
                };

                // Initialize doc with test "Hello" message
                let mut doc = Automerge::new();
                {
                    let mut tx = doc.transaction();
                    let mut content = DocContent::new();
                    content.messages = vec!["Hello".to_string()];
                    reconcile(&mut tx, &content)?;
                    tx.commit();
                }

                let doc_handle = iroh.automerge.repo().create(doc).await?;
                let doc_ui = cx.new(|cx| AutomergeChatUi::new(doc_handle, cx))?;

                ui.update(cx, |this, cx| {
                    this.docs.push(doc_ui);
                    cx.notify();
                })?;

                anyhow::Ok(())
            }
        })
        .detach_and_log_err(cx);
    }

    fn click_doc(
        ui: Entity<AutomergeChatUi>,
    ) -> impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) {
        move |this, _event, window, cx| {
            let Some(iroh) = this.iroh.as_ref() else {
                warn!("missing iroh - click-doc");
                return;
            };

            let ui = ui.clone();
            this.workspace.update(cx, {
                let ui = ui.clone();
                move |workspace, cx| {
                    workspace.add_item_to_active_pane(Box::new(ui), Some(0), false, window, cx);
                }
            });

            let me = iroh.endpoint.addr();
            let doc_id = ui.read(cx).doc.document_id().clone();
            let ticket = AutomergeTicket {
                doc_id,
                endpoints: vec![me],
            };
            cx.write_to_clipboard(ClipboardItem::new_string(ticket.to_string()));
        }
    }

    fn create_topic_ui(
        &mut self,
        topic_id: TopicId,
        endpoints: Vec<EndpointAddr>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(iroh) = self.iroh.as_ref() else {
            warn!("missing iroh - create-topic-ui");
            return;
        };

        let topic_chat_ui =
            cx.new(|cx| TopicChatUi::new(iroh.clone(), topic_id, endpoints, window, cx));

        let workspace = self.workspace.clone();
        workspace.update(cx, |workspace, cx| {
            workspace.add_item_to_active_pane(
                Box::new(topic_chat_ui.clone()),
                Some(0),
                false,
                window,
                cx,
            );
        });

        self.topics.insert(topic_id, topic_chat_ui);
    }

    // async fn docs_playground(&mut self, docs: Docs) -> anyhow::Result<()> {
    //     let author = docs.api().author_default().await?;
    //     let list = docs.api().list().await?;
    //     tokio::pin!(list);
    //     while let Some((namespace_id, capability)) = list.try_next().await? {
    //         //
    //         match capability {
    //             iroh_docs::CapabilityKind::Write => todo!(),
    //             iroh_docs::CapabilityKind::Read => todo!(),
    //         }
    //     }
    //     let doc = docs.api().create().await?;
    //     let namespace = doc.id();
    //     let doc_ticket = doc.share(ShareMode::Write, AddrInfoOptions::Relay).await?;
    //     let (doc, doc_stream) = docs.api().import_and_subscribe(doc_ticket).await?;
    //     tokio::pin!(doc_stream);
    //     while let Some(item) = doc_stream.try_next().await? {
    //         //
    //         match item {
    //             iroh_docs::engine::LiveEvent::InsertLocal { entry } => todo!(),
    //             iroh_docs::engine::LiveEvent::InsertRemote {
    //                 from,
    //                 entry,
    //                 content_status,
    //             } => todo!(),
    //             iroh_docs::engine::LiveEvent::ContentReady { hash } => todo!(),
    //             iroh_docs::engine::LiveEvent::PendingContentReady => todo!(),
    //             iroh_docs::engine::LiveEvent::NeighborUp(public_key) => todo!(),
    //             iroh_docs::engine::LiveEvent::NeighborDown(public_key) => todo!(),
    //             iroh_docs::engine::LiveEvent::SyncFinished(sync_event) => todo!(),
    //         }
    //     }

    //     let doc_stream = doc.get_many(Query::all()).await?;
    //     tokio::pin!(doc_stream);
    //     // while let Some(entry) = doc_stream.try_next().await? {
    //     //     //
    //     //     let id = entry.id();
    //     //     let record = entry.record();
    //     //     let data = entry.to_vec();
    //     // }
    //     // doc.share(mode, addr_options)

    //     Ok(())
    // }
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
