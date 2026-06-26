use plugin_galvanized::{Galvanized, app_behavior::AppBehavior};
use zed::unstable::{
    gpui::{self, Action, AppContext as _, actions},
    ui::{App, Context, IntoElement, Render, SharedString, Window, div},
};

actions!(
    files,
    [
        //
        OpenFiles,
    ]
);

pub fn init(cx: &mut App) {
    cx.observe_new::<Galvanized>(|galvanized, _window, cx| {
        let files = cx.new(|cx| FilesApp::new(cx));
        galvanized.register_app(files.clone(), cx);
        galvanized.register_action(cx, move |this, _workspace, _: &OpenFiles, _window, cx| {
            let files = files.clone();
            this.panel()
                .update(cx, |panel, cx| panel.set_active_app(files.clone(), cx));
        });
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

    fn open_action(&self) -> Box<dyn Action> {
        Box::new(OpenFiles)
    }
}

impl Render for FilesApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
