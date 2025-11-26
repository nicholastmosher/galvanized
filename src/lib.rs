// mod willow_panel_settings;

use anyhow::{Context as _, bail};
use db::kvp::KEY_VALUE_STORE;
use gpui::{prelude::FluentBuilder, *};
use serde::{Deserialize, Serialize};
use util::ResultExt as _;

// use willow_panel_settings::WillowPanelSettings;
use workspace::{
    Panel, Workspace,
    dock::{DockPosition, PanelEvent},
    ui::{
        ActiveTheme, Clickable, Color, Icon, IconButton, IconName, IconSize, Label, LabelCommon,
        LabelSize, ListItem, ListItemSpacing, h_flex, v_flex,
    },
};

actions!(
    workspace,
    [
        ToggleFocus,
        OpenWillow,
        CreateDocument,
        CreateSubspace,
        ViewDocument,
        DeleteDocument,
        RefreshDocuments,
        CloseDialog,
        SaveDocument,
    ]
);

pub fn init(cx: &mut App) {
    // WillowPanelSettings::register(cx);

    cx.observe_new(
        |workspace: &mut Workspace, window, cx: &mut Context<Workspace>| {
            let Some(window) = window else { return };

            workspace.register_action(|workspace, _: &ToggleFocus, window, cx| {
                workspace.toggle_panel_focus::<WillowPanel>(window, cx);
            });

            cx.spawn_in(window, async move |workspace_handle, cx| {
                let willow_panel = WillowPanel::load(workspace_handle.clone(), cx.clone()).await;
                let Ok(willow_panel) = willow_panel else {
                    bail!("missing willow panel");
                };

                workspace_handle.update_in(cx, move |workspace, window, cx| {
                    workspace.add_panel(willow_panel, window, cx);
                })?;

                Ok(())
            })
            .detach();
        },
    )
    .detach();
}

const WILLOW_PANEL_KEY: &str = "WillowPanel";

#[derive(Serialize, Deserialize)]
struct SerializedWillowPanel {
    width: Option<Pixels>,
}

#[derive(Debug, Clone)]
pub struct WillowDocument {
    pub id: String,
    pub path: String,
    pub content: String,
    pub author: String,
    pub timestamp: String,
    pub size: usize,
}

#[derive(Debug, Clone)]
pub struct WillowSubspace {
    pub id: String,
    pub name: String,
    pub document_count: usize,
    pub created_at: String,
}

#[derive(Debug, Clone)]
enum DialogState {
    None,
    CreateDocument { path: String, content: String },
    CreateSubspace {},
    ViewDocument { document: WillowDocument },
}

pub struct WillowPanel {
    width: Option<Pixels>,
    pending_serialization: Task<Option<()>>,
    focus_handle: FocusHandle,
    documents: Vec<WillowDocument>,
    subspaces: Vec<WillowSubspace>,
    dialog_state: DialogState,
    // TODO: Add Willow store when dependencies are working
    // store: Arc<WillowStore>,
}

impl WillowPanel {
    pub async fn load(
        workspace: WeakEntity<Workspace>,
        mut cx: AsyncWindowContext,
    ) -> anyhow::Result<Entity<Self>> {
        let serialized_panel = cx
            .background_executor()
            .spawn(async move { KEY_VALUE_STORE.read_kvp(WILLOW_PANEL_KEY) })
            .await
            .context("loading willow panel")
            .log_err()
            .flatten()
            .map(|panel| serde_json::from_str::<SerializedWillowPanel>(&panel))
            .transpose()
            .log_err()
            .flatten();

        workspace.update_in(&mut cx, |workspace, window, cx| {
            let panel = Self::new(workspace, window, cx);
            if let Some(serialized_panel) = serialized_panel {
                panel.update(cx, |panel, cx| {
                    panel.width = serialized_panel.width.map(|px| px.round());
                    cx.notify();
                });
            }
            panel
        })
    }

    pub fn new(
        workspace: &mut Workspace,
        _window: &mut Window,
        cx: &mut Context<Workspace>,
    ) -> Entity<Self> {
        let _user_store = workspace.app_state().user_store.clone();

        cx.new(|cx| {
            let panel = Self {
                width: None,
                pending_serialization: Task::ready(None),
                focus_handle: cx.focus_handle(),
                documents: vec![
                    // Sample data for demonstration
                    WillowDocument {
                        id: "doc1".to_string(),
                        path: "notes/meeting.txt".to_string(),
                        content: "Meeting notes from today's standup...".to_string(),
                        author: "alice@example.com".to_string(),
                        timestamp: "2024-01-15 10:30".to_string(),
                        size: 156,
                    },
                    WillowDocument {
                        id: "doc2".to_string(),
                        path: "docs/readme.md".to_string(),
                        content: "# Project Documentation\n\nThis is the main documentation..."
                            .to_string(),
                        author: "bob@example.com".to_string(),
                        timestamp: "2024-01-14 15:45".to_string(),
                        size: 234,
                    },
                ],
                subspaces: vec![
                    WillowSubspace {
                        id: "subspace1".to_string(),
                        name: "Personal Notes".to_string(),
                        document_count: 1,
                        created_at: "2024-01-10".to_string(),
                    },
                    WillowSubspace {
                        id: "subspace2".to_string(),
                        name: "Project Docs".to_string(),
                        document_count: 1,
                        created_at: "2024-01-12".to_string(),
                    },
                ],
                dialog_state: DialogState::None,
            };

            panel
        })
    }

