//! The top-level context of the Galvanized panel is within the scope of a Vault
//!
//! The Vault Menu is like a start menu, it has a button and a menu.
//!
//! This mod contains both pieces used in a `PopoverMenu` to allow full customizability.

use zed::unstable::{
    gpui::{
        self, AppContext as _, ClickEvent, Corner, CursorStyle, DismissEvent, Entity, EventEmitter,
        FocusHandle, Focusable, Stateful, linear_color_stop, linear_gradient, point, rgba,
    },
    ui::{
        ActiveTheme as _, App, Clickable, Context, Div, ElementId, Icon, IconName,
        InteractiveElement as _, IntoElement, ParentElement as _, PopoverMenu, Render, RenderOnce,
        StatefulInteractiveElement as _, Styled as _, Toggleable, Tooltip, Window, div, h_flex, px,
        v_flex,
    },
};

use crate::{Galvanized, users::User};

pub fn render_vault_menu<T>(
    galvanized: Entity<Galvanized>,
    user: Entity<User>,
    _window: &mut Window,
    _cx: &mut Context<T>,
) -> impl IntoElement {
    PopoverMenu::new("start-menu")
        .anchor(Corner::TopLeft)
        .attach(Corner::TopRight)
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
    galvanized: Entity<Galvanized>,
    user: Entity<User>,
}

impl VaultMenu {
    pub fn new(user: Entity<User>, galvanized: Entity<Galvanized>, cx: &mut Context<Self>) -> Self {
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let apps = self
            .galvanized
            .read(cx)
            .apps
            .iter()
            .map(|it| it.boxed_clone())
            .collect::<Vec<_>>();

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
            .child(render_menu_header(
                self.galvanized.clone(),
                self.user.clone(),
                cx,
            ))
            .children(apps.into_iter().map(|app| {
                //
                div()
                    .id(format!("app-{}", app.id()))
                    //
                    .p_2()
                    .gap_2()
                    .hover(|style| style.bg(cx.theme().colors().element_hover))
                    .child(app.nav(window, cx))
            }))
    }
}

fn render_menu_header<T: EventEmitter<DismissEvent>>(
    galvanized: Entity<Galvanized>,
    user: Entity<User>,
    cx: &mut Context<T>,
) -> impl IntoElement {
    let user_name = user.read(cx).name();
    let initial = user_name.chars().next().unwrap_or('?').to_string();

    h_flex()
        //
        .p_2()
        .gap_2()
        .border_b_1()
        .border_color(cx.theme().colors().border)
        .child(
            h_flex()
                .size_8()
                .rounded_full()
                .bg(rgba(0xea580cff))
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .mx_auto()
                        .text_xs()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(rgba(0xffffffff))
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
                        .text_color(cx.theme().colors().text)
                        .child(user_name),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().colors().text_muted)
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
                .border_color(cx.theme().colors().border)
                .rounded_md()
                .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                .cursor_pointer()
                .on_click(cx.listener(move |_this, _e, _window, cx| {
                    galvanized.update(cx, |it, _cx| it.active_user = None);
                    cx.emit(DismissEvent);
                }))
                .tooltip(Tooltip::text("Lock Vault"))
                .child(Icon::new(IconName::LockOutlined)),
        )
}
