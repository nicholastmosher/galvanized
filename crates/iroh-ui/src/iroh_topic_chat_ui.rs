use anyhow::bail;
use iroh_gossip::{
    TopicId,
    api::{GossipReceiver, GossipSender, Message},
};
use tracing::info;
use zed::unstable::{
    db::smol::stream::StreamExt as _,
    gpui::{AsyncApp, Entity, EventEmitter, FocusHandle, Focusable},
    ui::{
        App, Context, IntoElement, ParentElement as _, Render, SharedString, Styled as _, Window,
        div,
    },
    workspace::Item,
};

use crate::{DebugViewExt as _, Ticket, iroh_panel_ui::Iroh};

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
    topic_title: String,
    topic_sender: Option<GossipSender>,
    topic_receiver: Option<GossipReceiver>,
    topics: Vec<(TopicId, Ticket)>,
}

impl TopicChatUi {
    pub fn new(iroh: Iroh, topic_name: String, cx: &mut Context<Self>) -> Self {
        //

        let topic_id = TopicId::from_bytes(rand::random());
        let me = iroh.endpoint.addr();
        let ticket = Ticket {
            topic_id,
            endpoints: vec![me],
        };
        let topics = vec![(topic_id, ticket)];

        // Spawn gossip topic
        cx.spawn({
            let iroh = iroh.clone();
            async move |this, cx| {
                let Some(ui) = this.upgrade() else {
                    bail!("TopicChatUi is no longer available")
                };

                let bootstrap = vec![];
                let topic = iroh.gossip.subscribe_and_join(topic_id, bootstrap).await?;
                let (sender, mut receiver) = topic.split();

                // Receiver
                cx.spawn(async move |cx| {
                    while let Some(event) = receiver.try_next().await? {
                        let iroh_gossip::api::Event::Received(message) = event else {
                            continue;
                        };

                        // Each message handled by a new task
                        cx.spawn({
                            let ui = ui.clone();
                            async move |cx| {
                                Self::handle_received_message(ui, message, cx).await;
                            }
                        })
                        .detach();
                    }

                    anyhow::Ok(())
                })
                .detach();

                anyhow::Ok(())
            }
        })
        .detach_and_log_err(cx);

        // note(rustfmt): Self {} collapses even with // inside
        Self {
            //
            iroh,
            focus_handle: cx.focus_handle(),
            topic_title: topic_name,
            topic_sender: None,
            topic_receiver: None,
            topics,
        }
    }

    async fn handle_received_message(ui: Entity<TopicChatUi>, message: Message, cx: &mut AsyncApp) {
        // TODO: Message decoding
        let buffer = message.content;
        let text = String::from_utf8_lossy(&buffer);
        info!(%text, "Received");
    }
}

impl Render for TopicChatUi {
    fn render(
        //
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .debug_border()
            .p_2()
            .flex()
            .flex_row()
            .child(self.render_header(window, cx))
            .child(self.render_body(window, cx))
    }
}

/// Subcomponent renderings
impl TopicChatUi {
    fn render_header(
        //
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            //
            .debug_border()
            .text_2xl()
            .child(self.topic_title.to_string())
    }

    fn render_body(
        //
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            //
            .debug_border()
            .child("Body")
    }
}

pub enum TopicChatEvent {
    //
}

impl EventEmitter<TopicChatEvent> for TopicChatUi {}
impl Focusable for TopicChatUi {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Item for TopicChatUi {
    type Event = TopicChatEvent;

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        SharedString::from(&self.topic_title)
    }
}
