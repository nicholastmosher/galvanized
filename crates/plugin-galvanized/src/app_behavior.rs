use zed::unstable::{
    gpui::Entity,
    ui::{
        AnyElement, App, IntoElement, ParentElement as _, Render, SharedString, Styled, Window,
        h_flex,
    },
};

use crate::users::Space;

/// Trait for static app plugins
///
/// Requires [`Render`], which becomes the sidebar main view when the app is focused.
pub trait AppBehavior: Render + 'static {
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
    fn space_context_menu_items(
        &self,
        _space: Entity<Space>,
        _cx: &App,
    ) -> Vec<SpaceContextMenuItem> {
        Vec::new()
    }
}

/// An action shown in the context menu when right-clicking a space icon.
pub struct SpaceContextMenuItem {
    pub label: SharedString,
    pub handler: Box<dyn Fn(&mut Window, &mut App)>,
}

pub trait AppHandle: 'static {
    fn id(&self) -> &'static str;
    fn title(&self, cx: &mut App) -> SharedString;
    fn nav(&self, window: &mut Window, cx: &mut App) -> AnyElement;
    fn boxed_clone(&self) -> Box<dyn AppHandle>;
    fn space_context_menu_items(&self, space: Entity<Space>, cx: &App)
    -> Vec<SpaceContextMenuItem>;
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

    fn space_context_menu_items(
        &self,
        space: Entity<Space>,
        cx: &App,
    ) -> Vec<SpaceContextMenuItem> {
        self.read(cx).space_context_menu_items(space, cx)
    }
}
