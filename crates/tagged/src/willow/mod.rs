use std::{collections::HashMap, marker::PhantomData, path::PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::warn;
use zed::unstable::{
    gpui::{
        self, Action, AppContext, Entity, EventEmitter, FocusHandle, Focusable, Global, actions,
    },
    paths,
    ui::{
        App, Context, ElementId, IconName, IntoElement, ParentElement as _, Pixels, Render,
        SharedString, Styled as _, Window, div, px,
    },
    workspace::{
        Panel, Workspace,
        dock::{DockPosition, PanelEvent},
    },
};

use crate::willow::{button_input::ButtonInput, profile::Profile, space::Space};

pub mod button_input;
pub mod object_widget;
pub mod profile;
pub mod space;

actions!(willow, [ToggleWillowPanel]);

pub fn init(cx: &mut App) {
    cx.observe_new(move |workspace: &mut Workspace, window, cx| {
        let Some(window) = window else {
            warn!("WillowUi: no Window in Workspace");
            return;
        };

        // Save workspace handle
        let workspace_entity = cx.entity();

        let store_path = paths::data_dir();
        let willow = Willow::new(store_path, cx);
        cx.set_global(GlobalWillow(willow));
        let willow_ui = cx.new(|cx| WillowUi::new(cx.willow(), workspace_entity.clone(), cx));

        workspace.add_panel(willow_ui.clone(), window, cx);
        workspace.toggle_panel_focus::<WillowUi>(window, cx);
    })
    .detach();
}

// Meta-object that coordinates all components tied to a given Willow store?
//
// In other words, actual UI components hold an entity to this to look up the
// coordinated visual state
pub struct WillowUi {
    create_profile: Entity<ButtonInput>,
    focus_handle: FocusHandle,
    width: Option<Pixels>,
    willow: Willow,
    workspace: Entity<Workspace>,
}

impl WillowUi {
    fn new(willow: Willow, workspace: Entity<Workspace>, cx: &mut Context<Self>) -> Self {
        let create_profile = cx.new(|cx| {
            ButtonInput::new("create-profile-input", "+ Profile".into(), cx)
                .placeholder_text("Profile name")
                .on_submit({
                    |this, text, _window, cx| {
                        // TODO better IDs
                        let id = format!("profile-{text}");
                        cx.willow().create_profile(SharedString::from(id), text, cx);
                        this.clear();
                        cx.notify();
                    }
                })
        });

        Self {
            create_profile,
            focus_handle: cx.focus_handle(),
            width: None,
            willow,
            workspace,
        }
    }
}

impl Render for WillowUi {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h_full()
            .w(self.width.unwrap_or(px(300.)) - px(1.))
            .flex()
            .flex_col()
            // Column-stacked user profiles
            .children(self.willow.profiles(cx))
            .child(
                div()
                    //
                    .px_2()
                    .py_4()
                    .child(self.create_profile.clone()),
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
    /// Local state per Willow instance
    state: Entity<WillowState>,
}

/// State of a Willow instance. Probably 1:1 with a "store" on disk at a given path
struct WillowState {
    spaces: Vec<Entity<Space>>,
    store_path: PathBuf,
    /// Payloads in simple impl are just bytes
    paths: HashMap<String, Vec<u8>>,
    profiles: Vec<Entity<Profile>>,
}

impl Willow {
    fn new(store_path: impl Into<PathBuf>, cx: &mut App) -> Self {
        let state = cx.new(|cx| WillowState::new(store_path.into(), cx));
        let willow = Self { state };

        willow
    }

    /// Returns None if no workspace is available
    ///
    /// Otherwise, creates a new Space as a workspace item
    fn create_space(&mut self, name: String, cx: &mut App) -> Option<Entity<Space>> {
        self.state.update(cx, |state, cx| {
            let space = cx.new(|cx| Space::new(name, cx));
            state.spaces.push(space.clone());
            Some(space)
        })
    }

    fn create_profile(
        &mut self,
        id: impl Into<ElementId>,
        name: String,
        cx: &mut App,
    ) -> Entity<Profile> {
        let profile = cx.new(|cx| Profile::new(id.into(), name, cx));
        self.state.update(cx, |state, _cx| {
            state.profiles.push(profile.clone());
        });
        profile
    }

    fn spaces(&self, cx: &mut App) -> impl IntoIterator<Item = Entity<Space>> {
        self.state.read(cx).spaces.clone()
    }

    fn profiles(&self, cx: &mut App) -> impl IntoIterator<Item = Entity<Profile>> {
        self.state.read(cx).profiles.clone()
    }

    /// ```rust,no-run
    /// #[derive(Debug, WillowObject)]
    /// struct ChatBubble {
    ///     #[willow(path = "content/")]
    ///     content: ChatContent,
    ///     #[willow(path = "sender.txt")]
    ///     sender: Profile,
    ///     #[willow(path = "signature.txt")]
    ///     signature: (),
    /// }
    /// let chat_feed: WillowFeed<ChatBubble> = cx.willow()
    ///     //
    ///     .create_feed::<ChatBubble>("/apps/chat/feeds/family/");
    /// ```
    ///
    /// Gotta find a better name than "object index"
    ///
    /// - It's just a folder that holds some kinds of objects
    /// - Like, "put my Chat" objects in "/apps/chat/feeds/family/"
    /// - Oh yeah, call it a feed?
    pub fn create_feed<T>(&self, path: &str) -> WillowFeed<T> {
        //
        todo!()
    }
}

impl WillowState {
    fn new(store_path: PathBuf, cx: &mut Context<Self>) -> Self {
        let spaces = vec![
            cx.new(|cx| {
                let mut space = Space::new("Home".to_string(), cx);
                space
            }),
            cx.new(|cx| {
                let mut space = Space::new("Family".to_string(), cx);
                space
            }),
        ];

        let profiles = vec![
            cx.new(|cx| {
                let mut profile = Profile::new("profile-0".into(), "Profile 0".to_string(), cx);
                profile.join_space(spaces[0].clone());
                profile.join_space(spaces[1].clone());
                profile.active_space = Some(spaces[0].clone());
                profile
            }),
            cx.new(|cx| {
                let mut profile = Profile::new("profile-1".into(), "Profile 1".to_string(), cx);
                profile.join_space(spaces[0].clone());
                profile.active_space = Some(spaces[0].clone());
                profile
            }),
        ];

        Self {
            spaces,
            store_path,
            paths: Default::default(),
            profiles,
        }
    }
}

pub trait WillowExt {
    fn willow(&mut self) -> Willow;
}

impl WillowExt for App {
    fn willow(&mut self) -> Willow {
        self.global::<GlobalWillow>().0.clone()
    }
}

pub struct WillowObject<T> {
    _phantom: PhantomData<T>,
}

pub struct WillowFeed<T> {
    _phantom: PhantomData<T>,
}

/// A Willow Entity is a handle representing an object with a well-known type
///
/// To be a somewhat complete and well-addressed handle, a WillowEntity includes
/// information about the namespace and subspace of the underlying Entry.
///
/// So an Entity is like an address/handle for an Area, so it's defined by its
/// namespace, subspace, and path prefix (directory). The definition of a Willow
/// Area also includes a time range, I want to think about how to represent time
/// in a dedicated brainstorm.
///
/// - Area in the spec has `subspace_id: SubspaceId | any`, which implies an
///   arbitrary restriction in the expressiveness of the API. I think it should
///   easily be possible to specify a list of subspaces we're interested in.
struct WillowEntity<T: WillowModel> {
    _phantom: PhantomData<T>,
}

struct WillowContext<T> {
    _phantom: PhantomData<T>,
}

impl<T: WillowModel> WillowEntity<T> {
    fn read(&self, cx: &mut WillowContext<T>) -> Option<&T> {
        None
    }
}

// WillowComponent?
// WillowSpec
// WillowArea
// WillowModel <-- expresses paths to multiple files, typed extractors
// - Model would refer to a multi-"file" data construction which is located
//   at a path and described by the set of files the model refers to, as well
//   as the types of those files.
pub trait WillowModel: JsonSchema + Serialize + for<'de> Deserialize<'de> {}
