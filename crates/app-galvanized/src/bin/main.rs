use zed::unstable::gpui_platform::application;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    // gzed::observability::init();

    application()
        // .add_plugins(gzed::observability::init)
        .add_plugins(gzed::init)
        // .add_plugins(zed::init)
        // .add_plugins(plugin_calendar::init)
        // .add_plugins(plugin_chat::init)
        // .add_plugins(plugin_p2p::init)
        // .add_plugins(plugin_willow::init)
        .run();
}
