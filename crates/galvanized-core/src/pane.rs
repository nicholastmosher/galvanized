use gpui::{
    AnyElement, App, AppContext as _, ClickEvent, Context, DragMoveEvent, Element, Entity,
    EventEmitter, FocusHandle, Focusable, InteractiveElement, IntoElement, ParentElement, Render,
    StatefulInteractiveElement, Styled, Window, div, point, prelude::FluentBuilder, px,
};
use gpui_kit::component::{ActiveTheme, StyledExt, h_flex};

use crate::item::ItemHandle;
use crate::pane_group::SplitDirection;

/// Payload carried when a tab is dragged.
#[derive(Clone)]
pub struct DraggedTab {
    pub pane: Entity<Pane>,
    pub item_index: usize,
}

impl Render for DraggedTab {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_3()
            .py_1()
            .bg(cx.theme().colors.accent)
            .text_color(cx.theme().colors.accent_foreground)
            .rounded_md()
            .child("Tab")
    }
}

/// Events emitted by a Pane.
pub enum Event {
    /// The pane should be split in the given direction.
    Split { direction: SplitDirection },
    /// A tab was dropped on the edge of this pane to create a split.
    SplitWithItem {
        direction: SplitDirection,
        source_pane: Entity<Pane>,
        item_index: usize,
    },
    /// A tab was dropped on this pane's tab bar to merge it in.
    MoveItem {
        source_pane: Entity<Pane>,
        item_index: usize,
        target_index: usize,
    },
    /// The pane should be removed from its group.
    Remove,
}

impl EventEmitter<Event> for Pane {}

/// A single pane that holds items and displays them with a tab bar.
pub struct Pane {
    items: Vec<Box<dyn ItemHandle>>,
    active_item_index: usize,
    focus_handle: FocusHandle,
    drag_split_direction: Option<SplitDirection>,
}

