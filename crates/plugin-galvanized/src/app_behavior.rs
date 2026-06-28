use zed::unstable::{
    gpui::{Action, AnyView, Entity},
    ui::{App, Render, SharedString, Window},
};

use crate::domain::space::Space;

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

    /// The action to invoke when this app is selected from navigation
    fn open_action(&self) -> Box<dyn Action>;

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
    fn id(&self, cx: &App) -> &'static str;
    fn icon(&self, cx: &mut App) -> SharedString;
    fn title(&self, cx: &mut App) -> SharedString;
    fn open_action(&self, cx: &mut App) -> Box<dyn Action>;
    fn to_any_view(&self) -> AnyView;
    fn boxed_clone(&self) -> Box<dyn AppHandle>;
    fn space_context_menu_items(&self, space: Entity<Space>, cx: &App)
    -> Vec<SpaceContextMenuItem>;
}

impl<T: AppBehavior> AppHandle for Entity<T> {
    fn id(&self, cx: &App) -> &'static str {
        self.read(cx).id()
    }

    fn icon(&self, cx: &mut App) -> SharedString {
        self.read(cx).icon()
    }

    fn title(&self, cx: &mut App) -> SharedString {
        self.read(cx).title()
    }

    fn open_action(&self, cx: &mut App) -> Box<dyn Action> {
        self.read(cx).open_action()
    }

    fn to_any_view(&self) -> AnyView {
        self.clone().into()
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
