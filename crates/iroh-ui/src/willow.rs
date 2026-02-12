// Cleaning up ideas from willow_whimsy

use std::{fmt::Display, path::PathBuf};

use serde::{Deserialize, Serialize};
use tracing::warn;
use zed::unstable::{
    gpui::{
        self, Action, AppContext, Entity, EventEmitter, FocusHandle, Focusable, Global, actions,
    },
    paths,
    ui::{
        App, Context, IconButton, IconName, IntoElement, ListItem, ParentElement as _, Pixels,
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
    let path = paths::data_dir();
    let willow = Willow::new(path, cx);
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
            // Column-stacked user profiles
            .child(self.render_users(window, cx))
    }
}

/// Subcomponent rendering
impl WillowUi {
    fn render_users(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .children(
                self.willow
                    //
                    .users(cx)
                    .into_iter()
                    .map(|user| user),
            )
    }

    /// Each user is rendered with a header and a collapsing namespaces section
    fn render_user(
        &mut self,
        user: User,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .debug()
            .child(self.render_user_header(&user, window, cx))
            .child(self.render_user_namespaces(&user, window, cx))
    }

    /// The user header should show a profile icon and user details
    fn render_user_header(
        &mut self,
        user: &User,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            // Render User header
            .child(
                ListItem::new(SharedString::from(format!("user-{}", user.id()))).child(
                    //
                    div()
                        .debug_border()
                        .p_4()
                        .flex()
                        .flex_row()
                        .child(IconButton::new(
                            SharedString::from(format!("user-toggle-{}", user.id())),
                            IconName::ChevronDown,
                        ))
                        .child(
                            //
                            user.name().to_string(),
                        ),
                ),
            )
    }

    /// Render the namespaces of a particular user
    fn render_user_namespaces(
        &mut self,
        user: &User,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .debug_border()
            .flex()
            .flex_row()
            // Vertical left, sidebar
            .child(self.render_namespaces_bar(user, window, cx))
            // Verticle right, directory
            .child(self.render_active_namespace(user, window, cx))
    }

    fn render_user_namespace(
        &mut self,
        user: &User,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            //
            .child(
                //
                div()
                    //
                    .pl_4()
                    .flex()
                    .flex_col()
                    .children(user.namespaces()),
            )
    }

    /// Render the namespaces bar for one user.
    fn render_namespaces_bar(
        &mut self,
        user: &User,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            //
            .debug_border()
            .p_2()
            .flex()
            .flex_col()
            .children(user.namespaces().into_iter().map(|namespace| {
                //
                ListItem::new(SharedString::from(format!("ns-TODO"))).child(
                    //
                    "namespace",
                )
            }))
    }

    /// Render the namespaces bar for one user.
    fn render_active_namespace(
        &mut self,
        user: &User,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .children(
                ["One", "Two", "Three"]
                    .into_iter()
                    .enumerate()
                    .map(|(i, name)| {
                        ListItem::new(SharedString::from(format!("ns-{}-{i}", name))).child(
                            //
                            name.to_string(),
                        )
                    }),
            )
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

    fn toggle_action(&self) -> Box<dyn Action> {
        Box::new(ToggleWillowPanel)
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
    path: PathBuf,
    /// Local state per Willow instance
    state: Entity<WillowState>,
}

/// State of a Willow instance. Probably 1:1 with a "store" on disk at a given path
struct WillowState {
    namespaces: Vec<Entity<Namespace>>,
    users: Vec<Entity<User>>,
}

impl Willow {
    fn new(path: impl Into<PathBuf>, cx: &mut App) -> Self {
        let state = cx.new(|cx| WillowState::new(cx));
        let willow = Self {
            path: path.into(),
            state,
        };

        willow
    }

    fn create_namespace(&mut self, name: String, cx: &mut Context<Self>) -> Entity<Namespace> {
        let namespace = cx.new(|cx| Namespace::new(name, cx));
        self.state.update(cx, |state, _cx| {
            state.namespaces.push(namespace.clone());
        });
        namespace
    }

    fn create_user(&mut self, id: String, name: String, cx: &mut Context<Self>) -> Entity<User> {
        let user = cx.new(|cx| User::new(id, name, cx));
        self.state.update(cx, |state, _cx| {
            state.users.push(user.clone());
        });
        user
    }

    fn namespaces(&self, cx: &mut App) -> impl IntoIterator<Item = Entity<Namespace>> {
        self.state.read(cx).namespaces.clone()
    }

    fn users(&self, cx: &mut App) -> impl IntoIterator<Item = Entity<User>> + use<> {
        self.state.read(cx).users.clone()
    }
}

impl WillowState {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            namespaces: vec![
                cx.new(|cx| Namespace::new("Namespace 0".to_string(), cx)),
                cx.new(|cx| Namespace::new("Namespace 1".to_string(), cx)),
            ],
            users: vec![
                cx.new(|cx| User::new("0".to_string(), "User 0".to_string(), cx)),
                cx.new(|cx| User::new("1".to_string(), "User 1".to_string(), cx)),
            ],
        }
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

#[derive(Clone)]
struct User {
    id: String,
    name: String,
    namespaces: Vec<Entity<Namespace>>,
}

impl Render for User {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl User {
    fn new(id: String, name: String, _cx: &mut Context<Self>) -> Self {
        Self {
            id,
            name,
            namespaces: vec![],
        }
    }

    pub fn id(&self) -> impl Display {
        &self.id
    }

    fn name(&self) -> impl Display {
        &self.name
    }

    pub fn namespaces(&self) -> impl IntoIterator<Item = Entity<Namespace>> {
        self.namespaces.clone()
    }
}

pub struct Namespace {
    name: String,
}

impl Render for Namespace {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl Namespace {
    fn new(name: impl Into<String>, _cx: &mut Context<Self>) -> Self {
        Self { name: name.into() }
    }

    pub fn name(&self) -> impl Display {
        self.name.to_string()
    }
}
