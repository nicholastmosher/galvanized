use gpui::{AnyView, App, Entity, EventEmitter, Focusable, Render, SharedString};

/// Trait for types that can be displayed as items in a Pane.
/// Implementors must also implement `Render`, `EventEmitter<Self::Event>`, and `Focusable`.
pub trait Item: Focusable + EventEmitter<Self::Event> + Render {
    type Event: 'static;

    /// The text label shown on the tab.
    fn tab_content_text(&self, detail: usize, cx: &App) -> SharedString;
}

/// Object-safe handle for items stored in a Pane.
/// This is the type-erased equivalent of `Entity<T>` where `T: Item`.
pub trait ItemHandle: 'static {
    fn tab_content_text(&self, detail: usize, cx: &App) -> SharedString;
    fn to_any_view(&self) -> AnyView;
    fn boxed_clone(&self) -> Box<dyn ItemHandle>;
}

impl<T: Item> ItemHandle for Entity<T> {
    fn tab_content_text(&self, detail: usize, cx: &App) -> SharedString {
        self.read(cx).tab_content_text(detail, cx)
    }

    fn to_any_view(&self) -> AnyView {
        AnyView::from(self.clone())
    }

    fn boxed_clone(&self) -> Box<dyn ItemHandle> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn ItemHandle> {
    fn clone(&self) -> Self {
        self.boxed_clone()
    }
}