    pub fn create_document(
        &mut self,
        _action: &CreateDocument,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dialog_state = DialogState::CreateDocument {
            path: String::new(),
            content: String::new(),
        };
        cx.notify();
    }

    pub fn create_subspace(
        &mut self,
        _action: &CreateSubspace,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dialog_state = DialogState::CreateSubspace {};
        cx.notify();
    }

    pub fn close_dialog(
        &mut self,
        _action: &CloseDialog,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dialog_state = DialogState::None;
        cx.notify();
    }

    pub fn save_document(
        &mut self,
        _action: &SaveDocument,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // For demo purposes, create a mock document
        if let DialogState::CreateDocument { path, content } = &self.dialog_state {
            if !path.is_empty() {
                let new_doc = WillowDocument {
                    id: format!("doc_{}", self.documents.len() + 1),
                    path: path.clone(),
                    content: content.clone(),
                    author: "current_user@example.com".to_string(),
                    timestamp: "Just now".to_string(),
                    size: content.len(),
                };

                self.documents.push(new_doc);
                self.dialog_state = DialogState::None;
                cx.notify();
            }
        }
    }

    pub fn delete_document(
        &mut self,
        document: &WillowDocument,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.documents.retain(|d| d.id != document.id);
        cx.notify();
    }
}

impl Focusable for WillowPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<PanelEvent> for WillowPanel {}

impl Panel for WillowPanel {
    fn persistent_name() -> &'static str {
        "Willow"
    }

    fn position(&self, _window: &Window, _cx: &App) -> DockPosition {
        DockPosition::Left
    }

    fn position_is_valid(&self, position: DockPosition) -> bool {
        matches!(position, DockPosition::Left | DockPosition::Right)
    }

    fn set_position(
        &mut self,
        _position: DockPosition,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        // Position changes are handled by the dock
    }

    fn size(&self, _window: &Window, _cx: &App) -> Pixels {
        self.width.unwrap_or(px(320.))
        // .unwrap_or_else(|| {
        // WillowPanelSettings::get_global(cx)
        //     .default_width
        //     .unwrap_or(px(320.))
        // })
    }

    fn set_size(&mut self, size: Option<Pixels>, _window: &mut Window, cx: &mut Context<Self>) {
        self.width = size;
        self.serialize(cx);
    }

    fn icon(&self, _window: &Window, _cx: &App) -> Option<IconName> {
        Some(IconName::DatabaseZap)
    }

    fn icon_tooltip(&self, _window: &Window, _cx: &App) -> Option<&'static str> {
        Some("Willow Panel - Distributed data store explorer")
    }

    fn toggle_action(&self) -> Box<dyn Action> {
        Box::new(ToggleFocus)
    }

    fn activation_priority(&self) -> u32 {
        3
    }

    fn panel_key() -> &'static str {
        "willow"
    }
}

impl WillowPanel {
    fn serialize(&mut self, cx: &mut Context<Self>) {
        let width = self.width;
        self.pending_serialization = cx.background_executor().spawn(async move {
            KEY_VALUE_STORE
                .write_kvp(
                    WILLOW_PANEL_KEY.into(),
                    serde_json::to_string(&SerializedWillowPanel { width })
                        .unwrap()
                        .into(),
                )
                .await
                .log_err();
            Some(())
        });
    }

