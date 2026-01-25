use anyhow::bail;
use iroh::Endpoint;
use iroh::protocol::{ProtocolHandler, Router};
use iroh_blobs::store::mem::MemStore;
use iroh_blobs::{ALPN as BLOBS_ALPN, BlobsProtocol};
use iroh_docs::{ALPN as DOCS_ALPN, protocol::Docs};
use iroh_gossip::{ALPN as GOSSIP_ALPN, Gossip};
use rand::Rng;
use tracing::{info, warn};
use zed::unstable::editor::Editor;
use zed::unstable::gpui::{
    self, App, AppContext as _, Context, Element, Entity, EventEmitter, FocusHandle, Focusable,
    ParentElement as _, Render, Styled, Window, div, rgb,
};
use zed::unstable::ui::{Button, Clickable, IconPosition, IconSize, LabelSize, ListItem};
use zed::unstable::workspace::Workspace;
use zed::unstable::workspace::dock::PanelEvent;
use zed::unstable::{
    gpui::{actions, px},
    workspace::{Panel, dock::DockPosition, ui::IconName},
};

trait DebugViewExt: Styled {
    fn debug_border(self) -> Self {
        self.border_1().border_color(rgb(rand::rng().random()))
    }
}
impl<T: Styled> DebugViewExt for T {}

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, window, cx| {
        let Some(window) = window else {
            return;
        };

        workspace.register_action(|workspace, _: &ToggleIrohPanel, window, cx| {
            workspace.toggle_panel_focus::<IrohPanel>(window, cx);
        });

        let iroh_panel = cx.new(|cx| IrohPanel::new(window, cx));
        workspace.add_panel(iroh_panel, window, cx);
    })
    .detach();
}

actions!(workspace, [ToggleIrohPanel]);

pub struct IrohPanel {
    dock_position: DockPosition,
    focus_handle: FocusHandle,
    remote_endpoint_editor: Entity<Editor>,
    iroh: Option<(Endpoint, Router)>,
}

#[derive(Debug, Clone)]
struct Handler;
impl Handler {
    const APLN: &[u8] = b"/test/docs";
}
impl ProtocolHandler for Handler {
    fn accept(
        &self,
        connection: iroh::endpoint::Connection,
    ) -> impl Future<Output = Result<(), iroh::protocol::AcceptError>> + Send {
        async move {
            //
            info!("Accepted inbound connection");
            Ok(())
        }
    }
}

impl IrohPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Initialize Iroh endpoint
        cx.spawn({
            async move |panel, cx| {
                let Some(panel) = panel.upgrade() else {
                    warn!("Iroh panel not found");
                    bail!("iroh panel not found");
                };

                let endpoint = Endpoint::builder().bind().await?;
                let store = MemStore::new();
                let blobs = BlobsProtocol::new(&store, None);
                let gossip = iroh_gossip::Gossip::builder().spawn(endpoint.clone());
                let docs = Docs::memory()
                    .spawn(endpoint.clone(), (*blobs).clone(), gossip.clone())
                    .await?;
                let router = Router::builder(endpoint.clone())
                    .accept(BLOBS_ALPN, blobs)
                    .accept(GOSSIP_ALPN, gossip)
                    .accept(DOCS_ALPN, docs)
                    .accept(Handler::APLN, Handler)
                    .spawn();
                panel.update(cx, move |panel, _cx| {
                    panel.iroh = Some((endpoint, router));
                })?;

                anyhow::Ok(())
            }
        })
        .detach();

        let remote_endpoint_editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_placeholder_text("Remote endpoint URL", window, cx);
            editor
        });

        Self {
            dock_position: DockPosition::Left,
            focus_handle: cx.focus_handle(),
            remote_endpoint_editor,
            iroh: None,
        }
    }

    fn _remote_endpoint(&self, cx: &App) -> String {
        self.remote_endpoint_editor.read(cx).text(cx)
    }
}

impl EventEmitter<PanelEvent> for IrohPanel {}

impl Focusable for IrohPanel {
    fn focus_handle(&self, _cx: &gpui::App) -> gpui::FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for IrohPanel {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        div()
            .size_full()
            .debug_border()
            .flex_col()
            .child(
                // Header
                div()
                    .w_full()
                    .p_1()
                    .flex_row()
                    .debug_border()
                    // .child(
                    //     div()
                    //         .w(Length::Auto)
                    //         .map(|this| match self.iroh_repo.as_ref() {
                    //             None => this.child("No endpoint".into_any()),
                    //             Some(repo) => {
                    //                 let endpoint_id = repo.proto.endpoint().id().to_string();
                    //                 this.child(
                    //                     format!("Iroh ID: {}", endpoint_id).into_any_element(),
                    //                 )
                    //             }
                    //         }),
                    // )
                    .child(
                        Button::new("copy-endpoint-id", "Copy")
                            .label_size(LabelSize::Small)
                            .icon(IconName::Plus)
                            .icon_size(IconSize::Small)
                            .icon_position(IconPosition::Start)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                info!("Clicked Copy");
                            })),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .p_1()
                    .debug_border()
                    .flex()
                    .gap_2()
                    .child(self.remote_endpoint_editor.clone())
                    .child(
                        Button::new("remote-connect", "Connect")
                            .label_size(LabelSize::Small)
                            .icon(IconName::Plus)
                            .icon_size(IconSize::Small)
                            .icon_position(IconPosition::Start)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                info!("Clicked Connect");
                            })),
                    ),
            )
            .child(
                //
                div()
                    .size_full()
                    .p_1()
                    .debug_border()
                    .child("Content".into_any())
                    .children(
                        ["One", "Two", "Three"]
                            .into_iter()
                            .enumerate()
                            .map(|(i, name)| ListItem::new(i).child(name.into_any())),
                    ),
            )
    }
}

impl Panel for IrohPanel {
    fn persistent_name() -> &'static str {
        "Iroh"
    }

    fn panel_key() -> &'static str {
        "iroh-panel"
    }

    fn position(
        &self,
        _window: &zed::unstable::gpui::Window,
        _cx: &zed::unstable::gpui::App,
    ) -> zed::unstable::workspace::dock::DockPosition {
        self.dock_position
    }

    fn position_is_valid(&self, _position: zed::unstable::workspace::dock::DockPosition) -> bool {
        true
    }

    fn set_position(
        &mut self,
        position: zed::unstable::workspace::dock::DockPosition,
        _window: &mut zed::unstable::gpui::Window,
        _cx: &mut zed::unstable::gpui::Context<Self>,
    ) {
        self.dock_position = position;
    }

    fn size(
        &self,
        _window: &zed::unstable::gpui::Window,
        _cx: &zed::unstable::gpui::App,
    ) -> zed::unstable::gpui::Pixels {
        px(300.)
    }

    fn set_size(
        &mut self,
        _size: Option<zed::unstable::gpui::Pixels>,
        _window: &mut zed::unstable::gpui::Window,
        _cx: &mut zed::unstable::gpui::Context<Self>,
    ) {
    }

    fn icon(
        &self,
        _window: &zed::unstable::gpui::Window,
        _cx: &zed::unstable::gpui::App,
    ) -> Option<zed::unstable::workspace::ui::IconName> {
        Some(IconName::Link)
    }

    fn icon_tooltip(
        &self,
        _window: &zed::unstable::gpui::Window,
        _cx: &zed::unstable::gpui::App,
    ) -> Option<&'static str> {
        Some("Iroh")
    }

    fn toggle_action(&self) -> Box<dyn zed::unstable::gpui::Action> {
        Box::new(ToggleIrohPanel)
    }

    fn activation_priority(&self) -> u32 {
        0
    }
}
