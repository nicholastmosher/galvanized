use autosurgeon::{Hydrate, Reconcile};
/// ChatUi is a `Workspace` item, rendering into the tab window
use zed::unstable::{
    editor::Editor,
    gpui::{
        self, AppContext as _, Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent, actions,
        rgb,
    },
    ui::{
        ActiveTheme, App, Context, InteractiveElement as _, IntoElement, ParentElement, Render,
        RenderOnce, SharedString, StatefulInteractiveElement as _, Styled, Window, div, v_flex,
    },
    workspace::{Item, Workspace},
};

use crate::iroh::IrohExt;

actions!(
    chat,
    [
        /// Opens the chat interface
        OpenChat,
    ]
);

pub fn init(cx: &mut App) {
    cx.observe_new::<Workspace>(|workspace, window, cx| {
        let Some(window) = window else { return };
        let chat = cx.new(|cx| ChatUi::new("MyChat", window, cx));
        workspace.add_item_to_active_pane(Box::new(chat.clone()), Some(0), true, window, cx);
        workspace.register_action(move |workspace, _: &OpenChat, window, cx| {
            workspace.add_item_to_active_pane(Box::new(chat.clone()), Some(0), true, window, cx);
        });
    })
    .detach();
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
    document: ChatDocument,
    focus_handle: FocusHandle,
    input_editor: Entity<Editor>,
    title: SharedString,
}

#[derive(Hydrate, Reconcile)]
pub struct ChatDocument {
    //
    messages: Vec<ChatMessage>,
}

#[derive(Hydrate, Reconcile)]
pub struct ChatMessage {
    //
    from: String,
    body: String,
}

impl ChatMessage {
    pub fn new(from: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            body: message.into(),
        }
    }
}

impl ChatUi {
    pub fn new(
        title: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let messages = vec![
            ChatMessage::new("John", "Hey what's up?"),
            ChatMessage::new("Mary", "Nothing much"),
        ];

        let document = ChatDocument { messages };

        let input_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("Message", window, cx);
            editor
        });

        Self {
            document,
            focus_handle: cx.focus_handle(),
            input_editor,
            title: title.into(),
        }
    }
}

impl Render for ChatUi {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            //
            .bg(cx.theme().colors().editor_background)
            .child(
                //
                v_flex()
                    .flex_grow()
                    //
                    .p_2()
                    .gap_2()
                    .children(self.document.messages.iter().map(|message| {
                        //
                        ChatBubble::new(&message.from, &message.body)
                    })),
            )
            .child(
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

                        cx.spawn(async move |ui, cx| {
                            let doc = cx.iroh().create_doc(cx);

                            //
                            anyhow::Ok(())
                        })
                        .detach_and_log_err(cx);
                    }))
                    .child(self.input_editor.clone()),
            )
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
        SharedString::from(&self.title)
    }
}
