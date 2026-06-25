use plugin_galvanized::{
    Galvanized,
    app_behavior::{AppBehavior, SpaceContextMenuItem},
    users::Space,
};
use tracing::info;
use zed::unstable::{
    gpui::{AppContext as _, Entity},
    ui::{App, Context, IntoElement, Render, SharedString, Window, div},
};

pub fn init(cx: &mut App) {
    //
    cx.observe_new::<Galvanized>(|galvanized, _window, cx| {
        let files_app = cx.new(|cx| FilesApp::new(cx));
        galvanized.register_app(files_app);
        // galvanized.register_action(cx, |this, _workspace, action: &CreateArea, _window, cx| {
        //     let space_id = action.space_id.clone();
        // });
    })
    .detach();
}

pub struct FilesApp {
    //
}

impl FilesApp {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self {}
    }
}

impl AppBehavior for FilesApp {
    fn id(&self) -> &'static str {
        "files"
    }

    fn icon(&self) -> SharedString {
        "📁".into()
    }

    fn title(&self) -> SharedString {
        "Files".into()
    }

    fn space_context_menu_items(
        &self,
        _space: Entity<Space>,
        _cx: &App,
    ) -> Vec<SpaceContextMenuItem> {
        vec![SpaceContextMenuItem {
            label: "Create Area".into(),
            handler: Box::new(move |_window, _cx| {
                info!("Dispatching CreateArea action");
            }),
        }]
    }
}

impl Render for FilesApp {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
