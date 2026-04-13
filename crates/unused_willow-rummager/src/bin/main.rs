use zed::unstable::gpui_platform::application;

fn main() {
    application()
        .add_plugins(zed::init)
        .add_plugins(willow_rummager::init)
        .run();
}
