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

use tracing::info;
use zed::unstable::{
    gpui::{AppContext as _, Entity, EventEmitter, FocusHandle, Focusable},
    ui::{
        ActiveTheme as _, App, Context, InteractiveElement as _, IntoElement, ParentElement as _,
        Render, SharedString, StatefulInteractiveElement as _, Styled as _, Window, div,
    },
    workspace::{Item, Workspace},
};

use crate::{
    chat::ChatUi,
    willow::{Willow, button_input::ButtonInput},
};

pub trait SpaceExt {
    fn create_space(
        &self,
        name: String,
        workspace: Entity<Workspace>,
        cx: &mut App,
    ) -> Entity<Space>;
    fn spaces(&self, cx: &mut App) -> Vec<Entity<Space>>;
}

impl SpaceExt for Willow {
    fn create_space(
        &self,
        name: String,
        workspace: Entity<Workspace>,
        cx: &mut App,
    ) -> Entity<Space> {
        self.state.update(cx, |state, cx| {
            let space = cx.new(|cx| Space::new(name, workspace, cx));
            state.spaces.push(space.clone());
            space
        })
    }
    fn spaces(&self, cx: &mut App) -> Vec<Entity<Space>> {
        self.state.read(cx).spaces.clone()
    }
}

pub struct Space {
    chats: Vec<Entity<ChatUi>>,

    ///
    create_chat: Entity<ButtonInput>,

    /// The user-displayed name of the space.
    name: String,

    /// A list of handles to entities in this space.
    entries: Vec<Entity<Entry>>,

    focus_handle: FocusHandle,

    workspace: Entity<Workspace>,
}

#[derive(Debug)]
pub struct Entry {
    content: String,
}
impl Entry {
    fn new(content: impl Into<String>, _cx: &mut Context<Self>) -> Self {
        Self {
            //
            content: content.into(),
        }
    }
}

impl Render for Space {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_color = cx.theme().colors().ghost_element_active;
        let hover_bg_color = cx.theme().colors().ghost_element_hover;
        let hover_border_color = cx.theme().colors().border.opacity(1.0);
        div()
            //
            // TODO fix ID
            .id(SharedString::from(format!("entry-{}", self.name)))
            .p_2()
            .debug()
            .flex()
            .flex_col()
            .child(format!("Space div.child(): '{}'", self.name))
            .children(self.entries(cx).into_iter().enumerate().map(|(i, entry)| {
                div()
                    //
                    .id(SharedString::from(format!("entry-{}-{i}", self.name)))
                    .p_2()
                    .active(|style| style.bg(active_color))
                    .hover(|style| style.bg(hover_bg_color).border_color(hover_border_color))
                    .rounded_md()
                    .child(
                        //
                        div()
                            //
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
    pub fn new(
        name: impl Into<String>,
        workspace: Entity<Workspace>,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = name.into();
        let space = cx.entity();
        let create_chat = cx.new({
            let workspace = workspace.clone();
            |cx| {
                ButtonInput::new(
                    SharedString::from(format!("create-chat-{name}")),
                    "+ Chat".into(),
                    cx,
                )
                .on_submit(move |this, text, window, cx| {
                    info!("Submitted +Chat");

                    space.update(cx, |space, cx| {
                        let chat = cx.new(|cx| ChatUi::new(text, cx));
                        workspace.update(cx, |workspace, cx| {
                            //
                            space.chats.push(chat.clone());
                            workspace.add_item_to_active_pane(
                                Box::new(chat.clone()),
                                Some(0),
                                true,
                                window,
                                cx,
                            );
                        });
                    });

                    this.clear();
                    cx.notify();
                })
            }
        });

        Self {
            chats: Default::default(),
            create_chat,
            name,
            // entries: Default::default(),
            entries: vec![cx.new(|cx| Entry::new("apps/chat/{id}/", cx))],
            focus_handle: cx.focus_handle(),
            workspace,
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