    fn render_dialog(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        match &self.dialog_state {
            DialogState::None => None,
            DialogState::CreateDocument { .. } => Some(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .bg(gpui::black().alpha(0.5))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .bg(cx.theme().colors().panel_background)
                            .border_1()
                            .border_color(cx.theme().colors().border)
                            .rounded_lg()
                            .p_4()
                            .w(px(400.))
                            .child(
                                v_flex()
                                    .gap_3()
                                    .child(
                                        Label::new("Create Document")
                                            .size(LabelSize::Default)
                                            .color(Color::Default),
                                    )
                                    .child(
                                        Label::new("Path: documents/new-file.txt")
                                            .size(LabelSize::Small)
                                            .color(Color::Muted),
                                    )
                                    .child(
                                        Label::new("Content: Hello, Willow!")
                                            .size(LabelSize::Small)
                                            .color(Color::Muted),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .justify_end()
                                            .child(
                                                IconButton::new("cancel-create", IconName::Close)
                                                    .on_click(cx.listener(
                                                        |this, _event, _window, cx| {
                                                            this.dialog_state = DialogState::None;
                                                            cx.notify();
                                                        },
                                                    )),
                                            )
                                            .child(
                                                IconButton::new("confirm-create", IconName::Check)
                                                    .on_click(cx.listener(
                                                        |this, _event, window, cx| {
                                                            this.save_document(
                                                                &SaveDocument,
                                                                window,
                                                                cx,
                                                            );
                                                        },
                                                    )),
                                            ),
                                    ),
                            ),
                    ),
            ),
            DialogState::CreateSubspace { .. } => Some(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .bg(gpui::black().alpha(0.5))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .bg(cx.theme().colors().panel_background)
                            .border_1()
                            .border_color(cx.theme().colors().border)
                            .rounded_lg()
                            .p_4()
                            .w(px(400.))
                            .child(
                                v_flex()
                                    .gap_3()
                                    .child(
                                        Label::new("Create Subspace")
                                            .size(LabelSize::Default)
                                            .color(Color::Default),
                                    )
                                    .child(
                                        Label::new("Name: New Subspace")
                                            .size(LabelSize::Small)
                                            .color(Color::Muted),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .justify_end()
                                            .child(
                                                IconButton::new("cancel-subspace", IconName::Close)
                                                    .on_click(cx.listener(
                                                        |this, _event, _window, cx| {
                                                            this.dialog_state = DialogState::None;
                                                            cx.notify();
                                                        },
                                                    )),
                                            )
                                            .child(
                                                IconButton::new(
                                                    "confirm-subspace",
                                                    IconName::Check,
                                                )
                                                .on_click(cx.listener(
                                                    |this, _event, _window, cx| {
                                                        let new_subspace = WillowSubspace {
                                                            id: format!(
                                                                "subspace_{}",
                                                                this.subspaces.len() + 1
                                                            ),
                                                            name: "New Subspace".to_string(),
                                                            document_count: 0,
                                                            created_at: "Just now".to_string(),
                                                        };
                                                        this.subspaces.push(new_subspace);
                                                        this.dialog_state = DialogState::None;
                                                        cx.notify();
                                                    },
                                                )),
                                            ),
                                    ),
                            ),
                    ),
            ),
            DialogState::ViewDocument { document, .. } => Some(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .bg(gpui::black().alpha(0.5))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .bg(cx.theme().colors().panel_background)
                            .border_1()
                            .border_color(cx.theme().colors().border)
                            .rounded_lg()
                            .p_4()
                            .w(px(600.))
                            .h(px(500.))
                            .child(
                                v_flex()
                                    .gap_3()
                                    .child(
                                        h_flex()
                                            .justify_between()
                                            .items_center()
                                            .child(
                                                Label::new(&document.path)
                                                    .size(LabelSize::Default)
                                                    .color(Color::Default),
                                            )
                                            .child(
                                                IconButton::new("close-document", IconName::Close)
                                                    .on_click(cx.listener(
                                                        |this, _event, _window, cx| {
                                                            this.dialog_state = DialogState::None;
                                                            cx.notify();
                                                        },
                                                    )),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .overflow_hidden()
                                            .p_3()
                                            .bg(cx.theme().colors().editor_background)
                                            .rounded_md()
                                            .border_1()
                                            .border_color(cx.theme().colors().border_variant)
                                            .child(
                                                Label::new(&document.content)
                                                    .size(LabelSize::Small)
                                                    .color(Color::Default),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .justify_between()
                                            .child(
                                                Label::new(format!(
                                                    "{} bytes • {}",
                                                    document.size, document.author
                                                ))
                                                .size(LabelSize::XSmall)
                                                .color(Color::Muted),
                                            )
                                            .child(
                                                Label::new(&document.timestamp)
                                                    .size(LabelSize::XSmall)
                                                    .color(Color::Muted),
                                            ),
                                    ),
                            ),
                    ),
            ),
        }
    }
}

