use std::collections::{BTreeMap, BTreeSet};

use libp2p::{PeerId, StreamProtocol, mdns::Event as MdnsEvent};
use libp2p_stream::Control;
use libp2p_swarm::SwarmEvent;
use zed::unstable::{
    db::smol::stream::StreamExt,
    gpui::{self, Action, AppContext, EventEmitter, FocusHandle, Focusable, actions, rgb},
    ui::{
        App, Context, IconName, IntoElement, ListItem, ParentElement, Pixels, Render, Styled,
        Window, div, px,
    },
    util::ResultExt,
    workspace::{
        Panel, Workspace,
        dock::{DockPosition, PanelEvent},
    },
};

use crate::p2p::{PeerieBehaviour, PeerieBehaviourEvent};

pub mod p2p;

actions!(workspace, [ToggleLibp2pPanel]);

const PROTOCOL: StreamProtocol = StreamProtocol::new("/prototyping/1.0.0");

pub fn init(cx: &mut App) {
    let Ok(mut swarm) = PeerieBehaviour::try_init_swarm() else {
        tracing::error!("Failed to initialize libp2p swarm");
        return;
    };
    let local_peer_id = *swarm.local_peer_id();
    let control = swarm.behaviour_mut().stream.new_control();
    let libp2p_ui = cx.new({
        let control = control.clone();
        move |cx| Libp2pUi::new(cx, local_peer_id, control)
    });

    // Swarm stream
    cx.spawn({
        let libp2p_ui = libp2p_ui.clone();
        async move |cx| {
            let mut swarm = PeerieBehaviour::try_init_swarm()?;
            while let Some(event) = swarm.next().await {
                tracing::info!(?event, "Emitting SwarmEvent");
                libp2p_ui.update(cx, |_ui, cx| {
                    cx.emit(event);
                })?;
            }
            tracing::warn!("Ending Swarm task");
            anyhow::Ok(())
        }
    })
    .detach_and_log_err(cx);

    // Stream control acceptor
    cx.spawn({
        let mut control = control.clone();
        let libp2p_ui = libp2p_ui.clone();
        async move |cx| {
            let mut incoming = control.accept(PROTOCOL)?;
            tracing::info!("Accepting incoming streams");
            while let Some((peer_id, stream)) = incoming.next().await {
                libp2p_ui
                    .update(cx, |ui, _cx| {
                        tracing::trace!(%peer_id, "Accepted peer stream");
                        ui.peer_streams.insert(peer_id, stream);
                    })
                    .log_err();
            }
            tracing::debug!("Libp2p stream acceptor quit");
            anyhow::Ok(())
        }
    })
    .detach_and_log_err(cx);

    cx.observe_new(move |workspace: &mut Workspace, window, cx| {
        let Some(window) = window else { return };
        workspace.add_panel(libp2p_ui.clone(), window, cx);

        workspace.register_action(|workspace, _: &ToggleLibp2pPanel, window, cx| {
            workspace.toggle_panel_focus::<Libp2pUi>(window, cx);
        });
    })
    .detach();
}

struct Libp2pUi {
    dock_position: DockPosition,
    focus_handle: FocusHandle,
    width: Option<Pixels>,
    local_peer_id: PeerId,
    peers: BTreeSet<PeerId>,
    peer_streams: BTreeMap<PeerId, libp2p_swarm::Stream>,
    _stream_control: Control,
}

impl Libp2pUi {
    pub fn new(cx: &mut Context<Self>, local_peer_id: PeerId, stream_control: Control) -> Self {
        cx.subscribe_self(|this, event: &SwarmEvent<PeerieBehaviourEvent>, cx| {
            match event {
                SwarmEvent::Behaviour(PeerieBehaviourEvent::Mdns(MdnsEvent::Discovered(peers))) => {
                    for (peer_id, _addr) in peers {
                        tracing::info!(?peer_id, "Added peer");
                        this.peers.insert(*peer_id);
                    }
                    cx.notify();
                }
                SwarmEvent::Behaviour(PeerieBehaviourEvent::Mdns(MdnsEvent::Expired(peers))) => {
                    for (peer_id, _addr) in peers {
                        tracing::info!(?peer_id, "Removed peer");
                        this.peers.remove(peer_id);
                    }
                    cx.notify();
                }
                _ => {
                    //
                }
            }
        })
        .detach();

        Self {
            dock_position: DockPosition::Left,
            focus_handle: cx.focus_handle(),
            width: None,
            local_peer_id,
            peers: Default::default(),
            peer_streams: Default::default(),
            _stream_control: stream_control,
        }
    }

    fn connect_stream(&mut self, remote_peer: PeerId, cx: &mut Context<Self>) {
        cx.spawn({
            let mut control = self._stream_control.clone();
            async move |ui, cx| {
                let stream = control.open_stream(remote_peer, PROTOCOL).await?;
                tracing::info!(%remote_peer, "Connecting outbound stream");
                ui.update(cx, |ui, _cx| {
                    ui.peer_streams.insert(remote_peer, stream);
                    tracing::debug!(%remote_peer, "Connected outbound stream");
                })?;
                anyhow::Ok(())
            }
        })
        .detach_and_log_err(cx);
    }
}

impl Render for Libp2pUi {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .border_1()
            .border_color(rgb(0xaa00bb))
            .child(
                div()
                    .w_full()
                    .border_1()
                    .border_color(rgb(0x440099))
                    .child(format!("Local Peer ID: {}", self.local_peer_id)),
            )
            .child(
                div()
                    .w_full()
                    .border_1()
                    .border_color(rgb(0x440099))
                    .child("Discovered Peers:"),
            )
            .children(
                self.peers
                    .iter()
                    .copied()
                    .filter(|it| it != &self.local_peer_id)
                    .enumerate()
                    .map(|(i, remote_peer)| {
                        ListItem::new(i)
                            .on_click(cx.listener(move |ui, _click, _window, cx| {
                                tracing::debug!("Clicked on peer {}", remote_peer);
                                ui.connect_stream(remote_peer, cx);
                            }))
                            .child(remote_peer.to_string())
                    }),
            )
    }
}

impl EventEmitter<PanelEvent> for Libp2pUi {}
impl EventEmitter<SwarmEvent<PeerieBehaviourEvent>> for Libp2pUi {}

impl Focusable for Libp2pUi {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Panel for Libp2pUi {
    fn persistent_name() -> &'static str {
        "Libp2p"
    }

    fn panel_key() -> &'static str {
        "libp2p"
    }

    fn position(&self, _window: &Window, _cx: &App) -> DockPosition {
        self.dock_position
    }

    fn position_is_valid(&self, _position: DockPosition) -> bool {
        true
    }

    fn set_position(
        &mut self,
        position: DockPosition,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        self.dock_position = position;
    }

    fn size(&self, _window: &Window, _cx: &App) -> Pixels {
        self.width.unwrap_or_else(|| px(300.))
    }

    fn set_size(&mut self, size: Option<Pixels>, _window: &mut Window, _cx: &mut Context<Self>) {
        self.width = size;
    }

    fn icon(&self, _window: &Window, _cx: &App) -> Option<IconName> {
        Some(IconName::Link)
    }

    fn icon_tooltip(&self, _window: &Window, _cx: &App) -> Option<&'static str> {
        Some("Libp2p")
    }

    fn toggle_action(&self) -> Box<dyn Action> {
        Box::new(ToggleLibp2pPanel)
    }

    fn activation_priority(&self) -> u32 {
        0
    }
}
