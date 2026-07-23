use gpui::{
    App, AppContext as _, Context, IntoElement, Render, Styled, Window, WindowBounds,
    WindowOptions, div, px, size,
};

pub fn init(cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(600.), px(400.)), cx)),
        ..Default::default()
    };
    let _window_handle = cx
        .open_window(options, |_window, cx| {
            //
            cx.new(|cx| Galvanized::new(cx))
        })
        .expect("should open main Galvanized window");
}

pub struct Galvanized {
    //
}

impl Galvanized {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {}
    }
}

impl Render for Galvanized {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .debug()
            .size_full()
    }
}
