//! The top-level context of the Galvanized panel is within the scope of a Vault
//!
//! The Vault Menu is like a start menu, it has a button and a menu.
//!
//! This mod contains both pieces used in a `PopoverMenu` to allow full customizability.

use gpui::{
    self, Anchor, App, AppContext as _, ClickEvent, Context, CursorStyle, DismissEvent, Div,
    ElementId, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, RenderOnce, Stateful, StatefulInteractiveElement as _, Styled as _,
    Window, div, linear_color_stop, linear_gradient, point, px,
};
use gpui_component::{ActiveTheme as _, h_flex, v_flex};
use zed_ui::{Clickable, PopoverMenu, Toggleable, Tooltip};

use crate::{Galvanized, domain::vault::Vault};

pub fn render_vault_menu<T>(
    galvanized: Entity<Galvanized>,
    user: Entity<Vault>,
    _window: &mut Window,
    _cx: &mut Context<T>,
) -> impl IntoElement {
    PopoverMenu::new("start-menu")
        .anchor(Anchor::TopLeft)
        .attach(Anchor::TopRight)
        .offset(point(px(6.), px(0.)))
        .trigger(VaultButton::new("vault-button"))
        .menu(move |_window, cx| {
            let user = user.clone();
            let galvanized = galvanized.clone();
            let menu = cx.new(move |cx| VaultMenu::new(user.clone(), galvanized.clone(), cx));
            Some(menu)
        })
}

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
            .active(|style| style.bg(cx.theme().colors.list_hover))
            .child(
                h_flex()
                    .mx_auto()
                    .size_full()
                    .rounded_2xl()
                    .bg(linear_gradient(
                        30. + 180.,
                        linear_color_stop(*crate::panel::GZED_ORANGE, 0.0),
                        linear_color_stop(cx.theme().colors.sidebar.opacity(0.8), 1.0),
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
    galvanized: Entity<Galvanized>,
    user: Entity<Vault>,
}

impl VaultMenu {
    pub fn new(
        user: Entity<Vault>,
        galvanized: Entity<Galvanized>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            galvanized,
            user,
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let apps = self
            .galvanized
            .read(cx)
            .apps
            .iter()
            .map(|(id, it)| (*id, it.boxed_clone()))
            .collect::<Vec<_>>();

        v_flex()
            .bg(cx.theme().colors.background)
            .size_full()
            .border_1()
            .border_color(cx.theme().colors.border)
            .rounded_md()
            .on_mouse_down_out(cx.listener(|_this, _e, _window, cx| {
                cx.emit(DismissEvent);
            }))
            .child(render_menu_header(
                self.galvanized.clone(),
                self.user.clone(),
                cx,
            ))
            .children(apps.into_iter().map(|(id, app)| {
                h_flex()
                    .id(format!("nav-{}", id))
                    .size_full()
                    .p_2()
                    .gap_2()
                    .hover(|style| style.bg(cx.theme().colors.list_hover))
                    .on_click(cx.listener({
                        let app = app.boxed_clone();
                        move |_this, _e, window, cx| {
                            window.dispatch_action(app.open_action(cx), cx);
                            cx.emit(DismissEvent);
                        }
                    }))
                    .child(app.icon(cx))
                    .child(app.title(cx))
            }))
    }
}

fn render_menu_header<T: EventEmitter<DismissEvent>>(
    galvanized: Entity<Galvanized>,
    user: Entity<Vault>,
    cx: &mut Context<T>,
) -> impl IntoElement {
    let user_name = user.read(cx).name();
    let initial = user_name.chars().next().unwrap_or('?').to_string();

    h_flex()
        //
        .p_2()
        .gap_2()
        .border_b_1()
        .border_color(cx.theme().colors.border)
        .child(
            h_flex()
                .size_8()
                .rounded_full()
                .bg(*crate::panel::GZED_ORANGE)
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .mx_auto()
                        .text_xs()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(cx.theme().colors.foreground)
                        .child(initial),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(cx.theme().colors.foreground)
                        .child(user_name),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().colors.muted_foreground)
                        .child("Vault unlocked"),
                ),
        )
        .child(
            // Lock button
            h_flex()
                .id("lock-vault")
                .p_2()
                .gap_2()
                .border_1()
                .border_color(cx.theme().colors.border)
                .rounded_md()
                .hover(|style| style.bg(cx.theme().colors.list_hover))
                .cursor_pointer()
                .on_click(cx.listener(move |_this, _e, _window, cx| {
                    galvanized.update(cx, |it, _cx| it.active_vault = None);
                    cx.emit(DismissEvent);
                }))
                .tooltip(Tooltip::text("Lock Vault")),
        )
}
