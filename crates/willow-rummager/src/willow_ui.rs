use crate::gpui;
use crate::workspace::ui::{
    ActiveTheme, Clickable, Color, Icon, IconButton, IconName, IconSize, Label, LabelCommon,
    LabelSize, ListItem, ListItemSpacing, h_flex, v_flex,
};
use crate::workspace::{Item, Workspace};
use gpui::{
    App, AppContext as _, Context, EventEmitter, FocusHandle, Focusable, ParentElement as _,
    Render, SharedString, actions, div, *,
};

actions!(
    workspace,
    [
        /// Open the willow filesystem browser
        OpenWillowUi,
        /// Toggle namespace expansion
        ToggleNamespace,
        /// Toggle subspace expansion
        ToggleSubspace,
        /// Navigate to path
        NavigateToPath,
        /// Create new folder
        CreateFolder,
        /// Upload file
        UploadFile,
    ]
);

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub namespace_id: String,
    pub subspace_id: String,
    pub path: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone)]
pub struct BreadcrumbItem {
    pub name: String,
    // pub path: String,
}

pub fn init(cx: &mut App) {
    cx.observe_new(move |workspace: &mut Workspace, _window, _cx| {
        workspace.register_action(move |workspace, _: &OpenWillowUi, window, cx| {
            let willow_ui = cx.new(|cx| WillowUi::new(cx));
            workspace.add_item_to_active_pane(Box::new(willow_ui), None, true, window, cx)
        });
    })
    .detach();
}

pub struct WillowUi {
    focus_handle: FocusHandle,
    entries: Vec<Entry>,
    current_path: Vec<BreadcrumbItem>,
    selected_namespace: Option<String>,
    selected_subspace: Option<String>,
}

impl WillowUi {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        // Initialize with sample data - flattened entries
        let entries = vec![
            Entry {
                namespace_id: "family".to_string(),
                subspace_id: "alice".to_string(),
                path: "/family/alice/Documents".to_string(),
                timestamp: 1704067200, // 2 days ago (example timestamp)
            },
            Entry {
                namespace_id: "family".to_string(),
                subspace_id: "alice".to_string(),
                path: "/family/alice/Photos".to_string(),
                timestamp: 1703462400, // 1 week ago (example timestamp)
            },
            Entry {
                namespace_id: "family".to_string(),
                subspace_id: "bob".to_string(),
                path: "/family/bob/Music".to_string(),
                timestamp: 1703980800, // 3 days ago (example timestamp)
            },
            Entry {
                namespace_id: "work".to_string(),
                subspace_id: "projects".to_string(),
                path: "/work/projects/willow-fs".to_string(),
                timestamp: 1704153600, // 1 hour ago (example timestamp)
            },
            Entry {
                namespace_id: "work".to_string(),
                subspace_id: "projects".to_string(),
                path: "/work/projects/presentation.pdf".to_string(),
                timestamp: 1704067200, // Yesterday (example timestamp)
            },
            Entry {
                namespace_id: "photos".to_string(),
                subspace_id: "vacation_2024".to_string(),
                path: "/photos/vacation_2024/beach.jpg".to_string(),
                timestamp: 1702857600, // 2 weeks ago (example timestamp)
            },
            Entry {
                namespace_id: "photos".to_string(),
                subspace_id: "vacation_2024".to_string(),
                path: "/photos/vacation_2024/mountains.jpg".to_string(),
                timestamp: 1702857600, // 2 weeks ago (example timestamp)
            },
        ];

