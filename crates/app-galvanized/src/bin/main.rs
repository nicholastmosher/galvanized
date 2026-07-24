use gpui_platform::application;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    application()
        //
        .add_plugin(gzed::init)
        .run();
}
