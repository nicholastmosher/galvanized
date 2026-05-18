use zed::unstable::{
    gpui,
    ui::{App, IntoElement, RenderOnce, Window, div},
};

#[derive(IntoElement)]
pub struct ProfileLoginPicker {
    //
}

impl ProfileLoginPicker {
    pub fn new() -> Self {
        Self {}
    }
}

impl RenderOnce for ProfileLoginPicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
    }
}
