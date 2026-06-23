use zed::unstable::{
    gpui::{DismissEvent, EventEmitter, FocusHandle, Focusable},
    ui::{App, Context, IntoElement, ParentElement as _, Render, Styled as _, Window, div},
};

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
        div()
            .debug()
            //
            .size_80()
            .child("Start Menu")
    }
}
