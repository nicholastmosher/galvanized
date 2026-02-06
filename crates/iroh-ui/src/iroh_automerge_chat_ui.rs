use samod::DocHandle;
use zed::unstable::{
    gpui::{EventEmitter, FocusHandle, Focusable},
    ui::{App, Context, IntoElement, ParentElement as _, Render, SharedString, Window, div},
    workspace::Item,
};

fn init(cx: &mut App) {
    //
}

pub struct AutomergeChatUi {
    //
    pub doc: DocHandle,
    focus_handle: FocusHandle,
}

impl AutomergeChatUi {
    pub fn new(doc: DocHandle, cx: &mut Context<Self>) -> Self {
        //
        Self {
            doc,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Render for AutomergeChatUi {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .child("AUTOMERGE DOC UI")
    }
}

impl Focusable for AutomergeChatUi {
    fn focus_handle(&self, cx: &App) -> zed::unstable::gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<()> for AutomergeChatUi {}

impl Item for AutomergeChatUi {
    type Event = ();

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        "Automerge".into()
    }
}
