//! The top-level context of the Galvanized panel is within the scope of a Vault
//!
//! The Vault Menu is like a start menu, it has a button and a menu.
//!
//! This mod contains both pieces used in a `PopoverMenu` to allow full customizability.

use zed::unstable::{
    gpui::{
        self, ClickEvent, CursorStyle, DismissEvent, EventEmitter, FocusHandle, Focusable,
        Stateful, linear_color_stop, linear_gradient, rgba,
    },
    ui::{
        ActiveTheme as _, App, Clickable, Context, Div, ElementId, InteractiveElement as _,
        IntoElement, ParentElement as _, Render, RenderOnce, StatefulInteractiveElement as _,
        Styled as _, Toggleable, Window, div, h_flex, px, v_flex,
    },
};

/// Vault menu button, like a start menu, to open the vault-scope context menu
#[derive(IntoElement)]
pub struct VaultButton {
    base: Stateful<Div>,
    selected: bool,
}

impl VaultButton {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            base: h_flex().id(id),
            selected: false,
        }
    }
}

impl Clickable for VaultButton {
    fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.base = self.base.on_click(handler);
        self
    }

    fn cursor_style(mut self, cursor_style: CursorStyle) -> Self {
        self.base = self.base.cursor(cursor_style);
        self
    }
}

impl Toggleable for VaultButton {
    fn toggle_state(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl RenderOnce for VaultButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.base
            .size(px(48.))
            .rounded_2xl()
            .hover(|style| style.rounded_xl().opacity(0.6))
            .active(|style| style.bg(cx.theme().colors().ghost_element_hover))
            .child(
                h_flex()
                    .mx_auto()
                    .size_full()
                    .rounded_2xl()
                    .bg(linear_gradient(
                        30. + 180.,
                        linear_color_stop(rgba(0xff6600ff), 0.0),
                        linear_color_stop(rgba(0x00002bff), 1.0),
                    ))
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            //
                            .mx_auto()
                            .child("G"),
                    ),
            )
    }
}

/// The menu that opens when the Vault menu button is clicked
pub struct VaultMenu {
    focus_handle: FocusHandle,
}

impl VaultMenu {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for VaultMenu {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<DismissEvent> for VaultMenu {}
impl Render for VaultMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .bg(cx.theme().colors().background)
            .min_w_40()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().colors().border)
            .on_mouse_down_out(cx.listener(|_this, _e, _window, cx| {
                cx.emit(DismissEvent);
            }))
            //
            .child(
                //
                h_flex()
                    //
                    .p_2()
                    .gap_2()
                    .border_b_1()
                    .border_color(cx.theme().colors().border)
                    .child("Header"),
            )
            .child(
                //
                h_flex()
                    //
                    .p_2()
                    .gap_2()
                    .child("Item1"),
            )
            .child(
                //
                h_flex()
                    //
                    .p_2()
                    .gap_2()
                    .child("Item2"),
            )
    }
}
