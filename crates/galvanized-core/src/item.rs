use gpui::{AnyView, SharedString};

/// A handle to an item displayed in a Pane.
/// Wraps an `AnyView` along with metadata needed for the tab bar.
pub struct ItemHandle {
    pub view: AnyView,
    pub tab_label: SharedString,
}

impl ItemHandle {
    pub fn new(view: impl Into<AnyView>, tab_label: impl Into<SharedString>) -> Self {
        Self {
            view: view.into(),
            tab_label: tab_label.into(),
        }
    }
}