impl Render for WillowPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let main_content = v_flex()
            .size_full()
            .bg(cx.theme().colors().panel_background)
            .on_action(cx.listener(Self::create_document))
            .on_action(cx.listener(Self::create_subspace))
            .on_action(cx.listener(Self::close_dialog))
            .on_action(cx.listener(Self::save_document))
            .child(
                h_flex()
                    .w_full()
                    .justify_between()
                    .p_2()
                    .border_b_1()
                    .border_color(cx.theme().colors().border_variant)
                    .child(
                        Label::new("Willow Data Store")
                            .size(LabelSize::Default)
                            .color(Color::Default),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(IconButton::new("create-document", IconName::File).on_click(
                                cx.listener(|this, _event, window, cx| {
                                    this.create_document(&CreateDocument, window, cx);
                                }),
                            ))
                            .child(
                                IconButton::new("create-subspace", IconName::Person).on_click(
                                    cx.listener(|this, _event, window, cx| {
                                        this.create_subspace(&CreateSubspace, window, cx);
                                    }),
                                ),
                            ),
                    ),
            )
            .child(
                div().flex_1().overflow_hidden().p_3().child(
                    v_flex()
                        .gap_3()
                        .child(
                            v_flex()
                                .gap_2()
                                .child(
                                    h_flex()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            Label::new("Subspaces")
                                                .size(LabelSize::Small)
                                                .color(Color::Muted),
                                        )
                                        .child(
                                            Label::new(format!("{}", self.subspaces.len()))
                                                .size(LabelSize::XSmall)
                                                .color(Color::Muted),
                                        ),
                                )
                                .children(self.subspaces.iter().enumerate().map(
                                    |(i, subspace)| {
                                        ListItem::new(("subspace", i))
                                            .spacing(ListItemSpacing::Dense)
                                            .start_slot(
                                                Icon::new(IconName::Person)
                                                    .size(IconSize::Small)
                                                    .color(Color::Accent),
                                            )
                                            .child(
                                                v_flex()
                                                    .child(
                                                        Label::new(&subspace.name)
                                                            .size(LabelSize::Small)
                                                            .color(Color::Default),
                                                    )
                                                    .child(
                                                        Label::new(format!(
                                                            "{} documents",
                                                            subspace.document_count
                                                        ))
                                                        .size(LabelSize::XSmall)
                                                        .color(Color::Muted),
                                                    ),
                                            )
                                    },
                                )),
                        )
                        .child(
                            div()
                                .h(px(1.))
                                .w_full()
                                .bg(cx.theme().colors().border_variant),
                        )
                        .child(
                            v_flex()
                                .gap_2()
                                .child(
                                    h_flex()
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            Label::new("Documents")
                                                .size(LabelSize::Small)
                                                .color(Color::Muted),
                                        )
                                        .child(
                                            Label::new(format!("{}", self.documents.len()))
                                                .size(LabelSize::XSmall)
                                                .color(Color::Muted),
                                        ),
                                )
                                .children(self.documents.iter().enumerate().map(|(i, document)| {
                                    ListItem::new(("doc", i))
                                        .spacing(ListItemSpacing::Dense)
                                        .start_slot(
                                            Icon::new(IconName::File)
                                                .size(IconSize::Small)
                                                .color(Color::Default),
                                        )
                                        .child(
                                            v_flex()
                                                .child(
                                                    Label::new(&document.path)
                                                        .size(LabelSize::Small)
                                                        .color(Color::Default),
                                                )
                                                .child(
                                                    h_flex()
                                                        .gap_2()
                                                        .child(
                                                            Label::new(format!(
                                                                "{} bytes",
                                                                document.size
                                                            ))
                                                            .size(LabelSize::XSmall)
                                                            .color(Color::Muted),
                                                        )
                                                        .child(
                                                            Label::new("•")
                                                                .size(LabelSize::XSmall)
                                                                .color(Color::Muted),
                                                        )
                                                        .child(
                                                            Label::new(&document.author)
                                                                .size(LabelSize::XSmall)
                                                                .color(Color::Muted),
                                                        ),
                                                ),
                                        )
                                        .end_slot(
                                            h_flex()
                                                .gap_1()
                                                .child(
                                                    IconButton::new(("view", i), IconName::Eye)
                                                        .on_click({
                                                            let document = document.clone();
                                                            cx.listener(
                                                                move |this, _event, _window, cx| {
                                                                    this.dialog_state =
                                                                        DialogState::ViewDocument {
                                                                            document: document
                                                                                .clone(),
                                                                        };
                                                                    cx.notify();
                                                                },
                                                            )
                                                        }),
                                                )
                                                .child(
                                                    IconButton::new(("delete", i), IconName::Trash)
                                                        .on_click({
                                                            let document = document.clone();
                                                            cx.listener(
                                                                move |this, _event, window, cx| {
                                                                    this.delete_document(
                                                                        &document, window, cx,
                                                                    );
                                                                },
                                                            )
                                                        }),
                                                ),
                                        )
                                }))
                                .when(self.documents.is_empty(), |this| {
                                    this.child(
                                        Label::new("No documents yet. Create one to get started!")
                                            .size(LabelSize::XSmall)
                                            .color(Color::Muted),
                                    )
                                }),
                        ),
                ),
            );

        if let Some(dialog) = self.render_dialog(cx) {
            div().relative().child(main_content).child(dialog)
        } else {
            main_content
        }
    }
}
