use autosurgeon::{Hydrate, Reconcile, hydrate, reconcile};
use iroh::EndpointId;
use plugin_galvanized::{Galvanized, app_behavior::AppBehavior};
use samod::DocHandle;
use tracing::{debug, info, instrument};
use uuid::Uuid;
use zed::unstable::{
    db::smol::stream::StreamExt as _,
    editor::Editor,
    gpui::{
        self, AppContext as _, Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent, actions,
        rgb,
    },
    ui::{
        ActiveTheme, App, Context, InteractiveElement as _, IntoElement, ParentElement, Render,
        RenderOnce, SharedString, Styled, Window, div, v_flex,
    },
    util::{ResultExt, TryFutureExt},
    workspace::Item,
};

actions!(
    chat,
    [
        /// Opens the chat interface
        OpenChat,
    ]
);

pub fn init(cx: &mut App) {
    cx.observe_new::<Galvanized>(|galvanized, _window, cx| {
        let galvanized_entity = cx.entity();
        let chat_app = cx.new(|cx| ChatApp::new(galvanized_entity, cx));
        galvanized.add_app(chat_app);
    })
    .detach();
}

pub struct ChatApp {
    galvanized: Entity<Galvanized>,
}

impl ChatApp {
    pub fn new(galvanized: Entity<Galvanized>, cx: &mut Context<Self>) -> Self {
        Self { galvanized }
    }
}

impl AppBehavior for ChatApp {
    fn id(&self) -> &'static str {
        "chat"
    }

    fn icon(&self) -> SharedString {
        "💬".into()
    }

    fn title(&self) -> SharedString {
        "Chat".into()
    }
}

#[derive(IntoElement)]
pub struct ChatBubble {
    //
    from: SharedString,
    message: SharedString,
}

impl ChatBubble {
    pub fn new(message: &ChatMessage) -> Self {
        Self {
            from: SharedString::from(&message.sender_id),
            message: SharedString::from(&message.body),
        }
    }
}

impl RenderOnce for ChatBubble {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        v_flex()
            //
            .p_2()
            .bg(cx.theme().colors().element_background)
            .border_1()
            .border_color(rgb(0x7008e7))
            .rounded_lg()
            // Bubble body
            .child(format!("From: {}", self.from))
            .child(format!("Message: {}", self.message))
    }
}

/// let ChatUi be the large Item in the main window,
/// let ChatBubble be one item in the feed
pub struct ChatUi {
    doc_handle: DocHandle,
    document: ChatDocument,
    endpoint_id: EndpointId,
    focus_handle: FocusHandle,
    input_editor: Entity<Editor>,
}

#[derive(Debug, Clone, Hydrate, Reconcile)]
pub struct ChatDocument {
    //
    messages: Vec<ChatMessage>,
}

impl ChatDocument {
    pub fn new() -> Self {
        Self {
            messages: Default::default(),
        }
    }

    pub fn add_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
    }
}

#[derive(Debug, Clone, Hydrate, Reconcile)]
pub struct ChatMessage {
    //
    #[key]
    id: Uuid,
    sender_id: String,
    sender_name: String,
    body: String,
}

impl ChatMessage {
    pub fn new(
        sender_id: impl Into<String>,
        sender_name: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            sender_id: sender_id.into(),
            sender_name: sender_name.into(),
            body: message.into(),
        }
    }
}

