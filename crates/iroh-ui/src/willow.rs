// Cleaning up ideas from willow_whimsy

use serde::{Deserialize, Serialize};
use tracing::warn;
use zed::unstable::{
    gpui::{self, AppContext, Entity, EventEmitter, FocusHandle, Focusable, Global, actions},
    ui::{
        App, Context, FluentBuilder, IconName, IntoElement, ListItem, ParentElement as _, Pixels,
        Render, SharedString, Styled as _, Window, div, px,
    },
    workspace::{
        Panel, Workspace,
        dock::{DockPosition, PanelEvent},
    },
};

use crate::DebugViewExt as _;

actions!(willow, [ToggleWillowPanel]);

pub fn init(cx: &mut App) {
    let willow = Willow::new(cx);
    cx.set_global(GlobalWillow(willow.clone()));
    let willow_ui = cx.new(|cx| WillowUi::new(willow, cx));

    cx.observe_new({
        let willow_ui = willow_ui.clone();
        move |workspace: &mut Workspace, window, cx| {
            let Some(window) = window else {
                warn!("WillowUi: no Window in Workspace");
                return;
            };

            workspace.add_panel(willow_ui.clone(), window, cx);
            workspace.toggle_panel_focus::<WillowUi>(window, cx);
        }
    })
    .detach();
}

pub struct WillowUi {
    focus_handle: FocusHandle,
    selected_namespace: Option<Entity<Namespace>>,
    width: Option<Pixels>,
    willow: Willow,
}

/// Serialized to KEY_VALUE_STORE for persistence
#[derive(Debug, Serialize, Deserialize)]
struct WillowUiState {
    //
}

impl WillowUi {
    fn new(willow: Willow, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            selected_namespace: None,
            width: None,
            willow,
        }
    }
}

impl Render for WillowUi {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_border()
            .h_full()
            .w(self.width.unwrap_or(px(300.)) - px(1.))
            .flex()
            .flex_col()
            // Vertical left, sidebar
            .child(self.render_namespaces(window, cx))
            // Verticle right, directory
            .child(self.render_namespace_feed(window, cx))
    }
}

/// Subcomponent rendering
impl WillowUi {
    fn render_namespaces(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            //
            .p_2()
            .flex()
            .flex_col()
            .children(
                self.willow
                    .namespaces()
                    .into_iter()
                    .enumerate()
                    .map(|(i, namespace)| {
                        ListItem::new(SharedString::from(format!(
                            "namespace-{}-{i}",
                            namespace.name()
                        )))
                        .child(namespace.name().to_string())
                    }),
            )
    }

    fn render_namespace_feed(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div().when_some(self.selected_namespace.as_ref(), |div, namespace| {
            div.child(namespace.clone())
        })
    }
}

impl EventEmitter<PanelEvent> for WillowUi {}
impl Focusable for WillowUi {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
impl Panel for WillowUi {
    fn persistent_name() -> &'static str {
        "Willow"
    }

    fn panel_key() -> &'static str {
        "willow"
    }

    fn position(&self, _window: &Window, _cx: &App) -> DockPosition {
        DockPosition::Left
    }

    fn position_is_valid(&self, _position: DockPosition) -> bool {
        true
    }

    fn set_position(
        &mut self,
        _position: DockPosition,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    fn size(&self, _window: &Window, _cx: &App) -> Pixels {
        self.width.unwrap_or(px(300.))
    }

    fn set_size(&mut self, size: Option<Pixels>, _window: &mut Window, _cx: &mut Context<Self>) {
        self.width = size;
    }

    fn icon(&self, _window: &Window, _cx: &App) -> Option<IconName> {
        Some(IconName::Hash)
    }

    fn icon_tooltip(&self, _window: &Window, _cx: &App) -> Option<&'static str> {
        Some("Willow")
    }

    fn toggle_action(&self) -> Box<dyn zed::unstable::gpui::Action> {
        todo!()
    }

    fn activation_priority(&self) -> u32 {
        0
    }
}

// =====

impl Global for GlobalWillow {}
struct GlobalWillow(Willow);

/// Willow API entrypoint
///
/// Willow "store" level operations
#[derive(Clone)]
pub struct Willow {
    /// Local state per Willow instance
    state: Entity<WillowState>,
}

/// State of a Willow instance. Probably 1:1 with a "store" on disk at a given path
struct WillowState {
    namespaces: Vec<Namespace>,
}

impl WillowState {
    fn new(_cx: &mut Context<Self>) -> Self {
        Self {
            namespaces: Default::default(),
        }
    }
}

impl Willow {
    fn new(cx: &mut App) -> Self {
        let state = cx.new(|cx| WillowState::new(cx));
        let willow = Self {
            //
            state,
        };

        willow
    }

    fn namespaces(&self) -> impl IntoIterator<Item = Namespace> {
        []
    }
}

trait WillowExt {
    fn willow(&mut self) -> Willow;
}

impl WillowExt for App {
    fn willow(&mut self) -> Willow {
        self.global::<GlobalWillow>().0.clone()
    }
}

pub struct Namespace {
    name: String,
    willow: Willow,
}

impl Render for Namespace {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl Namespace {
    fn new(willow: Willow) -> Self {
        Self {
            name: "<Namespace name>".to_string(),
            willow,
        }
    }

    pub fn name(&self) -> impl std::fmt::Display {
        self.name.to_string()
    }
}
