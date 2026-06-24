use plugin_galvanized::{Galvanized, app_behavior::AppBehavior};
use zed::unstable::{
    gpui::AppContext as _,
    ui::{App, Context, SharedString},
};

pub fn init(cx: &mut App) {
    //
    cx.observe_new::<Galvanized>(|galvanized, _window, cx| {
        let files_app = cx.new(|cx| FilesApp::new(cx));
        galvanized.add_app(files_app);
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
}
