use serde_json::json;
/// ChatUi is a `Workspace` item, rendering into the tab window
use zed::unstable::{
    editor::Editor,
    gpui::{AppContext as _, Entity, EventEmitter, FocusHandle, Focusable},
    ui::{
        ActiveTheme, App, Context, IntoElement, ParentElement, Render, SharedString, Styled,
        Window, div,
    },
    workspace::Item,
};

use crate::willow::object_widget::ObjectWidget;

pub struct Feed<T> {
    //
    children: Vec<Entity<T>>,
}

impl<T> Feed<T> {
    //
    pub fn new(children: impl IntoIterator<Item = Entity<T>>, _cx: &mut Context<Self>) -> Self {
        Self {
            //
            children: children.into_iter().collect(),
        }
    }
}

impl<T: Render> Render for Feed<T> {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
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
    pub fn new(from: String, message: String, _cx: &mut Context<Self>) -> Self {
        Self { from, message }
    }
}

impl Render for ChatBubble {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .p_2()
            .bg(cx.theme().colors().element_background)
            .flex()
            .flex_col()
            .rounded_lg()
            // Bubble body
            .child(format!("From: {}", self.from))
            .child(format!("Message: {}", self.message))
    }
}

/// let ChatUi be the large Item in the main window,
/// let Feed be one column of content in the item window,
/// let ChatBubble be one item in the feed
pub struct ChatUi {
    // TODO: Plural feeds
    chat_feed: Entity<Feed<ChatBubble>>,
    focus_handle: FocusHandle,
    input_editor: Entity<Editor>,
    title: String,
}

impl ChatUi {
    pub fn new(title: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let feed_items = [
            cx.new(|cx| ChatBubble::new("John".to_string(), "Hey what's up?".to_string(), cx)),
            cx.new(|cx| ChatBubble::new("Mary".to_string(), "Nothing much".to_string(), cx)),
        ];
        let chat_feed = cx.new(|cx| Feed::new(feed_items, cx));
        let input_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("Message", window, cx);
            editor
        });
        Self {
            chat_feed,
            focus_handle: cx.focus_handle(),
            input_editor,
            title,
        }
    }
}

impl Render for ChatUi {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let object_widget = cx.new(|cx| {
            ObjectWidget::new(
                json!({
                    //
                    // "OneKey": "OneValue",
                    "One": {
                        "One.One": "11",
                        // "One.Two": [
                        //     "One.Two.One",
                        //     "One.Two.Two",
                        //     "One.Two.Three",
                        // ]
                    // },
                    "Two": 2,
                    "Three": 3,
                    // "Four": {
                    //     "FourOne": "41",
                    //     "FourTwo": "42",
                    //     "FourThree": null,
                    }
                }),
                cx,
            )
        });

        div()
            //
            .size_full()
            .flex()
            .flex_col()
            .child(self.chat_feed.clone())
            .child(div().flex_grow().debug())
            .child(object_widget.clone())
            .child(
                div()
                    .debug()
                    //
                    .p_4()
                    .child(self.input_editor.clone()),
            )
    }
}

impl Focusable for ChatUi {
    fn focus_handle(&self, _cx: &App) -> zed::unstable::gpui::FocusHandle {
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
