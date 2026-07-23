use gpui_platform::application;

pub fn main() {
    //
    application()
        //
        .add_plugin(galvanized_core::init)
        .run();
}
