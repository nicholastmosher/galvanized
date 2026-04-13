use std::str::FromStr;
use std::sync::Arc;

use iroh::{EndpointAddr, EndpointId};
use rand::Rng;
use tracing::info;
use zed::unstable::editor::Editor;
use zed::unstable::gpui::{
    self, App, AppContext as _, ClipboardItem, Context, Element, Entity, EventEmitter, FocusHandle,
    Focusable, IntoElement, Length, ParentElement as _, Render, Styled, Window, div, rgb,
};
use zed::unstable::ui::{Button, Clickable, IconPosition, IconSize, LabelSize, ListItem};
use zed::unstable::workspace::Workspace;
use zed::unstable::workspace::dock::PanelEvent;
use zed::unstable::workspace::ui::FluentBuilder;
use zed::unstable::{
    gpui::{actions, px},
    workspace::{Panel, dock::DockPosition, ui::IconName},
};

use crate::iroh_repo::{GlobalIrohRepo, IrohRepository};

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
    iroh_repo: Option<Arc<IrohRepository>>,
    remote_endpoint_editor: Entity<Editor>,
}

impl IrohPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let iroh_repo = cx.global::<GlobalIrohRepo>().0.clone();
        cx.observe_global::<GlobalIrohRepo>(move |iroh_panel, cx| {
            let iroh_repo = cx.global::<GlobalIrohRepo>().0.clone();
            iroh_panel.iroh_repo = iroh_repo;
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
            iroh_repo,
            remote_endpoint_editor,
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
                    .child(
                        div()
                            .w(Length::Auto)
                            .map(|this| match self.iroh_repo.as_ref() {
                                None => this.child("No endpoint".into_any()),
                                Some(repo) => {
                                    let endpoint_id = repo.proto.endpoint().id().to_string();
                                    this.child(
                                        format!("Iroh ID: {}", endpoint_id).into_any_element(),
                                    )
                                }
                            }),
                    )
                    .child(
                        Button::new("copy-endpoint-id", "Copy")
                            .label_size(LabelSize::Small)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                let repo = this
                                    .iroh_repo
                                    .as_ref()
                                    .map(|repo| repo.proto.endpoint().id().to_string())
                                    .unwrap_or_else(|| "<no endpoint>".to_string());
                                info!(%repo, "Clicked Copy");
                                cx.write_to_clipboard(ClipboardItem::new_string(repo));
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
                            .on_click(cx.listener(|this, _, _window, cx| {
                                let Some(repo) = this.iroh_repo.as_ref() else {
                                    return;
                                };

                                let remote_id = this.remote_endpoint_editor.read(cx).text(cx);
                                let remote_id =
                                    EndpointId::from_str(&remote_id).expect("Invalid endpoint ID");
                                let endpoint_id: EndpointAddr = EndpointAddr::from(remote_id);

                                cx.spawn({
                                    let repo = repo.clone();
                                    async move |_a, _b| {
                                        let _it = repo
                                            .proto
                                            .sync_with(endpoint_id)
                                            .await
                                            .expect("connect");
                                    }
                                })
                                .detach();
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
