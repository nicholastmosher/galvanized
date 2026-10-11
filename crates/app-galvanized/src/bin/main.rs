use gpui_platform::application;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    application().run(move |cx| {
        gzed::init(cx);
    });
}
