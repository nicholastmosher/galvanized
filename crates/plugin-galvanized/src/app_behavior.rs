use zed::unstable::{
    gpui::{Action, AnyView, Entity},
    ui::{
        AnyElement, App, Context, InteractiveElement as _, IntoElement, ParentElement as _, Render,
        SharedString, StatefulInteractiveElement as _, Styled, Window, h_flex,
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

    /// The action to invoke when this app is selected from navigation
    fn open_action(&self) -> Box<dyn Action>;

    /// In the app navigation, the element to select this app
    fn nav(&self, _window: &mut Window, _cx: &App) -> impl IntoElement {
        let open_action = self.open_action();
        h_flex()
            .id(format!("nav-{}", self.id()))
            .gap_2()
            .on_click(move |_e, window, cx| {
                window.dispatch_action(open_action.boxed_clone(), cx);
            })
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
    fn id(&self, cx: &App) -> &'static str;
    fn title(&self, cx: &mut App) -> SharedString;
    fn nav(&self, window: &mut Window, cx: &mut App) -> AnyElement;
    fn to_any_view(&self) -> AnyView;
    fn boxed_clone(&self) -> Box<dyn AppHandle>;
    fn space_context_menu_items(&self, space: Entity<Space>, cx: &App)
    -> Vec<SpaceContextMenuItem>;
}

impl<T: AppBehavior> AppHandle for Entity<T> {
    fn id(&self, cx: &App) -> &'static str {
        self.read(cx).id()
    }

    fn title(&self, cx: &mut App) -> SharedString {
        self.read(cx).title()
    }

    fn nav(&self, window: &mut Window, cx: &mut App) -> AnyElement {
        self.read(cx).nav(window, cx).into_any_element()
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
