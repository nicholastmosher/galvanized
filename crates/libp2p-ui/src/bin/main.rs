use zed::unstable::gpui::Application;

#[tokio::main]
async fn main() {
    Application::new()
        .add_plugins(zed::init)
        .add_plugins(libp2p_ui::init)
        .run();
}
