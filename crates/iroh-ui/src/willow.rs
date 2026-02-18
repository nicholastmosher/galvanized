// Cleaning up ideas from willow_whimsy

use std::{collections::HashMap, fmt::Display, path::PathBuf, rc::Rc};

use tracing::{info, warn};
use zed::unstable::{
    component,
    editor::Editor,
    gpui::{
        self, Action, AppContext, Entity, EventEmitter, FocusHandle, Focusable, Global, actions,
    },
    paths,
    ui::{
        ActiveTheme as _, App, Clickable, Component, Context, ElementId, FluentBuilder, IconButton,
        IconName, IconSize, InteractiveElement, IntoElement, ListItem, ParentElement as _, Pixels,
        RegisterComponent, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
        Window, div, px,
    },
    workspace::{
        Panel, Workspace,
        dock::{DockPosition, PanelEvent},
    },
};

actions!(willow, [ToggleWillowPanel]);

pub fn init(cx: &mut App) {
    let store_path = paths::data_dir();
    let willow = Willow::new(store_path, cx);
    cx.set_global(GlobalWillow(willow));

    let willow_ui = cx.new(|cx| WillowUi::new(cx.willow(), cx));
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
    width: Option<Pixels>,
    willow: Willow,
    create_profile: Entity<ButtonInput>,
}