impl ChatUi {
    pub fn new(
        remote_id: EndpointId,
        doc_handle: DocHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let document = ChatDocument::new();

        let input_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("Message", window, cx);
            editor
        });

        cx.spawn({
            let doc_handle = doc_handle.clone();
            async move |this, cx| {
                let (tx, rx) = flume::bounded(10);
                cx.background_spawn(
                    async move {
                        info!(doc_id = ?doc_handle.document_id(), "Starting Automerge listen loop");

                        // Watch the change stream and re-hydrate the document on each update
                        let mut doc_stream = doc_handle.changes();
                        while let Some(changes) = doc_stream.next().await {
                            info!(?changes, "Received Automerge update for Chat");

                            doc_handle.with_document(|am| {
                                let chat_document: ChatDocument = hydrate(am)?;
                                tx.send(chat_document).log_err();
                                info!("Automerge document listen loop sent update to UI");
                                anyhow::Ok(())
                            })?;
                        }

                        info!("Leaving Doc Change loop");
                        anyhow::Ok(())
                    }
                    .log_err(),
                )
                .detach();

                let mut rx_stream = rx.into_stream();
                while let Some(chat_document) = rx_stream.next().await {
                    info!("Automerge UI listen loop received update");
                    this.update(cx, |this, cx| {
                        // Update the UI's document with the new chat document
                        this.document = chat_document;
                        cx.notify();
                        info!("Applied Automerge update to ChatUI");
                    })?;
                }

                anyhow::Ok(())
            }
        })
        .detach_and_log_err(cx);

        Self {
            document,
            doc_handle,
            endpoint_id: remote_id,
            focus_handle: cx.focus_handle(),
            input_editor,
        }
    }
}

impl Render for ChatUi {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            //
            .bg(cx.theme().colors().editor_background)
            // Messages above
            .child(
                //
                v_flex()
                    //
                    .child(
                        //
                        v_flex()
                            .size_full()
                            //
                            .p_4()
                            .gap_2()
                            .bg(cx.theme().colors().panel_background)
                            .child(
                                //
                                div()
                                    //
                                    .text_xl()
                                    .child(format!(
                                        "Document ID: {}",
                                        self.doc_handle.document_id()
                                    )),
                            )
                            .child(
                                //
                                div()
                                    //
                                    .text_color(cx.theme().colors().text_muted)
                                    .child({
                                        //
                                        let mut id = self.endpoint_id.to_string();
                                        let id = id.split_off(id.len() - 8);
                                        format!("Remote peer: {}", id)
                                    }),
                            ),
                    )
                    .child(
                        //
                        // The direct container of every child ChatBubble
                        v_flex()
                            .flex_grow()
                            //
                            .p_2()
                            .gap_2()
                            .children(self.document.messages.iter().map(|message| {
                                //
                                ChatBubble::new(&message)
                            })),
                    ),
            )
            .child(div().flex_grow())
            // Text input below
            .child(self.render_chat_input(window, cx))
    }
}

impl ChatUi {
    fn render_chat_input(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            //
            .p_2()
            .child(
                //
                div()
                    .id("chat-input")
                    .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                        if e.keystroke.key != "enter" {
                            return;
                        }
                        info!("Pressed Enter to send");
                        this.send_message(window, cx);
                    }))
                    //
                    .p_2()
                    .border_2()
                    .border_color(cx.theme().colors().border_selected)
                    .rounded_lg()
                    .child(self.input_editor.clone()),
            )
    }

    #[instrument(skip_all)]
    fn send_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.input_editor.read(cx).text(cx);
        debug!(text, "Begin send_message");
        if text.is_empty() {
            return;
        }
        let message = ChatMessage::new("ID", "Name", text);
        self.document.add_message(message);
        self.input_editor
            .update(cx, |it, cx| it.set_text("", window, cx));
        debug!("Added new message to local ChatUI");

        let document = self.document.clone();
        let doc_handle = self.doc_handle.clone();
        cx.spawn(async move |_ui, cx| {
            cx.background_spawn(async move {
                doc_handle.with_document(|am| {
                    let mut tx = am.transaction();
                    reconcile(&mut tx, &document)?;
                    let (_change_hash, _patch_log) = tx.commit();
                    info!(?document, "Wrote new message to Automerge");
                    anyhow::Ok(())
                })?;
                anyhow::Ok(())
            })
            .await?;
            //
            anyhow::Ok(())
        })
        .detach_and_log_err(cx);
    }
}

impl Focusable for ChatUi {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
type ChatEvent = ();
impl EventEmitter<ChatEvent> for ChatUi {}
impl Item for ChatUi {
    type Event = ChatEvent;

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        let mut string = self.endpoint_id.to_string();
        let suffix = string.split_off(string.len() - 8);
        suffix.into()
    }
}
