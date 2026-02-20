//! # Spaces
//!
//! > Yes, this is Willow's Namespaces. As part of my experimentation in
//! > this project, I want to consider how to craft an easy-to-follow
//! > terminology and end-user mental model. So I'll be describing the
//! > terms the way I'd want to present the idea to the user.
//!
//! A Space is like a magic folder that's shared between members.
//!
//! There are two kinds of spaces, Owned (invite-only), and Communal
//! (open to anyone). You can be a member of more than one space, and
//! you can have more than one Profile.
//!
//! You can imagine that each Profile that's a member of a space has its
//! own dedicated home directory. You and your apps read and write in
//! your profile's home directory. You can also give other profiles the
//! ability to read and write to your choice of subdirectories in the form
//! of capabilities.
//!
//! Apps are effectively an interface to viewing data.
//!
//! - A photo album is just a list of files rendered as a grid
//! - A calendar is just a list of events, rendered on a week or month view
//! - A chat is just a list of messages and media, rendered as a conversation

use std::fmt::Display;

use tracing::warn;
use zed::unstable::{
    gpui::{AppContext as _, Entity, EventEmitter, FocusHandle, Focusable},
    ui::{
        App, Context, IntoElement, ListItem, ParentElement as _, Render, SharedString, Styled as _,
        Window, div,
    },
    workspace::{Item, Workspace},
};

use crate::{chat::ChatUi, willow::button_input::ButtonInput};

pub struct Space {
    chats: Vec<Entity<ChatUi>>,

    ///
    create_chat: Entity<ButtonInput>,

    /// The user-displayed name of the space.
    name: String,

    /// A list of handles to entities in this space.
    entries: Vec<Entity<Entry>>,

    focus_handle: FocusHandle,
}

#[derive(Debug)]
pub struct Entry {
    content: String,
}
impl Entry {
    fn new(content: impl Into<String>, cx: &mut Context<Self>) -> Self {
        Self {
            //
            content: content.into(),
        }
    }
}

impl Render for Space {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .debug()
            .flex()
            .flex_col()
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
            .child(
                //
                self.create_chat.clone(),
            )
    }
}

impl Space {
    pub fn new(name: impl Into<String>, cx: &mut Context<Self>) -> Self {
        let name = name.into();
        let weak_space = cx.weak_entity();
        let create_chat = cx.new(|cx| {
            ButtonInput::new(
                SharedString::from(format!("create-chat-{name}")),
                "+ Chat".into(),
                cx,
            )
            .on_submit(move |this, text, _window, cx| {
                let Some(space) = weak_space.upgrade() else {
                    warn!("Weak space");
                    return;
                };

                // let chat_ui = cx.new(|cx| ChatUi::new(text, cx));
                // space.update(cx, |space, _cx| {
                //     space.chats.push(chat_ui);
                // });

                // let workspace = workspace.clone();
                // workspace.update(cx, |workspace, cx| {
                //     workspace.add_item_to_active_pane(
                //         Box::new(chat_ui),
                //         Some(0),
                //         false,
                //         window,
                //         cx,
                //     );
                // });

                this.clear();
                cx.notify();
            })
        });

        Self {
            chats: Default::default(),
            create_chat,
            name,
            // entries: Default::default(),
            entries: vec![cx.new(|cx| Entry::new("apps/chat/{id}/", cx))],
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