impl WillowUi {
    fn new(willow: Willow, cx: &mut Context<Self>) -> Self {
        let create_profile = cx.new(|cx| {
            ButtonInput::new("create-profile-input", "+ Profile".into(), cx)
                .placeholder_text("Profile name")
                .on_submit({
                    |this, text, _window, cx| {
                        // TODO better IDs
                        let id = format!("profile-{text}");
                        cx.willow().create_profile(SharedString::from(id), text, cx);
                        this.editor.take();
                        cx.notify();
                    }
                })
        });
        Self {
            focus_handle: cx.focus_handle(),
            width: None,
            willow,
            create_profile,
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
    namespaces: Vec<Entity<Namespace>>,
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

    fn create_namespace(&mut self, name: String, cx: &mut App) -> Entity<Namespace> {
        let namespace = cx.new(|cx| Namespace::new(name, cx));
        self.state.update(cx, |state, _cx| {
            state.namespaces.push(namespace.clone());
        });
        namespace
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

    fn namespaces(&self, cx: &mut App) -> impl IntoIterator<Item = Entity<Namespace>> {
        self.state.read(cx).namespaces.clone()
    }

    fn profiles(&self, cx: &mut App) -> impl IntoIterator<Item = Entity<Profile>> {
        self.state.read(cx).profiles.clone()
    }
}

impl WillowState {
    fn new(store_path: PathBuf, cx: &mut Context<Self>) -> Self {
        let namespaces = vec![
            cx.new(|cx| {
                let mut namespace = Namespace::new("Home".to_string(), cx);
                namespace.create_entry("entry/0".to_string());
                namespace.create_entry("entry/1".to_string());
                namespace.create_entry("entry/2".to_string());
                namespace.create_entry("entry/3".to_string());
                namespace
            }),
            cx.new(|cx| {
                let mut namespace = Namespace::new("Family".to_string(), cx);
                namespace.create_entry("entry/4".to_string());
                namespace.create_entry("entry/5".to_string());
                namespace.create_entry("entry/6".to_string());
                namespace.create_entry("entry/7".to_string());
                namespace
            }),
            cx.new(|cx| {
                let mut namespace = Namespace::new("Work".to_string(), cx);
                namespace.create_entry("entry/8".to_string());
                namespace.create_entry("entry/9".to_string());
                namespace.create_entry("entry/10".to_string());
                namespace.create_entry("entry/11".to_string());
                namespace
            }),
        ];

        let profiles = vec![
            cx.new(|cx| {
                let mut profile = Profile::new("profile-0".into(), "Profile 0".to_string(), cx);
                profile.join_namespace(namespaces[0].clone());
                profile.join_namespace(namespaces[1].clone());
                profile.join_namespace(namespaces[2].clone());
                profile.active_namespace = Some(namespaces[0].clone());
                profile
            }),
            cx.new(|cx| {
                let mut profile = Profile::new("profile-1".into(), "Profile 1".to_string(), cx);
                profile.join_namespace(namespaces[0].clone());
                profile.join_namespace(namespaces[1].clone());
                profile.active_namespace = Some(namespaces[0].clone());
                profile
            }),
            cx.new(|cx| {
                let mut profile = Profile::new("profile-2".into(), "Profile 2".to_string(), cx);
                profile.join_namespace(namespaces[1].clone());
                profile.join_namespace(namespaces[2].clone());
                profile.active_namespace = Some(namespaces[1].clone());
                profile
            }),
        ];

        Self {
            namespaces,
            store_path,
            paths: Default::default(),
            profiles,
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
struct Profile {
    active_namespace: Option<Entity<Namespace>>,
    name: String,
    namespaces: Vec<Entity<Namespace>>,
    create_namespace: Entity<ButtonInput>,
    open: bool,
}

impl Render for Profile {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .child(self.render_profile_header(window, cx))
            .when(self.open, |div| {
                div.child(self.render_profile_namespaces(window, cx))
            })
    }
}

impl Profile {
    /// The user header should show a profile icon and user details
    fn render_profile_header(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            //
            .border_t_2()
            .border_color(cx.theme().colors().border.opacity(0.6))
            .bg(cx.theme().colors().ghost_element_background)
            .child(
                ListItem::new(SharedString::from(format!("user-{}", self.name())))
                    .child(
                        div().p_2().child(
                            div()
                                .py_2()
                                .flex()
                                .flex_row()
                                .child(IconButton::new(
                                    SharedString::from(format!("user-toggle-{}", self.name())),
                                    {
                                        if self.open {
                                            IconName::ChevronDown
                                        } else {
                                            IconName::ChevronRight
                                        }
                                    },
                                ))
                                .child(
                                    //
                                    self.name().to_string(),
                                ),
                        ),
                    )
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.open = !this.open;
                        cx.notify();
                    })),
            )
    }

    /// Render the namespaces of a particular user
    fn render_profile_namespaces(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_row()
                    // Vertical left, sidebar
                    .child(self.render_namespaces_bar(window, cx))
                    // Verticle right, directory
                    .child(self.render_active_namespace(window, cx)),
            )
            // When creating namespace, render ItemAdd widget
            .when(self.create_namespace.read(cx).editor.is_some(), |this| {
                //
                this.child(
                    div()
                        //
                        .px_2()
                        .pb_2()
                        .child(
                            //
                            self.create_namespace.clone(),
                        ),
                )
            })
    }

    /// Render the namespaces bar for one user.
    fn render_namespaces_bar(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .p_2()
            .flex()
            .flex_col()
            .gap_2()
            .border_r_1()
            .border_color(cx.theme().colors().border.opacity(0.6))
            // Each namespace in this profile
            .children(self.namespaces().into_iter().map(|namespace| {
                let ns = namespace.read(cx);
                div()
                    .id(SharedString::from(format!("ns-{}", ns.name())))
                    .p_4()
                    .border_1()
                    .rounded_lg()
                    .border_color(cx.theme().colors().border.opacity(0.6))
                    .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                    .hover(|style| {
                        style
                            .bg(cx.theme().colors().ghost_element_hover)
                            .border_color(cx.theme().colors().border.opacity(1.0))
                    })
                    .on_click(cx.listener(move |this, _event, _window, _cx| {
                        this.active_namespace = Some(namespace.clone());
                    }))
                    .child(
                        //
                        ns.name().to_string(),
                    )
            }))
            // Push the add-namespace button to the bottom
            .child(div().flex_grow())
            // New namespace + Icon button, only when not actively adding
            .when_none(&self.create_namespace.read(cx).editor, |this| {
                //
                this.child(
                    div()
                        //
                        .id("create-namespace-mini")
                        .p_4()
                        .border_1()
                        .rounded_lg()
                        .justify_center()
                        .border_color(cx.theme().colors().border.opacity(0.6))
                        .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                        .hover(|style| {
                            style
                                .bg(cx.theme().colors().ghost_element_hover)
                                .border_color(cx.theme().colors().border.opacity(1.0))
                        })
                        .child(
                            IconButton::new("create-namespace-mini-icon", IconName::Plus).on_click(
                                cx.listener(|this, _event, window, cx| {
                                    info!("Clicked Create Namespace");
                                    this.create_namespace.update(cx, |this, cx| {
                                        this.editor = Some(cx.new(|cx| {
                                            let mut editor = Editor::single_line(window, cx);
                                            if let Some(placeholder) = &this.placeholder {
                                                editor.set_placeholder_text(
                                                    &**placeholder,
                                                    window,
                                                    cx,
                                                );
                                            }
                                            editor
                                        }));
                                        cx.notify();
                                    });
                                }),
                            ),
                        ),
                )
            })
    }

    /// Render the namespaces bar for one user.
    fn render_active_namespace(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            //
            .flex_grow()
            .p_2()
            .flex()
            .flex_col()
            .when_some(self.active_namespace.as_ref(), |div, namespace| {
                div.child(namespace.clone())
            })
    }
}

impl Profile {
    fn new(id: ElementId, name: String, cx: &mut Context<Self>) -> Self {
        let create_namespace = cx.new(|cx| {
            ButtonInput::new(
                SharedString::from(format!("{id}-create-namespace")),
                "+ Namespace".into(),
                cx,
            )
            .placeholder_text("Create namespace")
            .on_submit({
                let id = id.clone();
                move |_this, _text, _window, _cx| {
                    info!("Submitted create namespace {id}");
                }
            })
        });
        Self {
            active_namespace: None,
            name,
            namespaces: vec![],
            create_namespace,
            open: true,
        }
    }

    fn name(&self) -> impl Display {
        &self.name
    }

    pub fn join_namespace(&mut self, namespace: Entity<Namespace>) {
        self.namespaces.push(namespace);
    }

