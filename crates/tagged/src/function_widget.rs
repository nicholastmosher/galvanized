use std::panic::Location;

use zed::unstable::{
    gpui::{
        AppContext as _, Bounds, Entity, EventEmitter, FocusHandle, Focusable, GlobalElementId,
        InspectorElementId, Interactivity, LayoutId,
    },
    ui::{
        ActiveTheme, App, Context, Element, ElementId, IntoElement, ParentElement, Pixels, Render,
        SharedString, Styled, Window, div, px,
    },
    workspace::{Item, Workspace},
};

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, window, cx| {
        let Some(window) = window else {
            return;
        };

        let function = cx.new(FunctionWidget::new);
        let canvas = cx.new(|cx| {
            let mut canvas = FunctionCanvas::new(cx);
            canvas.add(function);
            canvas
        });

        workspace.add_item_to_active_pane(Box::new(canvas), Some(0), true, window, cx);
    })
    .detach();
}

pub struct FunctionCanvas {
    focus_handle: FocusHandle,
    widgets: Vec<Entity<FunctionWidget>>,
}

impl FunctionCanvas {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            //
            focus_handle: cx.focus_handle(),
            widgets: Default::default(),
        }
    }

    pub fn add(&mut self, function: Entity<FunctionWidget>) {
        //
        self.widgets.push(function);
    }
}

impl Render for FunctionCanvas {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .p_2()
            .rounded_lg()
            .bg(cx.theme().colors().editor_background)
            .children(self.widgets.iter().cloned())
    }
}

impl EventEmitter<()> for FunctionCanvas {}
impl Focusable for FunctionCanvas {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
impl Item for FunctionCanvas {
    type Event = ();

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        "FunctionCanvas".into()
    }
}

pub struct FunctionWidget {
    focus_handle: FocusHandle,
    interactivity: Interactivity,
}

impl FunctionWidget {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            interactivity: Interactivity::new(),
        }
    }
}

impl Render for FunctionWidget {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            //
            .w(px(300.))
            .bg(cx.theme().colors().panel_background)
    }
}

impl IntoElement for FunctionWidget {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for FunctionWidget {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        todo!()
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        todo!()
    }

    fn request_layout(
        &mut self,
        global_id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let layout_id = self.interactivity.request_layout(
            global_id,
            inspector_id,
            window,
            cx,
            |style, window, cx| window.request_layout(style, None, cx),
        );

        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        todo!()
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        //

        //

        todo!()
    }
}

impl EventEmitter<()> for FunctionWidget {}
impl Focusable for FunctionWidget {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
