use crate::db::kvp::KEY_VALUE_STORE;
use crate::gpui;
use crate::util::ResultExt as _;
use anyhow::{Context as _, bail};
use gpui::{prelude::FluentBuilder, *};
use serde::{Deserialize, Serialize};

// Import Entry from willow_ui for consistent data model
use crate::willow_ui::Entry;

// use willow_panel_settings::WillowPanelSettings;
use crate::workspace::{
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

// Remove old structures and use Entry from willow_ui instead
#[derive(Debug, Clone)]
enum DialogState {
    None,
    CreateDocument { path: String, _content: String },
    CreateSubspace {},
    ViewDocument { entry: Entry },
}

pub struct WillowPanel {
    width: Option<Pixels>,
    pending_serialization: Task<Option<()>>,
    focus_handle: FocusHandle,
    entries: Vec<Entry>,
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
                entries: vec![
                    // Sample data matching willow_ui entries
                    Entry {
                        namespace_id: "family".to_string(),
                        subspace_id: "alice".to_string(),
                        path: "/family/alice/Documents".to_string(),
                        timestamp: 1704067200, // 2 days ago
                    },
                    Entry {
                        namespace_id: "family".to_string(),
                        subspace_id: "alice".to_string(),
                        path: "/family/alice/Photos".to_string(),
                        timestamp: 1703462400, // 1 week ago
                    },
                    Entry {
                        namespace_id: "family".to_string(),
                        subspace_id: "bob".to_string(),
                        path: "/family/bob/Music".to_string(),
                        timestamp: 1703980800, // 3 days ago
                    },
                    Entry {
                        namespace_id: "work".to_string(),
                        subspace_id: "projects".to_string(),
                        path: "/work/projects/willow-fs".to_string(),
                        timestamp: 1704153600, // 1 hour ago
                    },
                    Entry {
                        namespace_id: "work".to_string(),
                        subspace_id: "projects".to_string(),
                        path: "/work/projects/presentation.pdf".to_string(),
                        timestamp: 1704067200, // Yesterday
                    },
                    Entry {
                        namespace_id: "photos".to_string(),
                        subspace_id: "vacation_2024".to_string(),
                        path: "/photos/vacation_2024/beach.jpg".to_string(),
                        timestamp: 1702857600, // 2 weeks ago
                    },
                    Entry {
                        namespace_id: "photos".to_string(),
                        subspace_id: "vacation_2024".to_string(),
                        path: "/photos/vacation_2024/mountains.jpg".to_string(),
                        timestamp: 1702857600, // 2 weeks ago
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
            _content: String::new(),
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
        // For demo purposes, create a mock entry
        if let DialogState::CreateDocument { path, _content: _ } = &self.dialog_state {
            if !path.is_empty() {
                let new_entry = Entry {
                    namespace_id: "user".to_string(),
                    subspace_id: "documents".to_string(),
                    path: path.clone(),
                    timestamp: 1704157200, // Current timestamp (example)
                };

                self.entries.push(new_entry);
                self.dialog_state = DialogState::None;
                cx.notify();
            }
        }
    }

    pub fn delete_document(&mut self, entry: &Entry, _window: &mut Window, cx: &mut Context<Self>) {
        self.entries.retain(|e| {
            !(e.path == entry.path
                && e.namespace_id == entry.namespace_id
                && e.subspace_id == entry.subspace_id)
        });
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
                                                        // Create a sample entry for the new subspace
                                                        let new_entry = Entry {
                                                            namespace_id: "user".to_string(),
                                                            subspace_id: "new_subspace".to_string(),
                                                            path: "/user/new_subspace/welcome.txt"
                                                                .to_string(),
                                                            timestamp: 1704157200, // Current timestamp
                                                        };
                                                        this.entries.push(new_entry);
                                                        this.dialog_state = DialogState::None;
                                                        cx.notify();
                                                    },
                                                )),
                                            ),
                                    ),
                            ),
                    ),
            ),
            DialogState::ViewDocument { entry, .. } => Some(
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
                                                Label::new(&entry.path)
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
                                                v_flex()
                                                    .gap_2()
                                                    .child(
                                                        h_flex()
                                                            .gap_2()
                                                            .child(
                                                                Label::new("Namespace:")
                                                                    .size(LabelSize::Small)
                                                                    .color(Color::Muted),
                                                            )
                                                            .child(
                                                                Label::new(&entry.namespace_id)
                                                                    .size(LabelSize::Small)
                                                                    .color(Color::Default),
                                                            ),
                                                    )
                                                    .child(
                                                        h_flex()
                                                            .gap_2()
                                                            .child(
                                                                Label::new("Subspace:")
                                                                    .size(LabelSize::Small)
                                                                    .color(Color::Muted),
                                                            )
                                                            .child(
                                                                Label::new(&entry.subspace_id)
                                                                    .size(LabelSize::Small)
                                                                    .color(Color::Default),
                                                            ),
                                                    )
                                                    .child(
                                                        h_flex()
                                                            .gap_2()
                                                            .child(
                                                                Label::new("Last Modified:")
                                                                    .size(LabelSize::Small)
                                                                    .color(Color::Muted),
                                                            )
                                                            .child(
                                                                Label::new(&self.format_timestamp(
                                                                    entry.timestamp,
                                                                ))
                                                                .size(LabelSize::Small)
                                                                .color(Color::Default),
                                                            ),
                                                    ),
                                            ),
                                    ),
                            ),
                    ),
            ),
        }
    }

    fn format_timestamp(&self, timestamp: i64) -> String {
        // Simple timestamp formatting - in a real app you'd use a proper date library
        match timestamp {
            1704153600 => "1 hour ago".to_string(),
            1704067200 => "Yesterday".to_string(),
            1703980800 => "3 days ago".to_string(),
            1703462400 => "1 week ago".to_string(),
            1702857600 => "2 weeks ago".to_string(),
            _ => format!("Timestamp: {}", timestamp),
        }
    }

    fn get_file_icon(&self, path: &str) -> IconName {
        let path_parts: Vec<&str> = path.split('/').collect();
        let name = path_parts.last().unwrap_or(&"").to_string();
        let is_directory = !name.contains('.');

        if is_directory {
            IconName::Folder
        } else {
            match name.split('.').last().unwrap_or("") {
                "jpg" | "jpeg" | "png" | "gif" | "bmp" => IconName::Image,
                "pdf" => IconName::File,
                "mp3" | "wav" | "flac" => IconName::File,
                "mp4" | "avi" | "mkv" => IconName::File,
                _ => IconName::File,
            }
        }
    }

    fn render_timeline(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut sorted_entries = self.entries.clone();
        sorted_entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp)); // Most recent first

        v_flex()
            .gap_2()
            // Timeline header
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        Label::new("Recent Activity")
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                    .child(
                        Label::new(format!("{}", sorted_entries.len()))
                            .size(LabelSize::XSmall)
                            .color(Color::Muted),
                    ),
            )
            // Timeline entries
            .children(sorted_entries.iter().enumerate().map(|(i, entry)| {
                let path_parts: Vec<&str> = entry.path.split('/').collect();
                let filename = path_parts.last().unwrap_or(&"").to_string();
                let is_directory = !filename.contains('.');

                ListItem::new(("timeline_entry", i))
                    .spacing(ListItemSpacing::Dense)
                    .start_slot(
                        Icon::new(self.get_file_icon(&entry.path))
                            .size(IconSize::Small)
                            .color(if is_directory {
                                Color::Accent
                            } else {
                                Color::Default
                            }),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Label::new(&filename)
                                            .size(LabelSize::Small)
                                            .color(Color::Default),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_1()
                                            .child(
                                                div()
                                                    .px_1p5()
                                                    .py_0p5()
                                                    .bg(cx.theme().colors().element_background)
                                                    .rounded_md()
                                                    .child(
                                                        Label::new(&entry.namespace_id)
                                                            .size(LabelSize::XSmall)
                                                            .color(Color::Accent),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .px_1p5()
                                                    .py_0p5()
                                                    .bg(cx.theme().colors().element_background)
                                                    .rounded_md()
                                                    .child(
                                                        Label::new(&entry.subspace_id)
                                                            .size(LabelSize::XSmall)
                                                            .color(Color::Muted),
                                                    ),
                                            ),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Label::new(&entry.path)
                                            .size(LabelSize::XSmall)
                                            .color(Color::Muted),
                                    )
                                    .child(
                                        Label::new("•").size(LabelSize::XSmall).color(Color::Muted),
                                    )
                                    .child(
                                        Label::new(&self.format_timestamp(entry.timestamp))
                                            .size(LabelSize::XSmall)
                                            .color(Color::Muted),
                                    ),
                            ),
                    )
                    .end_slot(
                        h_flex()
                            .gap_1()
                            .child(IconButton::new(("view", i), IconName::Eye).on_click({
                                let entry_clone = entry.clone();
                                cx.listener(move |this, _event, _window, cx| {
                                    this.dialog_state = DialogState::ViewDocument {
                                        entry: entry_clone.clone(),
                                    };
                                    cx.notify();
                                })
                            }))
                            .child(IconButton::new(("delete", i), IconName::Trash).on_click({
                                let entry_clone = entry.clone();
                                cx.listener(move |this, _event, window, cx| {
                                    this.delete_document(&entry_clone, window, cx);
                                })
                            })),
                    )
            }))
            .when(sorted_entries.is_empty(), |this| {
                this.child(
                    Label::new("No entries yet. Create some content to get started!")
                        .size(LabelSize::XSmall)
                        .color(Color::Muted),
                )
            })
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
            // Sidebar header
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
            // Sidebar body
            .child(
                div().flex_1().overflow_hidden().p_3().child(
                    v_flex()
                        .gap_3()
                        // Subspaces section
                        .child(self.render_timeline(cx)),
                ),
            );

        if let Some(dialog) = self.render_dialog(cx) {
            div().relative().child(main_content).child(dialog)
        } else {
            main_content
        }
    }
}
