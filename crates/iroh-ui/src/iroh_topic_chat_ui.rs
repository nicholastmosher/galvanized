use iroh_gossip::{TopicId, api::GossipTopic};
use zed::unstable::{
    gpui::{EventEmitter, FocusHandle, Focusable},
    ui::{App, Context, IntoElement, Render, SharedString, Window, div},
    workspace::Item,
};

use crate::{Ticket, iroh_panel_ui::Iroh};

pub fn init(cx: &mut App) {
    //
    cx.observe_new(|this: &mut TopicChatUi, window, cx| {
        //
    })
    .detach();
}

/// Tab item UI for an instance of a topic chat
pub struct TopicChatUi {
    //
    iroh: Iroh,
    focus_handle: FocusHandle,
    tab_title: String,
    topic: GossipTopic,
    topics: Vec<(TopicId, Ticket)>,
}

impl Render for TopicChatUi {
    fn render(
        //
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
    }
}

impl TopicChatUi {
    pub fn new(iroh: Iroh, topic: GossipTopic, topic_name: String, cx: &mut Context<Self>) -> Self {
        //

        let topic_id = TopicId::from_bytes(rand::random());
        let me = iroh.endpoint.addr();
        let ticket = Ticket {
            topic_id,
            endpoints: vec![me],
        };
        let topics = vec![(topic_id, ticket)];

        // note(rustfmt): Self {} collapses even with // inside
        Self {
            //
            iroh,
            focus_handle: cx.focus_handle(),
            tab_title: topic_name,
            topic,
            topics,
        }
    }
}

impl EventEmitter<()> for TopicChatUi {}
impl Focusable for TopicChatUi {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Item for TopicChatUi {
    type Event = ();

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        SharedString::from(&self.tab_title)
    }
}
