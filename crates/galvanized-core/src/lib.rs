use std::sync::Arc;

use gpui::{
    Action, AnyView, App, AppContext as _, Context, Div, Entity, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, Styled, Window, WindowBounds, WindowOptions, div,
    prelude::FluentBuilder, px, rgb, size,
};
use gpui_component::resizable::{h_resizable, resizable_panel};
use tracing::info;

pub fn init(cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(600.), px(400.)), cx)),
        ..Default::default()
    };

    let _window_handle = cx
        .open_window(options, |_window, cx| {
            //
            cx.new(|cx| Workspace::new(cx))
        })
        .expect("should open main Galvanized window");
}

/// Minimal reproduction of Zed's Workspace entity, serves as the top-level view
pub struct Workspace {
    //
    left_panel: Option<AnyView>,
    main_content: Option<Box<AnyView>>,
    actions: Vec<Box<dyn Fn(Div, &Workspace, &mut Window, &mut Context<Self>) -> Div>>,
}

impl Workspace {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            left_panel: None,
            main_content: None,
            actions: Default::default(),
        }
    }

    pub fn register_action<A: Action>(
        &mut self,
        callback: impl Fn(&mut Self, &A, &mut Window, &mut Context<Self>) + 'static,
    ) -> &mut Self {
        let callback = Arc::new(callback);

        self.actions.push(Box::new(move |div, _, _, cx| {
            let callback = callback.clone();
            div.on_action(cx.listener(move |workspace, event, window, cx| {
                (callback)(workspace, event, window, cx)
            }))
        }));
        self
    }

    pub fn add_panel(
        &mut self,
        panel: impl Into<AnyView>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        info!("Adding panel");
        self.left_panel = Some(panel.into());
        cx.notify();
    }

    pub fn add_item_to_active_pane(
        &mut self,
        item: Box<impl Into<AnyView>>,
        destination_index: Option<usize>,
        focus_item: bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        let item = *item;
        let view = item.into();
        self.main_content = Some(Box::new(view));
    }
}

pub trait GalvanizedHandle {
    fn register_action<A: Action>(
        &self,
        cx: &mut App,
        action: impl 'static + Fn(&mut Workspace, &A, &mut Window, &mut Context<Workspace>),
    );
}

impl GalvanizedHandle for Entity<Workspace> {
    fn register_action<A: Action>(
        &self,
        cx: &mut App,
        action: impl 'static + Fn(&mut Workspace, &A, &mut Window, &mut Context<Workspace>),
    ) {
        self.update(cx, move |this, _cx| {
            this.register_action(action);
        })
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .debug()
            .bg(rgb(0xaaaaaa))
            .size_full()
            .child(
                //
                h_resizable("resizable")
                    .when_some(self.left_panel.clone(), |el, panel| {
                        //
                        el.child(
                            resizable_panel()
                                //
                                .child(
                                    div()
                                        //
                                        .size_full()
                                        .child(panel),
                                ),
                        )
                    })
                    .when_some(self.main_content.clone(), |el, content| {
                        //
                        el.child(resizable_panel().child(*content))
                    })
                    .when_none(&self.main_content, |el| {
                        //
                        el.child(resizable_panel().child(div().size_full().child("Main View")))
                    }),
            )
    }
}

pub struct Panel {
    //
}

impl Panel {
    pub fn new(_cx: &mut Context<Self>) -> Self {
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
