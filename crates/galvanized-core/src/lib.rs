use gpui::{
    AnyView, App, AppContext as _, Context, IntoElement, ParentElement as _, Render, Styled,
    Window, WindowBounds, WindowOptions, div, prelude::FluentBuilder, px, rgb, size,
};
use gpui_component::resizable::{h_resizable, resizable_panel};

pub fn init(cx: &mut App) {
    gpui_component::init(cx);

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(600.), px(400.)), cx)),
        ..Default::default()
    };

    cx.observe_new::<Galvanized>(|galvanized, _window, cx| {
        let panel = cx.new(|cx| Panel::new(cx));
        galvanized.set_left_panel(panel, cx);
    })
    .detach();

    let _window_handle = cx
        .open_window(options, |_window, cx| {
            //
            cx.new(|cx| Galvanized::new(cx))
        })
        .expect("should open main Galvanized window");
}

pub struct Galvanized {
    //
    left_panel: Option<AnyView>,
}

impl Galvanized {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self { left_panel: None }
    }

    pub fn set_left_panel(&mut self, panel: impl Into<AnyView>, cx: &mut Context<Self>) {
        self.left_panel = Some(panel.into());
        cx.notify();
    }
}

impl Render for Galvanized {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .debug()
            .bg(rgb(0xaaaaaa))
            .size_full()
            .child(
                //
                h_resizable("resizable")
                    .when_some(self.left_panel.as_ref(), |el, panel| {
                        //
                        el.child(
                            resizable_panel()
                                //
                                .child(
                                    div()
                                        //
                                        .size_full()
                                        .child(panel.clone()),
                                ), //
                        )
                    })
                    .child(
                        div()
                            //
                            .size_full()
                            .child("Main View")
                            .into_any_element(),
                    ),
            )
    }
}

pub struct Panel {
    //
}

impl Panel {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {}
    }
}

impl Render for Panel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .size_full()
            .debug()
            .child("Panel!")
    }
}
