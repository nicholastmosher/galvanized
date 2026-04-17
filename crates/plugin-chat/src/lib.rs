use automerge::AutoCommit;
use autosurgeon::{Hydrate, Reconcile};
use iroh::EndpointId;
use samod::DocHandle;
/// ChatUi is a `Workspace` item, rendering into the tab window
use zed::unstable::{
    db::smol::stream::StreamExt,
    editor::Editor,
    gpui::{
        self, AppContext as _, Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent, actions,
        rgb,
    },
    ui::{
        ActiveTheme, App, Context, InteractiveElement as _, IntoElement, ParentElement, Render,
        RenderOnce, SharedString, Styled, Window, div, v_flex,
    },
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
    // cx.observe_new::<Workspace>(|workspace, window, cx| {
    //     let Some(window) = window else { return };
    //     let chat = cx.new(|cx| ChatUi::new("MyChat", window, cx));
    //     workspace.add_item_to_active_pane(Box::new(chat.clone()), Some(0), true, window, cx);
    //     workspace.register_action(move |workspace, _: &OpenChat, window, cx| {
    //         workspace.add_item_to_active_pane(Box::new(chat.clone()), Some(0), true, window, cx);
    //     });
    // })
    // .detach();
}

#[derive(IntoElement)]
pub struct ChatBubble {
    //
    from: SharedString,
    message: SharedString,
}

impl ChatBubble {
    pub fn new(from: impl Into<SharedString>, message: impl Into<SharedString>) -> Self {
        Self {
            from: from.into(),
            message: message.into(),
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

#[derive(Hydrate, Reconcile)]
pub struct ChatDocument {
    //
    messages: Vec<ChatMessage>,
}

#[derive(Hydrate, Reconcile)]
pub struct ChatMessage {
    //
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
            sender_id: sender_id.into(),
            sender_name: sender_name.into(),
            body: message.into(),
        }
    }
}

impl ChatUi {
    pub fn new(
        endpoint_id: EndpointId,
        doc_handle: DocHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let messages = vec![
            // ChatMessage::new("John", "Hey what's up?"),
            // ChatMessage::new("Mary", "Nothing much"),
        ];

        let document = ChatDocument { messages };

        let input_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("Message", window, cx);
            editor
        });

        cx.spawn({
            let doc = doc_handle.clone();
            async move |weak_this, cx| {
                let mut doc_stream = doc.changes();
                while let Some(changes) = doc_stream.next().await {
                    // for change in &changes.new_heads {
                    //
                    let result = doc
                        .with_document(|automerge| {
                            let autocommit = AutoCommit::load(&automerge.save())?;
                            anyhow::Ok(autocommit)
                        })
                        .map_err(|e| e.context("failed to load Automerge doc"));
                    let autocommit = match result {
                        Ok(ac) => ac,
                        Err(error) => {
                            tracing::error!(?error, "error while handling doc_stream event");
                            continue;
                        }
                    };
                }
            }
        })
        .detach();

        Self {
            document,
            doc_handle,
            endpoint_id,
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
                    .flex_grow()
                    //
                    .p_2()
                    .gap_2()
                    .children(self.document.messages.iter().map(|message| {
                        //
                        ChatBubble::new(&message.sender_id, &message.body)
                    })),
            )
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
            .id("chat-input")
            //
            .border_2()
            .border_color(cx.theme().colors().border_selected)
            .p_4()
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                if e.keystroke.key != "enter" {
                    return;
                }
                let text = this.input_editor.read(cx).text(cx);
                if !text.is_empty() {
                    return;
                }

                let doc = this.doc_handle.clone();
                cx.spawn(async move |ui, cx| {
                    cx.background_spawn(async move {
                        doc.with_document(|am| {
                            let auto_commit = AutoCommit::load(&am.save())?;
                            anyhow::Ok(())
                        })?;

                        anyhow::Ok(())
                    })
                    .detach();

                    //
                    anyhow::Ok(())
                })
                .detach_and_log_err(cx);
            }))
            .child(self.input_editor.clone())
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
        SharedString::from(self.endpoint_id.to_string())
    }
}
