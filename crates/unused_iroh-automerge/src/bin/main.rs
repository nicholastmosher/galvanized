use zed::unstable::gpui_platform::application;

#[tokio::main]
async fn main() {
    application()
        .add_plugins(zed::init)
        .add_plugins(iroh_automerge::init)
        .run();
}
