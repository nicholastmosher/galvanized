/// ChatUi is a `Workspace` item, rendering into the tab window
use zed::unstable::{
    gpui::{AppContext as _, Entity, EventEmitter, FocusHandle, Focusable},
    ui::{App, Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, div},
    workspace::Item,
};

pub struct Feed<T> {
    //
    children: Vec<Entity<T>>,
}

impl<T> Feed<T> {
    //
    pub fn new(children: impl IntoIterator<Item = Entity<T>>, cx: &mut Context<Self>) -> Self {
        Self {
            //
            children: children.into_iter().collect(),
        }
    }
}

impl<T: Render> Render for Feed<T> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .p_2()
            .gap_2()
            .flex()
            .flex_col()
            .children(self.children.clone())
    }
}

pub struct ChatBubble {
    //
    from: String,
    message: String,
}

impl ChatBubble {
    pub fn new(from: String, message: String, cx: &mut Context<Self>) -> Self {
        Self { from, message }
    }
}

impl Render for ChatBubble {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .flex()
            .flex_col()
            // Top bar across bubble
            .child(
                div()
                    //
                    .w_full(),
            )
            // Bubble body
            .child(
                div()
                    //
                    .flex_grow()
                    .child("One")
                    .child("Two"),
            )
    }
}

/// let ChatUi be the large Item in the main window,
/// let Feed be one column of content in the item window,
/// let ChatBubble be one item in the feed
pub struct ChatUi {
    // TODO: Plural feeds
    chat_feed: Entity<Feed<ChatBubble>>,
    focus_handle: FocusHandle,
    title: String,
}

impl Render for ChatUi {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .child(self.chat_feed.clone())
    }
}

impl ChatUi {
    pub fn new(title: String, cx: &mut Context<Self>) -> Self {
        let feed_items = [
            //
            cx.new(|cx| ChatBubble::new("John".to_string(), "Hey what's up?".to_string(), cx)),
            cx.new(|cx| ChatBubble::new("Mary".to_string(), "Nothing much".to_string(), cx)),
        ];
        let chat_feed = cx.new(|cx| Feed::new(feed_items, cx));
        Self {
            chat_feed,
            focus_handle: cx.focus_handle(),
            title,
        }
    }
}

impl Focusable for ChatUi {
    fn focus_handle(&self, cx: &App) -> zed::unstable::gpui::FocusHandle {
        self.focus_handle.clone()
    }
}
type ChatEvent = ();
impl EventEmitter<ChatEvent> for ChatUi {}
impl Item for ChatUi {
    type Event = ChatEvent;

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        "Chat".into()
    }
}
