use zed::unstable::{gpui::Application, gpui_platform::application};

#[tokio::main]
async fn main() {
    application()
        .add_plugins(zed::init)
        .add_plugins(libp2p_ui::init)
        .run();
}
