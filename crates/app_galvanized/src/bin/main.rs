use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};
use zed::unstable::gpui_platform::application;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    application()
        .add_plugins(gzed::init)
        // .add_plugins(zed::init)
        // .add_plugins(plugin_calendar::init)
        // .add_plugins(plugin_chat::init)
        // .add_plugins(plugin_iroh::init)
        // .add_plugins(plugin_willow::init)
        .run();
}
