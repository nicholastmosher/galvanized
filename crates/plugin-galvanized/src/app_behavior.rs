use willow25::entry::NamespaceId;
use zed::unstable::{
    gpui::Entity,
    ui::{AnyElement, App, IntoElement, ParentElement as _, SharedString, Styled, Window, h_flex},
};

/// Trait for static app plugins
pub trait AppBehavior: 'static {
    /// Unique identifier for this app type
    fn id(&self) -> &'static str;

    /// The icon to use for this app's display
    fn icon(&self) -> SharedString; // TODO use better than emoji

    /// The title to display wherever the app is referenced in UI
    fn title(&self) -> SharedString;

    /// In the app navigation, the element to select this app
    fn nav(&self, _window: &mut Window, _cx: &App) -> impl IntoElement {
        h_flex()
            .gap_2()
            .child(self.icon())
            .child(self.title())
            .into_any_element()
    }

    /// Context menu actions to show when right-clicking a space icon.
    /// Each action has a label and a handler to invoke when selected.
    fn space_context_menu_actions(&self, _space_id: NamespaceId) -> Vec<SpaceContextMenuAction> {
        Vec::new()
    }
}

/// An action shown in the context menu when right-clicking a space icon.
pub struct SpaceContextMenuAction {
    pub label: SharedString,
    pub handler: Box<dyn Fn(&mut Window, &mut App)>,
}

pub struct FileAppBehavior;

impl AppBehavior for FileAppBehavior {
    fn id(&self) -> &'static str {
        "files"
    }

    fn icon(&self) -> SharedString {
        "📂".into()
    }

    fn title(&self) -> SharedString {
        "Files".into()
    }
}

pub trait AppHandle: 'static {
    fn id(&self) -> &'static str;
    fn title(&self, cx: &mut App) -> SharedString;
    fn nav(&self, window: &mut Window, cx: &mut App) -> AnyElement;
    fn boxed_clone(&self) -> Box<dyn AppHandle>;
    fn space_context_menu_actions(
        &self,
        space_id: NamespaceId,
        cx: &App,
    ) -> Vec<SpaceContextMenuAction>;
}

impl<T: AppBehavior> AppHandle for Entity<T> {
    fn id(&self) -> &'static str {
        "files"
    }

    fn title(&self, _cx: &mut App) -> SharedString {
        "Files".into()
    }

    fn nav(&self, window: &mut Window, cx: &mut App) -> AnyElement {
        self.read(cx).nav(window, cx).into_any_element()
    }

    fn boxed_clone(&self) -> Box<dyn AppHandle> {
        Box::new(self.clone())
    }

    fn space_context_menu_actions(
        &self,
        space_id: NamespaceId,
        cx: &App,
    ) -> Vec<SpaceContextMenuAction> {
        self.read(cx).space_context_menu_actions(space_id)
    }
}
