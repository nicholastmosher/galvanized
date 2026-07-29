use gpui::{
    AnyElement, App, Context, Element, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Window, div,
    prelude::FluentBuilder,
};
use gpui_component::{ActiveTheme, StyledExt};

use crate::item::ItemHandle;
use crate::pane_group::SplitDirection;

/// Events emitted by a Pane.
pub enum Event {
    /// The pane should be split in the given direction.
    Split { direction: SplitDirection },
    /// The pane should be removed from its group.
    Remove,
}

impl EventEmitter<Event> for Pane {}

/// A single pane that holds items and displays them with a tab bar.
pub struct Pane {
    items: Vec<Box<dyn ItemHandle>>,
    active_item_index: usize,
    focus_handle: FocusHandle,
}

impl Pane {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            items: Vec::new(),
            active_item_index: 0,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn add_item(&mut self, item: Box<dyn ItemHandle>, cx: &mut Context<Self>) {
        self.items.push(item);
        self.active_item_index = self.items.len().saturating_sub(1);
        cx.notify();
    }

    pub fn remove_item(&mut self, index: usize) {
        if index < self.items.len() {
            self.items.remove(index);
            if self.active_item_index >= self.items.len() && !self.items.is_empty() {
                self.active_item_index = self.items.len() - 1;
            }
        }
    }

    pub fn activate_item(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.items.len() {
            self.active_item_index = index;
            cx.notify();
        }
    }

    pub fn active_item(&self) -> Option<&dyn ItemHandle> {
        self.items
            .get(self.active_item_index)
            .map(|item| item.as_ref())
    }

    pub fn items(&self) -> &[Box<dyn ItemHandle>] {
        &self.items
    }

    pub fn items_len(&self) -> usize {
        self.items.len()
    }

    pub fn active_item_index(&self) -> usize {
        self.active_item_index
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn split(
        &mut self,
        direction: SplitDirection,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.emit(Event::Split { direction });
    }
}

impl Focusable for Pane {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Pane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("pane")
            .size_full()
            .v_flex()
            .overflow_hidden()
            .child(self.render_tab_bar(window, cx))
            .child(
                div()
                    .id("pane-content")
                    .flex_1()
                    .size_full()
                    .overflow_hidden()
                    .child(if let Some(item) = self.active_item() {
                        item.to_any_view().into_any_element()
                    } else {
                        div()
                            .id("pane-placeholder")
                            .size_full()
                            .h_flex()
                            .justify_center()
                            .child("No items")
                            .into_any_element()
                    }),
            )
    }
}

impl Pane {
    fn render_tab_bar(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.items.is_empty() {
            return div().id("tab-bar-empty").into_any();
        }

        let colors = &cx.theme().colors;

        let mut tab_bar = div()
            .id("tab-bar")
            .h_flex()
            .w_full()
            .bg(colors.muted)
            .gap_1()
            .px_1();

        for (index, item) in self.items.iter().enumerate() {
            let is_active = index == self.active_item_index;
            let label = item.tab_content_text(0, cx);

            let tab = div()
                .id(("tab", index))
                .px_3()
                .py_1()
                .cursor_pointer()
                .rounded_t_md()
                .child(label)
                .when(is_active, |tab| {
                    tab.bg(colors.background).text_color(colors.foreground)
                })
                .when(!is_active, |tab| {
                    tab.text_color(colors.muted_foreground)
                        .hover(|tab| tab.bg(colors.list_hover))
                })
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.activate_item(index, cx);
                }));

            tab_bar = tab_bar.child(tab);
        }

        tab_bar.into_any()
    }
}
