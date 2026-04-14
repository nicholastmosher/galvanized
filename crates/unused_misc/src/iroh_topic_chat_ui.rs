use anyhow::{Context as _, bail};
use bytes::Bytes;
use iroh::EndpointAddr;
use iroh_gossip::{TopicId, api::Message};
use tracing::{info, warn};
use zed::unstable::{
    db::smol::stream::StreamExt as _,
    editor::Editor,
    gpui::{AppContext as _, AsyncApp, Entity, EventEmitter, FocusHandle, Focusable},
    ui::{
        App, Button, Clickable as _, Context, IconName, IconPosition, IconSize, IntoElement,
        LabelSize, ListItem, ParentElement as _, Render, SharedString, Styled as _, Window, div,
    },
    util::ResultExt,
    workspace::Item,
};

use crate::{DebugViewExt as _, iroh_panel_ui::Iroh};

pub fn init(cx: &mut App) {
    //
    cx.observe_new(|_this: &mut TopicChatUi, _window, _cx| {
        //
    })
    .detach();
}

/// Tab item UI for an instance of a topic chat
#[allow(unused)]
pub struct TopicChatUi {
    //
    iroh: Iroh,
    focus_handle: FocusHandle,

    messages: Vec<String>,
    sender: Option<flume::Sender<String>>,
    title: String,
    topic_editor: Entity<Editor>,
}

impl TopicChatUi {
    pub fn new(
        //
        iroh: Iroh,
        topic_id: TopicId,
        endpoints: Vec<EndpointAddr>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        info!("Creating TopicChat");

        // Spawn gossip topic
        cx.spawn({
            let iroh = iroh.clone();
            async move |this, cx| {
                let Some(ui) = this.upgrade() else {
                    bail!("TopicChatUi is no longer available")
                };

                info!(
                    location = %core::panic::Location::caller(),
                    "Spawning gossip topic"
                );
                let bootstrap = endpoints.iter().map(|it| it.id).collect();
                let topic = iroh
                    .gossip
                    .subscribe(topic_id, bootstrap)
                    .await
                    .with_context(|| format!("failed to subscribe to topic {}", topic_id))?;
                let (tx, rx) = flume::bounded::<String>(10);
                let (sender, mut receiver) = topic.split();

                // Save sender
                info!("🟣 Assigning Sender");
                ui.update(cx, move |ui, _cx| {
                    warn!("✅ Assigning Sender");
                    ui.sender = Some(tx);
                })?;

                // Spawn gossip sender
                cx.spawn(async move |_cx| {
                    while let Ok(message) = rx.recv_async().await {
                        info!(%message, "Sender forwaring");
                        sender.broadcast(Bytes::from(message)).await?;
                    }
                    anyhow::Ok(())
                })
                .detach();

                // Spawn receiver
                cx.spawn(async move |cx| {
                    while let Some(event) = receiver.try_next().await? {
                        let iroh_gossip::api::Event::Received(message) = event else {
                            continue;
                        };
                        info!(?message, "Gossip received message");

                        // Each message handled by a new task
                        cx.spawn({
                            let ui = ui.clone();
                            async move |cx| {
                                Self::handle_received_message(ui, message, cx).await?;
                                anyhow::Ok(())
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

        let topic_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("to send", window, cx);
            editor
        });

        // note(rustfmt): Self {} collapses even with // inside
        Self {
            //
            iroh,
            focus_handle: cx.focus_handle(),
            messages: Default::default(),
            sender: None,
            title: topic_id.to_string(),
            topic_editor,
        }
    }

    async fn handle_received_message(
        ui: Entity<TopicChatUi>,
        message: Message,
        cx: &mut AsyncApp,
    ) -> anyhow::Result<()> {
        // TODO: Message decoding
        let text = String::from_utf8_lossy(&message.content).to_string();
        info!(%text, "Received topic message");

        ui.update(cx, move |this, _cx| {
            this.messages.push(text);
        })?;

        Ok(())
    }

    fn send_message(
        &mut self,
        text: String,
        _window: &mut Window,
        _cx: &mut Context<'_, TopicChatUi>,
    ) {
        let Some(tx) = &self.sender else {
            warn!("No sender");
            return;
        };

        self.messages.push(text.to_string());
        info!(messages = ?self.messages, "Sent message");

        // Send message to sender task
        tx.send(text).log_err();
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
            .flex_col()
            .child(self.render_header(window, cx))
            .child(self.render_body(window, cx))
    }
}

/// Subcomponent renderings
impl TopicChatUi {
    fn render_header(
        //
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            //
            .debug_border()
            .text_2xl()
            .text_ellipsis()
            .child(format!("Topic {}", self.title))
    }

    fn render_body(
        //
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            //
            .debug_border()
            .flex()
            .flex_col()
            .child("Body")
            .children(self.messages.iter().enumerate().map(|(i, message)| {
                //
                ListItem::new(i).child(
                    //
                    div().p_2().child(message.to_string()),
                )
            }))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .child(self.topic_editor.clone())
                    .child(
                        Button::new("topic-send", "Send")
                            .label_size(LabelSize::Small)
                            .icon(IconName::Plus)
                            .icon_size(IconSize::Small)
                            .icon_position(IconPosition::Start)
                            .on_click(cx.listener(|this, _, window, cx| {
                                let text = this.topic_editor.read(cx).text(cx);
                                info!(%text, "Clicked Send");
                                this.send_message(text, window, cx);
                            })),
                    ),
            )
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
        SharedString::from(&self.title)
    }
}