impl Pane {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            items: Vec::new(),
            active_item_index: 0,
            focus_handle: cx.focus_handle(),
            drag_split_direction: None,
        }
    }

    pub fn add_item(&mut self, item: Box<dyn ItemHandle>, cx: &mut Context<Self>) {
        self.items.push(item);
        self.active_item_index = self.items.len().saturating_sub(1);
        cx.notify();
    }

    pub fn insert_item_at(
        &mut self,
        index: usize,
        item: Box<dyn ItemHandle>,
        cx: &mut Context<Self>,
    ) {
        let index = index.min(self.items.len());
        self.items.insert(index, item);
        if self.active_item_index >= index {
            self.active_item_index += 1;
        }
        cx.notify();
    }

    pub fn move_item(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if from >= self.items.len() || from == to {
            return;
        }
        let item = self.items.remove(from);
        let to = to.min(self.items.len());
        self.items.insert(to, item);
        if self.active_item_index == from {
            self.active_item_index = to;
        } else if from < self.active_item_index && to >= self.active_item_index {
            self.active_item_index -= 1;
        } else if from > self.active_item_index && to <= self.active_item_index {
            self.active_item_index += 1;
        }
        cx.notify();
    }

    pub fn remove_item(&mut self, index: usize) -> Option<Box<dyn ItemHandle>> {
        if index < self.items.len() {
            let item = self.items.remove(index);
            if self.active_item_index >= self.items.len() && !self.items.is_empty() {
                self.active_item_index = self.items.len() - 1;
            }
            Some(item)
        } else {
            None
        }
    }

    pub fn close_item(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.items.len() {
            self.items.remove(index);
            if self.active_item_index >= self.items.len() && !self.items.is_empty() {
                self.active_item_index = self.items.len() - 1;
            }
            if self.items.is_empty() {
                cx.emit(Event::Remove);
            }
            cx.notify();
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

    fn handle_drag_move(
        &mut self,
        event: &DragMoveEvent<DraggedTab>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let rect = event.bounds.size;
        let drop_target_size = px(80.0);
        let relative_cursor = point(
            event.event.position.x - event.bounds.left(),
            event.event.position.y - event.bounds.top(),
        );

        let direction = if relative_cursor.x < drop_target_size
            || relative_cursor.x > rect.width - drop_target_size
            || relative_cursor.y < drop_target_size
            || relative_cursor.y > rect.height - drop_target_size
        {
            [
                SplitDirection::Up,
                SplitDirection::Right,
                SplitDirection::Down,
                SplitDirection::Left,
            ]
            .iter()
            .min_by_key(|side| match side {
                SplitDirection::Up => relative_cursor.y,
                SplitDirection::Right => rect.width - relative_cursor.x,
                SplitDirection::Down => rect.height - relative_cursor.y,
                SplitDirection::Left => relative_cursor.x,
            })
            .cloned()
        } else {
            None
        };

        if direction != self.drag_split_direction {
            self.drag_split_direction = direction;
            cx.notify();
        }
    }

    fn handle_tab_drop(
        &mut self,
        dragged_tab: &DraggedTab,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(direction) = self.drag_split_direction.take() {
            cx.emit(Event::SplitWithItem {
                direction,
                source_pane: dragged_tab.pane.clone(),
                item_index: dragged_tab.item_index,
            });
        }
        cx.notify();
    }

    fn handle_drop_at_index(
        &mut self,
        dragged_tab: &DraggedTab,
        target_index: usize,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.drag_split_direction = None;
        cx.emit(Event::MoveItem {
            source_pane: dragged_tab.pane.clone(),
            item_index: dragged_tab.item_index,
            target_index,
        });
        cx.notify();
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
                    .relative()
                    .group("")
                    .overflow_hidden()
                    .on_drag_move::<DraggedTab>(cx.listener(Self::handle_drag_move))
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
                    })
                    .child(self.render_drop_target(cx)),
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
            .px_1()
            .on_drop::<DraggedTab>(cx.listener(move |this, dragged_tab, window, cx| {
                this.handle_drop_at_index(dragged_tab, this.items.len(), window, cx);
            }));

        for (index, item) in self.items.iter().enumerate() {
            let is_active = index == self.active_item_index;
            let label = item.tab_content_text(0, cx);

            let tab = h_flex()
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
                }))
                .on_drag(
                    DraggedTab {
                        pane: cx.entity(),
                        item_index: index,
                    },
                    |tab, _, _, cx| cx.new(|_| tab.clone()),
                )
                .on_drop::<DraggedTab>(cx.listener(move |this, dragged_tab, window, cx| {
                    this.handle_drop_at_index(dragged_tab, index, window, cx);
                }))
                .drag_over::<DraggedTab>(|tab, _, _, cx| tab.bg(cx.theme().colors.drop_target))
                .child(
                    div()
                        .id(("tab-close", index))
                        .ml_2()
                        .px_1()
                        .cursor_pointer()
                        .rounded_sm()
                        .text_color(colors.muted_foreground)
                        .hover(|btn| btn.bg(colors.list_hover).text_color(colors.foreground))
                        .child("×")
                        .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                            this.close_item(index, cx);
                            cx.stop_propagation();
                        })),
                );

            tab_bar = tab_bar.child(tab);
        }

        tab_bar.into_any()
    }

    fn render_drop_target(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = &cx.theme().colors;

        let target = div()
            .id("drop-target")
            .absolute()
            .invisible()
            .group_drag_over::<DraggedTab>("", |style| style.visible())
            .on_drop::<DraggedTab>(cx.listener(Self::handle_tab_drop));

        let target = match self.drag_split_direction {
            None => target.top_0().left_0().right_0().bottom_0(),
            Some(SplitDirection::Up) => target
                .top_0()
                .left_0()
                .right_0()
                .h(px(80.))
                .bg(colors.accent.opacity(0.7)),
            Some(SplitDirection::Down) => target
                .bottom_0()
                .left_0()
                .right_0()
                .h(px(80.))
                .bg(colors.accent.opacity(0.7)),
            Some(SplitDirection::Left) => target
                .top_0()
                .left_0()
                .bottom_0()
                .w(px(80.))
                .bg(colors.accent.opacity(0.7)),
            Some(SplitDirection::Right) => target
                .top_0()
                .bottom_0()
                .right_0()
                .w(px(80.))
                .bg(colors.accent.opacity(0.7)),
        };

        target.into_any()
    }
}
