use std::fmt::Display;

use zed::unstable::{
    gpui::{AppContext as _, Entity, EventEmitter, FocusHandle, Focusable},
    ui::{
        App, Context, IntoElement, ListItem, ParentElement as _, Render, SharedString, Styled as _,
        Window, div,
    },
    workspace::Item,
};

pub struct Space {
    /// The user-displayed name of the space.
    name: String,

    /// A list of handles to entities in this space.
    entries: Vec<Entity<Entry>>,

    focus_handle: FocusHandle,
}

#[derive(Debug)]
pub struct Entry {
    data: Vec<u8>,
}
impl Entry {
    fn new(data: impl Into<Vec<u8>>, cx: &mut Context<Self>) -> Self {
        Self {
            //
            data: data.into(),
        }
    }
}

impl Render for Space {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .children(self.entries(cx).into_iter().enumerate().map(|(i, entry)| {
                //
                ListItem::new(SharedString::from(format!("ns-entry-{i}")))
                    .rounded()
                    .child(
                        //
                        div()
                            //
                            .p_2()
                            .child(format!("{}/{:?}", self.name(), entry)),
                    )
            }))
    }
}

impl Space {
    pub fn new(name: impl Into<String>, cx: &mut Context<Self>) -> Self {
        Self {
            name: name.into(),
            entries: Default::default(),
            focus_handle: cx.focus_handle(),
        }
    }

    // TODO: Index entries by digest
    pub fn create_entry(&mut self, entry: String, cx: &mut Context<Self>) {
        let entry = cx.new(move |cx| Entry::new(entry, cx));
        self.entries.push(entry);
    }

    pub fn name(&self) -> impl Display {
        self.name.to_string()
    }

    pub fn entries<'a>(&self, cx: &'a mut Context<Self>) -> impl IntoIterator<Item = &'a Entry> {
        self.entries.iter().map(|entry| entry.read(cx))
    }
}

impl Focusable for Space {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
type SpaceEvent = ();
impl EventEmitter<SpaceEvent> for Space {}
impl Item for Space {
    type Event = SpaceEvent;

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        SharedString::from(&self.name)
    }
}