        Self {
            focus_handle,
            entries,
            current_path: vec![],
            selected_namespace: None,
            selected_subspace: None,
        }
    }

    fn toggle_namespace(&mut self, namespace_id: &str, _cx: &mut Context<Self>) {
        if self.selected_namespace.as_ref() == Some(&namespace_id.to_string()) {
            self.selected_namespace = None;
            self.selected_subspace = None;
        } else {
            self.selected_namespace = Some(namespace_id.to_string());
            self.selected_subspace = None;
        }
    }

    fn toggle_subspace(&mut self, namespace_id: &str, subspace_id: &str, _cx: &mut Context<Self>) {
        if self.selected_namespace.as_ref() == Some(&namespace_id.to_string())
            && self.selected_subspace.as_ref() == Some(&subspace_id.to_string())
        {
            self.selected_subspace = None;
        } else {
            self.selected_namespace = Some(namespace_id.to_string());
            self.selected_subspace = Some(subspace_id.to_string());
            self.current_path = vec![
                BreadcrumbItem {
                    name: namespace_id.to_string(),
                    // path: format!("/{}", namespace_id),
                },
                BreadcrumbItem {
                    name: subspace_id.to_string(),
                    // path: format!("/{}/{}", namespace_id, subspace_id),
                },
            ];
        }
    }

    fn render_breadcrumbs(&self, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        h_flex()
            .gap_2()
            .items_center()
            .p_2()
            .bg(cx.theme().colors().surface_background)
            .border_b_1()
            .border_color(cx.theme().colors().border_variant)
            .child(
                Icon::new(IconName::File)
                    .size(IconSize::Small)
                    .color(Color::Muted),
            )
            .children(self.current_path.iter().enumerate().map(|(i, item)| {
                let mut flex = h_flex().gap_1();
                if i > 0 {
                    flex = flex.child(
                        Icon::new(IconName::ChevronRight)
                            .size(IconSize::Small)
                            .color(Color::Muted),
                    );
                }
                flex.child(Label::new(&item.name).size(LabelSize::Small).color(
                    if i == self.current_path.len() - 1 {
                        Color::Default
                    } else {
                        Color::Muted
                    },
                ))
            }))
    }

    fn render_namespace(
        &self,
        namespace_id: &str,
        namespace_idx: usize,
        cx: &mut Context<Self>,
    ) -> impl gpui::IntoElement {
        let namespace_id_owned = namespace_id.to_string();
        let is_expanded = self.selected_namespace.as_ref() == Some(&namespace_id_owned);

        let mut namespace_container = v_flex().w_full().child(
            ListItem::new(("namespace", namespace_idx))
                .spacing(ListItemSpacing::Sparse)
                .start_slot(
                    h_flex()
                        .gap_2()
                        .child(
                            IconButton::new(
                                ("expand-ns", namespace_idx),
                                if is_expanded {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                },
                            )
                            .on_click({
                                let namespace_id_for_callback = namespace_id_owned.clone();
                                cx.listener(move |this, _event, _window, cx| {
                                    this.toggle_namespace(&namespace_id_for_callback, cx);
                                    cx.notify();
                                })
                            }),
                        )
                        .child(
                            Icon::new(IconName::Person)
                                .size(IconSize::Small)
                                .color(Color::Accent),
                        ),
                )
                .child(
                    Label::new(&namespace_id_owned)
                        .size(LabelSize::Default)
                        .color(Color::Default),
                )
                .end_slot(
                    Label::new("Namespace")
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                ),
        );

        if is_expanded {
            let subspaces = self.get_subspaces_for_namespace(namespace_id);
            let mut subspace_container = v_flex().ml_6();
            for (subspace_idx, subspace_id) in subspaces.iter().enumerate() {
                subspace_container = subspace_container.child(self.render_subspace(
                    namespace_id,
                    subspace_id,
                    subspace_idx,
                    cx,
                ));
            }
            namespace_container = namespace_container.child(subspace_container);
        }

        namespace_container
    }

    fn render_subspace(
        &self,
        namespace_id: &str,
        subspace_id: &str,
        subspace_idx: usize,
        cx: &mut Context<Self>,
    ) -> impl gpui::IntoElement {
        let subspace_id_owned = subspace_id.to_string();
        let namespace_id_owned = namespace_id.to_string();
        let is_expanded = self.selected_namespace.as_ref() == Some(&namespace_id_owned)
            && self.selected_subspace.as_ref() == Some(&subspace_id_owned);

        let mut subspace_container = v_flex().w_full().child(
            ListItem::new(("subspace", subspace_idx))
                .spacing(ListItemSpacing::Sparse)
                .start_slot(
                    h_flex()
                        .gap_2()
                        .child(
                            IconButton::new(
                                ("expand-ss", subspace_idx),
                                if is_expanded {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                },
                            )
                            .on_click({
                                let namespace_id_for_callback = namespace_id_owned.clone();
                                let subspace_id_for_callback = subspace_id_owned.clone();
                                cx.listener(move |this, _event, _window, cx| {
                                    this.toggle_subspace(
                                        &namespace_id_for_callback,
                                        &subspace_id_for_callback,
                                        cx,
                                    );
                                    cx.notify();
                                })
                            }),
                        )
                        .child(
                            Icon::new(IconName::Person)
                                .size(IconSize::Small)
                                .color(Color::Accent),
                        ),
                )
                .child(
                    Label::new(&subspace_id_owned)
                        .size(LabelSize::Default)
                        .color(Color::Default),
                ),
        );

        if is_expanded {
            let entries = self.get_entries_for_subspace(namespace_id, subspace_id);
            let mut items_container = v_flex().ml_6();
            for (entry_idx, entry) in entries.iter().enumerate() {
                items_container = items_container.child(self.render_entry(entry, entry_idx, cx));
            }
            subspace_container = subspace_container.child(items_container);
        }

        subspace_container
    }

    fn render_entry(
        &self,
        entry: &Entry,
        entry_idx: usize,
        _cx: &mut Context<Self>,
    ) -> impl gpui::IntoElement {
        let path_parts: Vec<&str> = entry.path.split('/').collect();
        let name = path_parts.last().unwrap_or(&"").to_string();
        let is_directory = !name.contains('.');

        let icon = if is_directory {
            IconName::Folder
        } else {
            match name.split('.').last().unwrap_or("") {
                "jpg" | "jpeg" | "png" | "gif" | "bmp" => IconName::Image,
                "pdf" => IconName::File,
                "mp3" | "wav" | "flac" => IconName::File,
                "mp4" | "avi" | "mkv" => IconName::File,
                _ => IconName::File,
            }
        };

        ListItem::new(("entry", entry_idx))
            .spacing(ListItemSpacing::Sparse)
            .start_slot(
                Icon::new(icon)
                    .size(IconSize::Small)
                    .color(if is_directory {
                        Color::Accent
                    } else {
                        Color::Muted
                    }),
            )
            .child(
                h_flex()
                    .justify_between()
                    .w_full()
                    .child(
                        Label::new(&name)
                            .size(LabelSize::Default)
                            .color(Color::Default),
                    )
                    .child(
                        h_flex().gap_4().child(
                            Label::new(&self.format_timestamp(entry.timestamp))
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        ),
                    ),
            )
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

    fn get_namespaces(&self) -> Vec<String> {
        let mut namespaces = Vec::new();
        for entry in &self.entries {
            if !namespaces.contains(&entry.namespace_id) {
                namespaces.push(entry.namespace_id.clone());
            }
        }
        namespaces.sort();
        namespaces
    }

    fn get_subspaces_for_namespace(&self, namespace_id: &str) -> Vec<String> {
        let mut subspaces = Vec::new();
        for entry in &self.entries {
            if entry.namespace_id == namespace_id && !subspaces.contains(&entry.subspace_id) {
                subspaces.push(entry.subspace_id.clone());
            }
        }
        subspaces.sort();
        subspaces
    }

    fn get_entries_for_subspace(&self, namespace_id: &str, subspace_id: &str) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| entry.namespace_id == namespace_id && entry.subspace_id == subspace_id)
            .collect()
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        h_flex()
            .w_full()
            .justify_between()
            .p_2()
            .border_b_1()
            .border_color(cx.theme().colors().border_variant)
            .bg(cx.theme().colors().toolbar_background)
            .child(
                Label::new("Willow Filesystem")
                    .size(LabelSize::Default)
                    .color(Color::Default),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(IconButton::new("create-folder", IconName::Folder))
                    .child(IconButton::new("upload-file", IconName::File))
                    .child(IconButton::new("refresh", IconName::ArrowCircle)),
            )
    }
}

pub enum WillowEvent {
    //
}

impl Item for WillowUi {
    type Event = WillowEvent;

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        "Willow FS".into()
    }
}

impl Focusable for WillowUi {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<WillowEvent> for WillowUi {}

impl Render for WillowUi {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().colors().panel_background)
            .on_action(
                cx.listener(|_this, _action: &ToggleNamespace, _window, cx| {
                    // Handle toggle namespace action
                    cx.notify();
                }),
            )
            .on_action(cx.listener(|_this, _action: &ToggleSubspace, _window, cx| {
                // Handle toggle subspace action
                cx.notify();
            }))
            .child(self.render_toolbar(cx))
            .child(self.render_breadcrumbs(cx))
            .child(div().flex_1().overflow_hidden().p_3().child({
                let mut flex = v_flex().gap_1();
                let namespaces = self.get_namespaces();
                for (namespace_idx, namespace_id) in namespaces.iter().enumerate() {
                    flex = flex.child(self.render_namespace(namespace_id, namespace_idx, cx));
                }
                flex
            }))
    }
}
