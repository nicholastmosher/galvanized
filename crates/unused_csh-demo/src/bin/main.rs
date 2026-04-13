use zed::unstable::gpui_platform::application;

fn main() {
    application()
        .add_plugins(zed::init)
        .add_plugins(csh_demo::init)
        .run();
}
