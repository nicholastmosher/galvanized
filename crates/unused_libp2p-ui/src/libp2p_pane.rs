use zed::unstable::{
    gpui::{self, AppContext as _, EventEmitter, FocusHandle, Focusable, actions},
    ui::{App, Context, IntoElement, Render, SharedString, Window},
    workspace::{Item, Workspace},
};

actions!(workspace, [OpenLibp2pPane]);

pub fn init(cx: &mut App) {
    let pane = cx.new(|cx| Libp2pPane::new(cx));
    cx.observe_new(|workspace: &mut Workspace, _window, cx| {
        workspace.register_action(
            move |workspace: &mut Workspace, _: &OpenLibp2pPane, window, cx| {
                workspace.add_item_to_active_pane(Box::new(pane), None, true, window, cx);
            },
        );
    });
}

pub struct Libp2pPane {
    focus: FocusHandle,
}

impl Libp2pPane {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus: cx.focus_handle(),
        }
    }
}

impl Focusable for Libp2pPane {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl EventEmitter<()> for Libp2pPane {}

impl Item for Libp2pPane {
    type Event = ();

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> SharedString {
        "Home".into()
    }
}

impl Render for Libp2pPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        todo!()
    }
}