    pub fn namespaces(&self) -> Vec<Entity<Namespace>> {
        self.namespaces.clone()
    }
}

pub struct Namespace {
    name: String,
    entries: Vec<String>,
}

impl Render for Namespace {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .children(self.entries().into_iter().enumerate().map(|(i, entry)| {
                //
                ListItem::new(SharedString::from(format!("ns-entry-{i}")))
                    .rounded()
                    .child(
                        //
                        div()
                            //
                            .p_2()
                            .child(format!("{}/{}", self.name(), entry)),
                    )
            }))
    }
}

impl Namespace {
    fn new(name: impl Into<String>, _cx: &mut Context<Self>) -> Self {
        Self {
            name: name.into(),
            entries: Default::default(),
        }
    }

    pub fn create_entry(&mut self, entry: String) {
        self.entries.push(entry);
    }

    pub fn name(&self) -> impl Display {
        self.name.to_string()
    }

    pub fn entries(&self) -> impl IntoIterator<Item = &String> {
        self.entries.iter()
    }
}

#[derive(RegisterComponent)]
struct ButtonInput {
    id: ElementId,
    name: SharedString,
    placeholder: Option<SharedString>,
    editor: Option<Entity<Editor>>,
    on_submit: Option<Rc<dyn Fn(&mut Self, String, &mut Window, &mut Context<Self>)>>,
}

impl Component for ButtonInput {
    fn preview(_window: &mut Window, cx: &mut App) -> Option<gpui::AnyElement> {
        let ui = cx.new(|cx| {
            Self::new("sample", "Sample".into(), cx)
                .placeholder_text("Input here")
                .on_submit(|_this, _text, _window, _cx| {
                    info!("Sample OnClick");
                })
        });
        let container = div().p_2().child(ui);
        Some(container.into_any_element())
    }
}

impl ButtonInput {
    pub fn new(id: impl Into<ElementId>, name: SharedString, _cx: &mut Context<Self>) -> Self {
        Self {
            id: id.into(),
            name,
            placeholder: None,
            editor: None,
            on_submit: None,
        }
    }

    pub fn placeholder_text(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    pub fn on_submit(
        mut self,
        on_submit: impl Fn(&mut Self, String, &mut Window, &mut Context<Self>) + 'static,
    ) -> Self {
        self.on_submit = Some(Rc::new(on_submit));
        self
    }
}

impl Render for ButtonInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .id(self.id.clone())
            .text_center()
            .justify_center()
            .border_2()
            .border_dashed()
            .border_color(cx.theme().colors().border.opacity(0.6))
            .rounded_sm()
            .when_none(&self.editor, |this| {
                //
                this
                    //
                    .px_2()
                    .py_4()
                    .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                    .hover(|style| {
                        style
                            .bg(cx.theme().colors().ghost_element_hover)
                            .border_color(cx.theme().colors().border.opacity(1.0))
                    })
                    // .child(self.name.clone())
                    .child(
                        div()
                            //
                            .text_color(cx.theme().colors().text_muted)
                            .child(
                                //
                                self.name.clone(),
                            ),
                    )
                    .on_click(cx.listener(|this, _event, window, cx| {
                        //
                        this.editor = Some(
                            //
                            cx.new(|cx| {
                                let mut editor = Editor::single_line(window, cx);
                                if let Some(placeholder) = &this.placeholder {
                                    editor.set_placeholder_text(&**placeholder, window, cx);
                                }
                                editor
                            }),
                        );
                        cx.notify();
                    }))
            })
            .when_some(self.editor.as_ref(), |this, editor| {
                //
                this
                    //
                    .h_full()
                    .w_full()
                    .flex()
                    .flex_row()
                    .child(
                        //
                        div()
                            //
                            .id(SharedString::from(format!(
                                "{}-create-profile-cancel",
                                self.id
                            )))
                            .p_4()
                            .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                            .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                            .child(
                                IconButton::new(
                                    SharedString::from(format!(
                                        "{}-create-profile-cancel-icon",
                                        self.id
                                    )),
                                    IconName::XCircle,
                                )
                                .icon_size(IconSize::Medium),
                            )
                            .on_click(cx.listener(|this, _event, _window, _cx| {
                                this.editor.take();
                            })),
                    )
                    .child(
                        div()
                            //
                            .px_2()
                            .py_4()
                            .flex_grow()
                            .child(editor.clone()),
                    )
                    .child(
                        //
                        div()
                            //
                            // .id("create-profile-submit")
                            .id(SharedString::from(format!(
                                "{}-create-profile-submit",
                                self.id
                            )))
                            .p_4()
                            .active(|style| style.bg(cx.theme().colors().ghost_element_active))
                            .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                            .child(IconButton::new(
                                SharedString::from(format!(
                                    "{}-create-profile-submit-icon",
                                    self.id
                                )),
                                IconName::ChevronRight,
                            ))
                            .on_click(cx.listener({
                                let editor = editor.clone();
                                move |this, _event, window, cx| {
                                    let name = editor.read(cx).text(cx);
                                    if let Some(on_submit) = this.on_submit.clone() {
                                        (on_submit)(this, name, window, cx)
                                    }
                                }
                            })),
                    )
            })
    }
}
