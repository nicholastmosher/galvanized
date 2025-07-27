fn main() {
    gpui::Application::new()
        .add_plugins(zed::init)
        .add_plugins(willow_rummager::init)
        .run();
}
